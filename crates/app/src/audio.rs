//! The JACK client: registers our ports and runs the engine in the process
//! callback. Routing and recovery live in `connection`.
//!
//! The audio thread talks to the UI only through wait-free ring buffers, so it
//! never blocks on the UI thread.

use jack::{AudioIn, AudioOut, Client, ClientOptions, Control, ProcessScope};
use rtrb::{Consumer, Producer};
use synthbass_engine::{Engine, Param, Readout};

const CLIENT_NAME: &str = "synthbass";
/// The engine's block size. Larger JACK periods are processed in chunks.
const MAX_BLOCK: usize = 1024;

/// The audio thread's ends of the UI rings. They outlive any one JACK client,
/// so the app can reconnect to a restarted server without touching the UI.
pub struct AudioParts {
    pub params: Consumer<(Param, f32)>,
    pub readouts: Producer<Readout>,
}

/// Full JACK names of our own ports.
pub struct OwnPorts {
    pub input: String,
    pub out_1: String,
    pub out_2: String,
}

pub type ActiveClient<N> = jack::AsyncClient<N, Processor>;

pub struct OpenError {
    pub error: jack::Error,
    /// `None` if JACK kept the processor when activation failed.
    pub parts: Option<AudioParts>,
}

/// Opens and activates the client. On failure the ring ends come back when
/// possible so a later attempt can use them.
pub fn open<N>(parts: AudioParts, notifier: N) -> Result<(ActiveClient<N>, OwnPorts), OpenError>
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
    let processor = Processor {
        engine: Engine::new(client.sample_rate() as f32, MAX_BLOCK),
        input,
        out_1,
        out_2,
        parts,
    };
    match client.activate_async(notifier, processor) {
        Ok(active) => Ok((active, names)),
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
    engine: Engine,
    input: jack::Port<AudioIn>,
    out_1: jack::Port<AudioOut>,
    out_2: jack::Port<AudioOut>,
    parts: AudioParts,
}

impl jack::ProcessHandler for Processor {
    fn process(&mut self, _: &Client, scope: &ProcessScope) -> Control {
        while let Ok((param, value)) = self.parts.params.pop() {
            self.engine.set_param(param, value);
        }

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
