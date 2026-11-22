//! The JACK client: registers our ports and runs the engine in the process
//! callback. Routing and recovery live in `connection`.
//!
//! The audio thread talks to the UI only through atomics and a wait-free
//! ring buffer, so it never blocks on the UI thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use jack::{AudioIn, AudioOut, Client, ClientOptions, Control, ProcessScope};
use rtrb::Producer;
use synthbass_engine::{Engine, NoiseGate, Param, Patch, Readout};

const CLIENT_NAME: &str = "synthbass";
/// The engine's block size. Larger JACK periods are processed in chunks.
const MAX_BLOCK: usize = 1024;

/// What the audio thread shares with the UI. It outlives any one JACK
/// client, so the app can reconnect to a restarted server without touching
/// the UI.
pub struct AudioParts {
    pub shared_patch: SharedPatch,
    pub readouts: Producer<Readout>,
    pub noise_gate: SharedNoiseGate,
}

/// The patch as the window last set it, param by param, shared with the audio
/// thread. The window stores values rather than queueing changes, so
/// whichever engine runs, including one built after a reconnect or a sample
/// rate change, plays the current patch, and no number of changes made
/// while no engine runs can overflow anything.
#[derive(Clone)]
pub struct SharedPatch(Arc<[AtomicU32; Param::ALL.len()]>);

impl SharedPatch {
    /// Every param at its default.
    pub fn new() -> Self {
        SharedPatch(Arc::new(
            Param::ALL.map(|param| AtomicU32::new(param.default_value().to_bits())),
        ))
    }

    pub fn set(&self, param: Param, value: f32) {
        self.0[param.index()].store(value.to_bits(), Ordering::Relaxed);
    }

    /// Every param of `patch`.
    pub fn set_all(&self, patch: &Patch) {
        for param in Param::ALL {
            self.set(param, patch.get(param));
        }
    }

    /// Copies the shared values into `patch`. Realtime-safe.
    fn load_into(&self, patch: &mut Patch) {
        for (param, shared) in Param::ALL.into_iter().zip(&*self.0) {
            patch.set(param, f32::from_bits(shared.load(Ordering::Relaxed)));
        }
    }
}

/// An engine and the patch it last applied, so it applies one only when
/// something changed. A new one applies the whole patch on its first `sync`.
struct SyncedEngine {
    engine: Engine,
    applied: Patch,
    /// Scratch for reading the shared values; swapped with `applied`, so
    /// syncing never allocates.
    wanted: Patch,
    fresh: bool,
    gate_applied: Option<u32>,
}

impl SyncedEngine {
    fn new(sample_rate: f32) -> Self {
        SyncedEngine {
            engine: Engine::new(sample_rate, MAX_BLOCK),
            applied: Patch::new(""),
            wanted: Patch::new(""),
            fresh: true,
            gate_applied: None,
        }
    }

    /// Brings the engine up to date with the window. Realtime-safe.
    fn sync(&mut self, shared_patch: &SharedPatch, gate: &SharedNoiseGate) {
        shared_patch.load_into(&mut self.wanted);
        if self.fresh || self.wanted != self.applied {
            self.engine.apply_patch(&self.wanted);
            std::mem::swap(&mut self.wanted, &mut self.applied);
            self.fresh = false;
        }
        let bits = gate.0.load(Ordering::Relaxed);
        if self.gate_applied != Some(bits) {
            self.engine.set_noise_gate(gate.get());
            self.gate_applied = Some(bits);
        }
    }

    fn process(&mut self, dry: &[f32], left: &mut [f32], right: &mut [f32]) {
        self.engine.process(dry, left, right);
    }

    fn readout(&self) -> Readout {
        self.engine.readout()
    }
}

/// The global noise gate, shared with the audio thread as its threshold's
/// bits. It is a setting of the room, not of a patch, so it travels apart
/// from the patch.
#[derive(Clone)]
pub struct SharedNoiseGate(Arc<AtomicU32>);

impl SharedNoiseGate {
    pub fn new(gate: NoiseGate) -> Self {
        SharedNoiseGate(Arc::new(AtomicU32::new(gate.threshold().to_bits())))
    }

    pub fn get(&self) -> NoiseGate {
        // Only valid thresholds are stored, so this never falls back.
        NoiseGate::new(f32::from_bits(self.0.load(Ordering::Relaxed))).unwrap_or_default()
    }

    pub fn set(&self, gate: NoiseGate) {
        self.0.store(gate.threshold().to_bits(), Ordering::Relaxed);
    }
}

/// Full JACK names of our own ports.
pub struct OwnPorts {
    pub input: String,
    pub out_1: String,
    pub out_2: String,
}

pub type ActiveClient<N> = jack::AsyncClient<N, Processor>;

/// A running client.
pub struct Opened<N> {
    pub client: ActiveClient<N>,
    pub ports: OwnPorts,
    /// The sample rate the engine was built for.
    pub sample_rate: u32,
}

pub struct OpenError {
    pub error: jack::Error,
    /// `None` if JACK kept the processor when activation failed.
    pub parts: Option<AudioParts>,
}

/// Opens and activates the client. On failure the ring ends come back when
/// possible so a later attempt can use them.
pub fn open<N>(parts: AudioParts, notifier: N) -> Result<Opened<N>, OpenError>
where
    N: 'static + Send + Sync + jack::NotificationHandler,
{
    let fail = |error, parts| OpenError {
        error,
        parts: Some(parts),
    };
    let client = match Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER) {
        Ok((client, _status)) => client,
        Err(err) => return Err(fail(err, parts)),
    };
    let ports = (|| {
        let input = client.register_port("in", AudioIn::default())?;
        let out_1 = client.register_port("out_1", AudioOut::default())?;
        let out_2 = client.register_port("out_2", AudioOut::default())?;
        let names = OwnPorts {
            input: input.name()?,
            out_1: out_1.name()?,
            out_2: out_2.name()?,
        };
        Ok((input, out_1, out_2, names))
    })();
    let (input, out_1, out_2, names) = match ports {
        Ok(ports) => ports,
        Err(err) => return Err(fail(err, parts)),
    };
    let sample_rate = client.sample_rate();
    let processor = Processor {
        engine: SyncedEngine::new(sample_rate as f32),
        input,
        out_1,
        out_2,
        parts,
    };
    match client.activate_async(notifier, processor) {
        Ok(client) => Ok(Opened {
            client,
            ports: names,
            sample_rate,
        }),
        Err(error) => Err(OpenError { error, parts: None }),
    }
}

/// Deactivates the client and takes the ring ends back, if JACK lets go of
/// the processor.
pub fn close<N>(client: ActiveClient<N>) -> Option<AudioParts>
where
    N: 'static + Send + Sync + jack::NotificationHandler,
{
    client
        .deactivate()
        .ok()
        .map(|(_client, _notifier, processor)| processor.parts)
}

pub struct Processor {
    engine: SyncedEngine,
    input: jack::Port<AudioIn>,
    out_1: jack::Port<AudioOut>,
    out_2: jack::Port<AudioOut>,
    parts: AudioParts,
}

impl jack::ProcessHandler for Processor {
    fn process(&mut self, _: &Client, scope: &ProcessScope) -> Control {
        self.engine
            .sync(&self.parts.shared_patch, &self.parts.noise_gate);

        let dry = self.input.as_slice(scope);
        let out_1 = self.out_1.as_mut_slice(scope);
        let out_2 = self.out_2.as_mut_slice(scope);
        for ((dry, left), right) in dry
            .chunks(MAX_BLOCK)
            .zip(out_1.chunks_mut(MAX_BLOCK))
            .zip(out_2.chunks_mut(MAX_BLOCK))
        {
            self.engine.process(dry, left, right);
        }

        // If the UI has fallen behind, it misses this snapshot; the next one
        // supersedes it anyway.
        let _ = self.parts.readouts.push(self.engine.readout());
        Control::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 48_000.0;

    fn sine(seconds: f32) -> Vec<f32> {
        let len = (seconds * RATE) as usize;
        (0..len)
            .map(|n| 0.25 * (std::f32::consts::TAU * 110.0 * n as f32 / RATE).sin())
            .collect()
    }

    /// Peak output over the second half of a 0.4 s sine.
    fn settled_peak(
        engine: &mut SyncedEngine,
        shared_patch: &SharedPatch,
        gate: &SharedNoiseGate,
    ) -> f32 {
        let dry = sine(0.4);
        let (mut left, mut right) = (vec![0.0; dry.len()], vec![0.0; dry.len()]);
        for ((dry, left), right) in dry
            .chunks(128)
            .zip(left.chunks_mut(128))
            .zip(right.chunks_mut(128))
        {
            engine.sync(shared_patch, gate);
            engine.process(dry, left, right);
        }
        left[dry.len() / 2..]
            .iter()
            .fold(0.0f32, |max, s| max.max(s.abs()))
    }

    #[test]
    fn an_engine_built_after_the_patch_was_set_plays_that_patch() {
        // As after a server restart or a sample rate change: the window set
        // the patch long before this engine existed.
        let shared_patch = SharedPatch::new();
        let gate = SharedNoiseGate::new(NoiseGate::default());
        shared_patch.set(Param::DryLevel, 0.5);
        let mut engine = SyncedEngine::new(RATE);
        let peak = settled_peak(&mut engine, &shared_patch, &gate);
        assert!((peak - 0.125).abs() < 0.005, "peak {peak}");
    }

    #[test]
    fn a_change_reaches_the_running_engine() {
        let shared_patch = SharedPatch::new();
        let gate = SharedNoiseGate::new(NoiseGate::default());
        let mut engine = SyncedEngine::new(RATE);
        let before = settled_peak(&mut engine, &shared_patch, &gate);
        shared_patch.set(Param::OutputLevel, 0.0);
        let after = settled_peak(&mut engine, &shared_patch, &gate);
        assert!(before > 0.2, "before {before}");
        assert!(after < 1e-6, "after {after}");
    }

    #[test]
    fn a_whole_patch_reaches_the_running_engine() {
        let shared_patch = SharedPatch::new();
        let gate = SharedNoiseGate::new(NoiseGate::default());
        let mut engine = SyncedEngine::new(RATE);
        let before = settled_peak(&mut engine, &shared_patch, &gate);
        let mut patch = Patch::new("Half dry");
        patch.set(Param::DryLevel, 0.5);
        shared_patch.set_all(&patch);
        let after = settled_peak(&mut engine, &shared_patch, &gate);
        assert!((before - 0.25).abs() < 0.005, "before {before}");
        assert!((after - 0.125).abs() < 0.005, "after {after}");
    }

    #[test]
    fn no_number_of_changes_without_an_engine_is_lost() {
        let shared_patch = SharedPatch::new();
        let gate = SharedNoiseGate::new(NoiseGate::default());
        for step in 0..=1000 {
            shared_patch.set(Param::DryLevel, 1.0 - step as f32 / 1000.0);
        }
        let mut engine = SyncedEngine::new(RATE);
        let peak = settled_peak(&mut engine, &shared_patch, &gate);
        assert!(peak < 1e-6, "peak {peak}");
    }
}
