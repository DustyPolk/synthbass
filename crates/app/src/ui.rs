//! The window: tuner, input meter and noise gate, direct layer and mix
//! knobs, and connection section.

use std::collections::HashMap;
use std::time::Duration;

use eframe::egui;
use rtrb::{Consumer, Producer};
use synthbass_engine::{NOISE_GATE_RANGE, Param, Readout};

use crate::audio::NoiseGate;
use crate::connection::Connection;
use crate::ports_panel;

/// Meter floor; anything quieter shows as an empty bar.
const METER_FLOOR_DB: f32 = -60.0;

pub struct SynthbassApp {
    params: Producer<(Param, f32)>,
    readouts: Consumer<Readout>,
    readout: Readout,
    connection: Connection,
    /// The window's copy of every param, as last sent to the engine.
    values: HashMap<Param, f32>,
    noise_gate: NoiseGate,
    gate_threshold: f32,
}

impl SynthbassApp {
    pub fn new(
        params: Producer<(Param, f32)>,
        readouts: Consumer<Readout>,
        connection: Connection,
        noise_gate: NoiseGate,
    ) -> Self {
        SynthbassApp {
            params,
            readouts,
            readout: Readout::default(),
            connection,
            values: Param::ALL
                .iter()
                .map(|&param| (param, param.default_value()))
                .collect(),
            gate_threshold: noise_gate.get(),
            noise_gate,
        }
    }

    /// Note name and cents, or a dash when there is no clear single note.
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

    fn noise_gate_knob(&mut self, ui: &mut egui::Ui) {
        let slider = egui::Slider::new(&mut self.gate_threshold, NOISE_GATE_RANGE)
            .logarithmic(true)
            .text("Noise gate")
            .custom_formatter(|gain, _| format_db(gain as f32));
        if ui.add(slider).changed() {
            self.noise_gate.set(self.gate_threshold);
        }
    }

    fn knob(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let value = self
            .values
            .get_mut(&param)
            .expect("every param has a value");
        let slider = egui::Slider::new(value, param.range()).text(label);
        let slider = match param {
            Param::DryLevel | Param::OutputLevel | Param::DirectLevel => {
                slider.custom_formatter(|gain, _| format_db(gain as f32))
            }
            Param::DirectCutoff => slider
                .logarithmic(true)
                .custom_formatter(|hz, _| format!("{hz:.0} Hz")),
            Param::DirectDrive | Param::DirectResonance | Param::DirectEnvelopeSensitivity => {
                slider.custom_formatter(|amount, _| format!("{:.0} %", amount * 100.0))
            }
        };
        if ui.add(slider).changed() {
            // The audio thread drains this every period, so the ring only
            // fills if audio has stopped; then there is nobody to tell.
            let _ = self.params.push((param, *value));
        }
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
            self.noise_gate_knob(ui);
            ui.add_space(8.0);

            ui.label("Direct");
            self.knob(ui, Param::DirectDrive, "Drive");
            self.knob(ui, Param::DirectCutoff, "Cutoff");
            self.knob(ui, Param::DirectResonance, "Resonance");
            self.knob(ui, Param::DirectEnvelopeSensitivity, "Envelope sensitivity");
            self.knob(ui, Param::DirectLevel, "Direct layer level");
            ui.add_space(8.0);

            ui.label("Mix");
            self.knob(ui, Param::DryLevel, "Dry signal level");
            self.knob(ui, Param::OutputLevel, "Output level");
            ui.add_space(8.0);

            ports_panel::show(ui, &self.connection);
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
