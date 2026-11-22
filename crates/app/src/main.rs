//! Synthbass: plays synth sound from a live bass through JACK (PipeWire).

mod audio;
mod connection;
mod ports;
mod ports_panel;
mod settings;
mod ui;

use eframe::egui;
use rtrb::RingBuffer;

/// The buffer we ask PipeWire for: 128 samples at 48 kHz (2.7 ms). It is a
/// request; PipeWire runs the graph at the smallest one any client asks for.
const REQUESTED_LATENCY: &str = "128/48000";

fn main() -> eframe::Result {
    // PipeWire's JACK library reads this when the client opens. Respect a
    // value the user set themselves.
    if std::env::var_os("PIPEWIRE_LATENCY").is_none() {
        // SAFETY: no other threads exist yet.
        unsafe { std::env::set_var("PIPEWIRE_LATENCY", REQUESTED_LATENCY) };
    }

    let (param_tx, param_rx) = RingBuffer::new(256);
    let (readout_tx, readout_rx) = RingBuffer::new(64);

    // Held for the life of the window; dropping it stops audio.
    let connection = connection::Connection::start(audio::AudioParts {
        params: param_rx,
        readouts: readout_tx,
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([480.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Synthbass",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(ui::SynthbassApp::new(
                param_tx, readout_rx, connection,
            )))
        }),
    )
}
