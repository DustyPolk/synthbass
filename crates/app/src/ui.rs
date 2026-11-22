//! The window: tuner, input meter and noise gate, direct layer, tracked
//! layer with its active voices, mix knobs, and connection section.

use std::collections::HashMap;
use std::time::Duration;

use eframe::egui;
use rtrb::{Consumer, Producer};
use synthbass_engine::{MAX_VOICES, NOISE_GATE_RANGE, Param, Readout, TunerReading};

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
            Param::DryLevel | Param::OutputLevel | Param::DirectLevel | Param::TrackedLevel => {
                slider.custom_formatter(|gain, _| format_db(gain as f32))
            }
            Param::DirectCutoff | Param::TrackedCutoff => slider
                .logarithmic(true)
                .custom_formatter(|hz, _| format!("{hz:.0} Hz")),
            Param::DirectDrive
            | Param::DirectResonance
            | Param::DirectEnvelopeSensitivity
            | Param::TrackedOscMix
            | Param::TrackedSubLevel
            | Param::TrackedResonance
            | Param::TrackedFilterEnvelopeAmount
            | Param::TrackedPickSensitivity => {
                slider.custom_formatter(|amount, _| format!("{:.0} %", amount * 100.0))
            }
            Param::TrackedOsc1Waveshape | Param::TrackedOsc2Waveshape => slider
                .step_by(1.0)
                .custom_formatter(|shape, _| waveshape_name(shape as f32).into()),
            Param::TrackedOsc2Octave => slider
                .step_by(1.0)
                .custom_formatter(|octave, _| format!("{octave:+.0}")),
            Param::TrackedOsc2Detune => {
                slider.custom_formatter(|cents, _| format!("{cents:+.0} ¢"))
            }
            Param::TrackedFilterEnvelopeDecay => slider
                .logarithmic(true)
                .custom_formatter(|seconds, _| format!("{:.0} ms", seconds * 1000.0)),
        };
        if ui.add(slider).changed() {
            self.send(param);
        }
    }

    fn send(&mut self, param: Param) {
        // The audio thread drains this every period, so the ring only fills
        // if audio has stopped; then there is nobody to tell.
        let _ = self.params.push((param, self.values[&param]));
    }

    /// Saw / square switch for a waveshape param.
    fn waveshape(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let value = self
            .values
            .get_mut(&param)
            .expect("every param has a value");
        let before = *value;
        ui.horizontal(|ui| {
            for shape in [0.0, 1.0] {
                ui.selectable_value(value, shape, waveshape_name(shape));
            }
            ui.label(label);
        });
        if *value != before {
            self.send(param);
        }
    }

    /// −1 / 0 / +1 octave switch.
    fn octave(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let value = self
            .values
            .get_mut(&param)
            .expect("every param has a value");
        let before = *value;
        ui.horizontal(|ui| {
            for (octave, name) in [(-1.0, "−1"), (0.0, "0"), (1.0, "+1")] {
                ui.selectable_value(value, octave, name);
            }
            ui.label(label);
        });
        if *value != before {
            self.send(param);
        }
    }

    /// One slot per voice: its note and how loud it is, dimmed when idle.
    fn voices(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Voices");
            for slot in 0..MAX_VOICES {
                let voice = self.readout.voices.get(slot);
                ui.vertical(|ui| {
                    ui.set_width(72.0);
                    let name = voice.map_or("–".into(), |v| note_name(v.pitch));
                    let text = egui::RichText::new(name).monospace();
                    ui.label(if voice.is_some() {
                        text.strong()
                    } else {
                        text.weak()
                    });
                    let loudness = voice.map_or(0.0, |v| v.loudness);
                    ui.add(egui::ProgressBar::new(meter_fill(loudness)).desired_width(64.0));
                });
            }
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
            let level = self.readout.input_level;
            ui.add(egui::ProgressBar::new(meter_fill(level)).text(format_db(level)));
            self.noise_gate_knob(ui);
            ui.add_space(8.0);

            ui.label("Direct");
            self.knob(ui, Param::DirectDrive, "Drive");
            self.knob(ui, Param::DirectCutoff, "Cutoff");
            self.knob(ui, Param::DirectResonance, "Resonance");
            self.knob(ui, Param::DirectEnvelopeSensitivity, "Envelope sensitivity");
            self.knob(ui, Param::DirectLevel, "Direct layer level");
            ui.add_space(8.0);

            ui.label("Tracked");
            self.waveshape(ui, Param::TrackedOsc1Waveshape, "Oscillator 1");
            self.waveshape(ui, Param::TrackedOsc2Waveshape, "Oscillator 2");
            self.octave(ui, Param::TrackedOsc2Octave, "Oscillator 2 octave");
            self.knob(ui, Param::TrackedOsc2Detune, "Oscillator 2 detune");
            self.knob(ui, Param::TrackedOscMix, "Oscillator mix (1 → 2)");
            self.knob(ui, Param::TrackedSubLevel, "Sub-oscillator level");
            self.knob(ui, Param::TrackedCutoff, "Cutoff");
            self.knob(ui, Param::TrackedResonance, "Resonance");
            self.knob(
                ui,
                Param::TrackedFilterEnvelopeAmount,
                "Filter envelope amount",
            );
            self.knob(
                ui,
                Param::TrackedFilterEnvelopeDecay,
                "Filter envelope decay",
            );
            self.knob(ui, Param::TrackedPickSensitivity, "Pick sensitivity");
            self.knob(ui, Param::TrackedLevel, "Tracked layer level");
            self.voices(ui);
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

fn waveshape_name(shape: f32) -> &'static str {
    if shape < 0.5 { "Saw" } else { "Square" }
}

/// Nearest note and cents, e.g. "A1 +3¢".
fn note_name(pitch: f32) -> String {
    let note = pitch.round().clamp(0.0, 127.0);
    let reading = TunerReading {
        note: note as u8,
        cents: 100.0 * (pitch - note),
    };
    format!(
        "{}{} {:+.0}¢",
        reading.name(),
        reading.octave(),
        reading.cents
    )
}

/// A level as a meter fill on a dB scale.
fn meter_fill(gain: f32) -> f32 {
    ((gain_to_db(gain) - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0)
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
