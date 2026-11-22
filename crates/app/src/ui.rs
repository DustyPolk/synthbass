//! The window: tuner, level knobs, input meter, noise gate, and connection
//! status.

use std::time::Duration;

use eframe::egui;
use rtrb::{Consumer, Producer};
use synthbass_engine::{NOISE_GATE_RANGE, Param, Readout};

use crate::audio::{ConnectionStatus, NoiseGate};

/// Meter floor; anything quieter shows as an empty bar.
const METER_FLOOR_DB: f32 = -60.0;

pub struct SynthbassApp {
    params: Producer<(Param, f32)>,
    readouts: Consumer<Readout>,
    readout: Readout,
    status: ConnectionStatus,
    dry_level: f32,
    output_level: f32,
    noise_gate: NoiseGate,
    gate_threshold: f32,
}

impl SynthbassApp {
    pub fn new(
        params: Producer<(Param, f32)>,
        readouts: Consumer<Readout>,
        status: ConnectionStatus,
        noise_gate: NoiseGate,
    ) -> Self {
        SynthbassApp {
            params,
            readouts,
            readout: Readout::default(),
            status,
            dry_level: Param::DryLevel.default_value(),
            output_level: Param::OutputLevel.default_value(),
            gate_threshold: noise_gate.get(),
            noise_gate,
        }
    }

    fn level_knob(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let value = match param {
            Param::DryLevel => &mut self.dry_level,
            Param::OutputLevel => &mut self.output_level,
        };
        let slider = egui::Slider::new(value, param.range())
            .text(label)
            .custom_formatter(|gain, _| format_db(gain as f32));
        if ui.add(slider).changed() {
            // The audio thread drains this every period, so the ring only
            // fills if audio has stopped; then there is nobody to tell.
            let _ = self.params.push((param, *value));
        }
    }
}

impl SynthbassApp {
    /// Note name and cents, or a blank line when there is no clear single
    /// note, so the layout doesn't jump.
    fn tuner(&self, ui: &mut egui::Ui) {
        let text = match self.readout.tuner {
            Some(reading) => format!(
                "{}{}  {:+.0} ¢",
                reading.name(),
                reading.octave(),
                reading.cents
            ),
            None => "–".into(),
        };
        let in_tune = self.readout.tuner.is_some_and(|r| r.cents.abs() < 5.0);
        let color = if in_tune {
            egui::Color32::LIGHT_GREEN
        } else {
            ui.visuals().text_color()
        };
        ui.horizontal(|ui| {
            ui.label("Tuner");
            ui.label(
                egui::RichText::new(text)
                    .size(28.0)
                    .monospace()
                    .color(color),
            );
        });
    }
}

impl eframe::App for SynthbassApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        while let Ok(readout) = self.readouts.pop() {
            self.readout = readout;
        }

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Synthbass");
            ui.add_space(8.0);

            self.tuner(ui);
            ui.add_space(8.0);

            ui.label("Input");
            let db = gain_to_db(self.readout.input_level);
            let fill = ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0);
            ui.add(egui::ProgressBar::new(fill).text(format_db(self.readout.input_level)));
            let gate = egui::Slider::new(&mut self.gate_threshold, NOISE_GATE_RANGE)
                .logarithmic(true)
                .text("Noise gate")
                .custom_formatter(|gain, _| format_db(gain as f32));
            if ui.add(gate).changed() {
                self.noise_gate.set(self.gate_threshold);
            }
            ui.add_space(8.0);

            ui.label("Mix");
            self.level_knob(ui, Param::DryLevel, "Dry signal level");
            self.level_knob(ui, Param::OutputLevel, "Output level");
            ui.add_space(8.0);

            let color = if self.status.ok {
                egui::Color32::LIGHT_GREEN
            } else {
                egui::Color32::LIGHT_RED
            };
            ui.colored_label(color, &self.status.message);
        });

        // Keep the meter moving without input events.
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }
}

fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(1e-6).log10()
}

fn format_db(gain: f32) -> String {
    let db = gain_to_db(gain);
    if db <= METER_FLOOR_DB {
        "−∞ dB".into()
    } else {
        format!("{db:+.1} dB")
    }
}
