//! Keeps the app connected. A supervisor thread owns the JACK client, opens
//! it (retrying while the server is unreachable), follows the chosen ports as
//! they come and go, and publishes a `View` for the window.
//!
//! It runs apart from the UI thread, so reconnecting works while the window
//! is minimised or busy, and the UI never waits on JACK.

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use jack::{AudioIn, Client, LatencyType, PortFlags, PortSpec};

use crate::audio::{self, ActiveClient, AudioParts, OwnPorts};
use crate::ports::{Health, PortLists, Routing, Slot};
use crate::settings::{self, Settings};

/// How often the supervisor re-checks the graph without being woken.
const POLL: Duration = Duration::from_secs(1);

/// What the window shows about the connection.
#[derive(Debug, Clone, Default)]
pub struct View {
    pub status: Status,
    pub ports: PortLists,
    pub routing: Routing,
    pub buffer: Option<BufferInfo>,
    /// A one-off problem worth showing, e.g. an unreadable settings file.
    pub notice: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub enum Status {
    #[default]
    Starting,
    Connected,
    ServerUnreachable(String),
    /// The server went away and the app could not get its audio state back.
    ServerLost,
    InterfaceNotFound,
    Vanished(Vec<String>),
}

#[derive(Debug, Clone, Copy)]
pub struct BufferInfo {
    pub frames: u32,
    pub sample_rate: u32,
    /// Input-to-output latency JACK reports for the current links, if any.
    pub round_trip_frames: Option<u32>,
}

enum Command {
    Choose(Slot, Option<String>),
    GraphChanged,
    ServerShutdown,
    Quit,
}

/// The UI's handle. Dropping it stops the supervisor and closes the client.
pub struct Connection {
    view: Arc<Mutex<View>>,
    commands: Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Connection {
    pub fn start(parts: AudioParts) -> Connection {
        let view = Arc::new(Mutex::new(View::default()));
        let (commands, inbox) = mpsc::channel();
        let path = settings::default_path();
        let loaded = path.as_deref().map(Settings::load);
        let supervisor = Supervisor {
            client: None,
            parts: Some(parts),
            routing: None,
            settings: loaded
                .as_ref()
                .map(|l| l.settings.clone())
                .unwrap_or_default(),
            settings_path: path,
            view: view.clone(),
            notifier_tx: commands.clone(),
        };
        view.lock().unwrap().notice = loaded.and_then(|l| l.warning);
        let thread = std::thread::Builder::new()
            .name("synthbass-connection".into())
            .spawn(move || supervisor.run(inbox))
            .expect("spawn connection thread");
        Connection {
            view,
            commands,
            thread: Some(thread),
        }
    }

    pub fn view(&self) -> View {
        self.view.lock().unwrap().clone()
    }

    pub fn choose(&self, slot: Slot, port: Option<String>) {
        let _ = self.commands.send(Command::Choose(slot, port));
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Wakes the supervisor from JACK's notification thread. JACK forbids
/// server calls there, so it only passes the news on.
pub struct Notifier {
    tx: Sender<Command>,
}

impl jack::NotificationHandler for Notifier {
    fn port_registration(&mut self, _: &Client, _: jack::PortId, _: bool) {
        let _ = self.tx.send(Command::GraphChanged);
    }

    unsafe fn shutdown(&mut self, _: jack::ClientStatus, _: &str) {
        let _ = self.tx.send(Command::ServerShutdown);
    }
}

struct Supervisor {
    client: Option<(ActiveClient<Notifier>, OwnPorts)>,
    /// Ring ends waiting for a client; `None` while a client holds them.
    parts: Option<AudioParts>,
    /// `None` until the first successful open picks the launch routing.
    routing: Option<Routing>,
    settings: Settings,
    settings_path: Option<PathBuf>,
    view: Arc<Mutex<View>>,
    notifier_tx: Sender<Command>,
}

impl Supervisor {
    fn run(mut self, inbox: Receiver<Command>) {
        loop {
            self.tick();
            match inbox.recv_timeout(POLL) {
                Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(Command::Choose(slot, port)) => self.choose(slot, port),
                Ok(Command::ServerShutdown) => self.drop_client(),
                Ok(Command::GraphChanged) | Err(RecvTimeoutError::Timeout) => {}
            }
        }
        self.drop_client();
    }

    fn tick(&mut self) {
        if self.client.is_none() && !self.open() {
            return;
        }
        let Some((client, own)) = &self.client else {
            return;
        };
        let client = client.as_client();
        let ports = list_ports(client);
        let routing = self
            .routing
            .get_or_insert_with(|| Routing::at_launch(&self.settings.ports, &ports));
        routing.adopt_defaults(&ports);
        for slot in Slot::ALL {
            if let Some(target) = routing
                .get(slot)
                .filter(|t| ports.for_slot(slot).iter().any(|p| p == t))
            {
                let (src, dst) = link(slot, own, target);
                // Already-connected is the common case and not a problem.
                let _ = client.connect_ports_by_name(src, dst);
            }
        }
        let status = match routing.health(&ports) {
            Health::Connected => Status::Connected,
            Health::InterfaceNotFound => Status::InterfaceNotFound,
            Health::Vanished(names) => Status::Vanished(names),
        };
        let buffer = buffer_info(client, own);
        let mut view = self.view.lock().unwrap();
        view.status = status;
        view.ports = ports;
        view.routing = routing.clone();
        view.buffer = Some(buffer);
    }

    /// Tries to open the client; reports why not in the view.
    fn open(&mut self) -> bool {
        let Some(parts) = self.parts.take() else {
            self.view.lock().unwrap().status = Status::ServerLost;
            return false;
        };
        let notifier = Notifier {
            tx: self.notifier_tx.clone(),
        };
        match audio::open(parts, notifier) {
            Ok(client) => {
                self.client = Some(client);
                true
            }
            Err(err) => {
                self.parts = err.parts;
                let mut view = self.view.lock().unwrap();
                view.status = Status::ServerUnreachable(err.error.to_string());
                view.buffer = None;
                view.ports = PortLists::default();
                false
            }
        }
    }

    fn drop_client(&mut self) {
        if let Some((client, _)) = self.client.take() {
            self.parts = audio::close(client);
        }
    }

    fn choose(&mut self, slot: Slot, port: Option<String>) {
        let routing = self
            .routing
            .get_or_insert_with(|| self.settings.ports.clone());
        if let (Some((client, own)), Some(old)) = (&self.client, routing.get(slot)) {
            let (src, dst) = link(slot, own, old);
            let _ = client.as_client().disconnect_ports_by_name(src, dst);
        }
        routing.set(slot, port);
        self.settings.ports = routing.clone();
        if let Some(path) = &self.settings_path
            && let Err(err) = self.settings.save(path)
        {
            self.view.lock().unwrap().notice = Some(format!("Could not save settings: {err}"));
        }
    }
}

/// The (source, destination) pair for a slot's link to `target`.
fn link<'a>(slot: Slot, own: &'a OwnPorts, target: &'a str) -> (&'a str, &'a str) {
    match slot {
        Slot::Input => (target, &own.input),
        Slot::Output1 => (&own.out_1, target),
        Slot::Output2 => (&own.out_2, target),
    }
}

fn list_ports(client: &Client) -> PortLists {
    let spec = AudioIn::default();
    let audio = Some(spec.jack_port_type());
    PortLists {
        sources: client.ports(None, audio, PortFlags::IS_OUTPUT | PortFlags::IS_PHYSICAL),
        sinks: client.ports(None, audio, PortFlags::IS_INPUT | PortFlags::IS_PHYSICAL),
    }
}

fn buffer_info(client: &Client, own: &OwnPorts) -> BufferInfo {
    // Capture latency of our input = everything upstream of it; playback
    // latency of our output = everything downstream. Both are zero when
    // unconnected, so only report a round trip when both are known.
    let latency = |name: &str, mode| {
        client
            .port_by_name(name)
            .and_then(|port| port.get_latency_range(mode).ok())
            .map(|(_, max)| max)
            .filter(|&max| max > 0)
    };
    let frames = client.buffer_size();
    let round_trip_frames = latency(&own.input, LatencyType::Capture)
        .zip(latency(&own.out_1, LatencyType::Playback))
        .map(|(capture, playback)| capture + frames + playback);
    BufferInfo {
        frames,
        sample_rate: client.sample_rate(),
        round_trip_frames,
    }
}
