//! The JACK side: registers the client, runs the engine in the process
//! callback, and auto-connects the Scarlett.
//!
//! The audio thread talks to the UI only through wait-free ring buffers, so it
//! never blocks on the UI thread.

use jack::{AudioIn, AudioOut, Client, ClientOptions, Control, PortFlags, PortSpec, ProcessScope};
use rtrb::{Consumer, Producer};
use synthbass_engine::{Engine, Param, Readout};

const CLIENT_NAME: &str = "synthbass";
const INTERFACE_PATTERN: &str = "Scarlett";
/// The engine's block size. Larger JACK periods are processed in chunks.
const MAX_BLOCK: usize = 1024;

/// What the window shows on its connection status line.
#[derive(Clone)]
pub struct ConnectionStatus {
    pub ok: bool,
    pub message: String,
}

/// Keeps the JACK client running while held; dropping it deactivates.
pub struct Audio {
    _client: Option<jack::AsyncClient<(), Processor>>,
    pub status: ConnectionStatus,
}

pub fn start(params: Consumer<(Param, f32)>, readouts: Producer<Readout>) -> Audio {
    match start_client(params, readouts) {
        Ok((client, status)) => Audio {
            _client: Some(client),
            status,
        },
        Err(err) => Audio {
            _client: None,
            status: ConnectionStatus {
                ok: false,
                message: format!("Audio server not reachable: {err}"),
            },
        },
    }
}

fn start_client(
    params: Consumer<(Param, f32)>,
    readouts: Producer<Readout>,
) -> Result<(jack::AsyncClient<(), Processor>, ConnectionStatus), jack::Error> {
    let (client, _status) = Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER)?;
    let processor = Processor {
        engine: Engine::new(client.sample_rate() as f32, MAX_BLOCK),
        input: client.register_port("in", AudioIn::default())?,
        out_1: client.register_port("out_1", AudioOut::default())?,
        out_2: client.register_port("out_2", AudioOut::default())?,
        params,
        readouts,
    };
    let own_ports = [
        processor.input.name()?,
        processor.out_1.name()?,
        processor.out_2.name()?,
    ];
    let client = client.activate_async((), processor)?;
    let status = auto_connect(client.as_client(), &own_ports);
    Ok((client, status))
}

/// Connects Scarlett input 1 to our input and our outputs to Scarlett
/// outputs 1/2.
fn auto_connect(client: &Client, [input, out_1, out_2]: &[String; 3]) -> ConnectionStatus {
    let spec = AudioIn::default();
    let audio = Some(spec.jack_port_type());
    let capture = client.ports(
        Some(INTERFACE_PATTERN),
        audio,
        PortFlags::IS_OUTPUT | PortFlags::IS_PHYSICAL,
    );
    let playback = client.ports(
        Some(INTERFACE_PATTERN),
        audio,
        PortFlags::IS_INPUT | PortFlags::IS_PHYSICAL,
    );
    let (Some(source), [dest_1, dest_2, ..]) = (capture.first(), playback.as_slice()) else {
        return ConnectionStatus {
            ok: false,
            message: "Scarlett not found. Connect ports manually or plug it in and restart.".into(),
        };
    };
    let result = client
        .connect_ports_by_name(source, input)
        .and_then(|()| client.connect_ports_by_name(out_1, dest_1))
        .and_then(|()| client.connect_ports_by_name(out_2, dest_2));
    match result {
        Ok(()) => ConnectionStatus {
            ok: true,
            message: format!("Connected: {source} → app → {dest_1}, {dest_2}"),
        },
        Err(err) => ConnectionStatus {
            ok: false,
            message: format!("Could not connect the Scarlett: {err}"),
        },
    }
}

pub struct Processor {
    engine: Engine,
    input: jack::Port<AudioIn>,
    out_1: jack::Port<AudioOut>,
    out_2: jack::Port<AudioOut>,
    params: Consumer<(Param, f32)>,
    readouts: Producer<Readout>,
}

impl jack::ProcessHandler for Processor {
    fn process(&mut self, _: &Client, scope: &ProcessScope) -> Control {
        while let Ok((param, value)) = self.params.pop() {
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
        let _ = self.readouts.push(self.engine.readout());
        Control::Continue
    }
}
