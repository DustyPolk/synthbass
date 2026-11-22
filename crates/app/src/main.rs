//! Synthbass: plays synth sound from a live bass through JACK (PipeWire).

mod audio;
mod ui;

use eframe::egui;
use rtrb::RingBuffer;

fn main() -> eframe::Result {
    let (param_tx, param_rx) = RingBuffer::new(256);
    let (readout_tx, readout_rx) = RingBuffer::new(64);

    let noise_gate = audio::NoiseGate::new(synthbass_engine::NOISE_GATE_DEFAULT);
    // Held for the life of the window; dropping it stops audio.
    let audio = audio::start(param_rx, readout_tx, noise_gate.clone());
    let status = audio.status.clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([420.0, 380.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Synthbass",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            Ok(Box::new(ui::SynthbassApp::new(
                param_tx, readout_rx, status, noise_gate,
            )))
        }),
    )
}
