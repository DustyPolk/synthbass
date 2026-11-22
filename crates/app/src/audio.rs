//! The JACK client: registers our ports and runs the engine in the process
//! callback. Routing and recovery live in `connection`.
//!
//! The audio thread talks to the UI only through atomics and a wait-free
//! ring buffer, so it never blocks on the UI thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use jack::{AudioIn, AudioOut, Client, ClientOptions, Control, ProcessScope};
use rtrb::Producer;
use synthbass_engine::{Engine, Param, Readout};

const CLIENT_NAME: &str = "synthbass";
/// The engine's block size. Larger JACK periods are processed in chunks.
const MAX_BLOCK: usize = 1024;

/// What the audio thread shares with the UI. It outlives any one JACK
/// client, so the app can reconnect to a restarted server without touching
/// the UI.
pub struct AudioParts {
    pub sound: SoundParams,
    pub readouts: Producer<Readout>,
    pub noise_gate: NoiseGate,
}

/// Every sound param as the window last set it, shared with the audio
/// thread. The window stores values rather than queueing changes, so
/// whichever engine runs, including one built after a reconnect or a sample
/// rate change, plays the current sound, and no number of changes made
/// while no engine runs can overflow anything.
#[derive(Clone)]
pub struct SoundParams(Arc<[AtomicU32; Param::ALL.len()]>);

impl SoundParams {
    /// Every param at its default.
    pub fn new() -> Self {
        SoundParams(Arc::new(
            Param::ALL.map(|param| AtomicU32::new(param.default_value().to_bits())),
        ))
    }

    pub fn set(&self, param: Param, value: f32) {
        self.0[param.index()].store(value.to_bits(), Ordering::Relaxed);
    }
}

/// An engine and the shared values it has applied, so it applies only what
/// changed. A new one applies everything on its first `sync`.
struct SyncedEngine {
    engine: Engine,
    applied: [Option<u32>; Param::ALL.len()],
    gate_applied: Option<u32>,
}

impl SyncedEngine {
    fn new(sample_rate: f32) -> Self {
        SyncedEngine {
            engine: Engine::new(sample_rate, MAX_BLOCK),
            applied: [None; Param::ALL.len()],
            gate_applied: None,
        }
    }

    /// Brings the engine up to date with the window. Realtime-safe.
    fn sync(&mut self, sound: &SoundParams, gate: &NoiseGate) {
        for ((param, shared), applied) in Param::ALL.iter().zip(&*sound.0).zip(&mut self.applied) {
            let bits = shared.load(Ordering::Relaxed);
            if *applied != Some(bits) {
                self.engine.set_param(*param, f32::from_bits(bits));
                *applied = Some(bits);
            }
        }
        let bits = gate.0.load(Ordering::Relaxed);
        if self.gate_applied != Some(bits) {
            self.engine.set_noise_gate(f32::from_bits(bits));
            self.gate_applied = Some(bits);
        }
    }
}

/// The global noise gate threshold, shared with the audio thread. It is a
/// setting of the room, not of a patch, so it travels apart from sound
/// parameters.
#[derive(Clone)]
pub struct NoiseGate(Arc<AtomicU32>);

impl NoiseGate {
    pub fn new(threshold: f32) -> Self {
        NoiseGate(Arc::new(AtomicU32::new(threshold.to_bits())))
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    pub fn set(&self, threshold: f32) {
        self.0.store(threshold.to_bits(), Ordering::Relaxed);
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
        self.engine.sync(&self.parts.sound, &self.parts.noise_gate);

        let dry = self.input.as_slice(scope);
        let out_1 = self.out_1.as_mut_slice(scope);
        let out_2 = self.out_2.as_mut_slice(scope);
        for ((dry, left), right) in dry
            .chunks(MAX_BLOCK)
            .zip(out_1.chunks_mut(MAX_BLOCK))
            .zip(out_2.chunks_mut(MAX_BLOCK))
        {
            self.engine.engine.process(dry, left, right);
        }

        // If the UI has fallen behind, it misses this snapshot; the next one
        // supersedes it anyway.
        let _ = self.parts.readouts.push(self.engine.engine.readout());
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
    fn settled_peak(engine: &mut SyncedEngine, sound: &SoundParams, gate: &NoiseGate) -> f32 {
        let dry = sine(0.4);
        let (mut left, mut right) = (vec![0.0; dry.len()], vec![0.0; dry.len()]);
        for ((dry, left), right) in dry
            .chunks(128)
            .zip(left.chunks_mut(128))
            .zip(right.chunks_mut(128))
        {
            engine.sync(sound, gate);
            engine.engine.process(dry, left, right);
        }
        left[dry.len() / 2..]
            .iter()
            .fold(0.0f32, |max, s| max.max(s.abs()))
    }

    #[test]
    fn an_engine_built_after_the_sound_was_set_plays_that_sound() {
        // As after a server restart or a sample rate change: the window set
        // the patch long before this engine existed.
        let sound = SoundParams::new();
        let gate = NoiseGate::new(0.002);
        sound.set(Param::DryLevel, 0.5);
        let mut engine = SyncedEngine::new(RATE);
        let peak = settled_peak(&mut engine, &sound, &gate);
        assert!((peak - 0.125).abs() < 0.005, "peak {peak}");
    }

    #[test]
    fn a_change_reaches_the_running_engine() {
        let sound = SoundParams::new();
        let gate = NoiseGate::new(0.002);
        let mut engine = SyncedEngine::new(RATE);
        let before = settled_peak(&mut engine, &sound, &gate);
        sound.set(Param::OutputLevel, 0.0);
        let after = settled_peak(&mut engine, &sound, &gate);
        assert!(before > 0.2, "before {before}");
        assert!(after < 1e-6, "after {after}");
    }

    #[test]
    fn no_number_of_changes_without_an_engine_is_lost() {
        let sound = SoundParams::new();
        let gate = NoiseGate::new(0.002);
        for step in 0..=1000 {
            sound.set(Param::DryLevel, 1.0 - step as f32 / 1000.0);
        }
        let mut engine = SyncedEngine::new(RATE);
        let peak = settled_peak(&mut engine, &sound, &gate);
        assert!(peak < 1e-6, "peak {peak}");
    }
}
