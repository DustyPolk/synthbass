//! The window: patch section, tuner, input meter and noise gate, direct
//! layer, tracked layer with its active voices, mix knobs, and connection
//! section.

use std::time::Duration;

use eframe::egui;
use rtrb::Consumer;
use synthbass_engine::{MAX_VOICES, NOISE_GATE_RANGE, Param, Readout, TunerReading};

use crate::audio::{NoiseGate, SoundParams};
use crate::connection::Connection;
use crate::patch::Patch;
use crate::patch_panel::PatchPanel;
use crate::patches::PatchLibrary;
use crate::{ports_panel, settings};

/// Meter floor; anything quieter shows as an empty bar.
const METER_FLOOR_DB: f32 = -60.0;

pub struct SynthbassApp {
    sound: SoundParams,
    readouts: Consumer<Readout>,
    readout: Readout,
    connection: Connection,
    /// Every sound param as last sent to the engine, named after the patch
    /// it came from.
    current: Patch,
    patches: PatchPanel,
    noise_gate: NoiseGate,
    gate_threshold: f32,
    settings: settings::Store,
}

impl SynthbassApp {
    pub fn new(
        sound: SoundParams,
        readouts: Consumer<Readout>,
        connection: Connection,
        noise_gate: NoiseGate,
        settings: settings::Store,
        library: PatchLibrary,
    ) -> Self {
        let (patches, patch) = PatchPanel::new(library, settings.clone());
        let mut app = SynthbassApp {
            sound,
            readouts,
            readout: Readout::default(),
            connection,
            current: patch.clone(),
            patches,
            gate_threshold: noise_gate.get(),
            noise_gate,
            settings,
        };
        app.play(patch);
        app
    }

    /// Makes `patch` the current sound and sends all of it to the engine.
    fn play(&mut self, patch: Patch) {
        self.current = patch;
        for param in Param::ALL {
            self.send(param);
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
        let response = ui.add(slider);
        if response.changed() {
            self.noise_gate.set(self.gate_threshold);
        }
        // Save once a drag ends rather than on every frame of it.
        if response.drag_stopped() || (response.changed() && !response.dragged()) {
            let threshold = self.gate_threshold;
            // A failed save only costs the threshold at the next launch.
            let _ = self.settings.update(|s| s.noise_gate = threshold);
        }
    }

    fn knob(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let mut value = self.current.get(param);
        let slider = egui::Slider::new(&mut value, param.range()).text(label);
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
            Param::TrackedGlide => {
                slider.custom_formatter(|seconds, _| format!("{:.0} ms", seconds * 1000.0))
            }
        };
        if ui.add(slider).changed() {
            self.change(param, value);
        }
    }

    fn change(&mut self, param: Param, value: f32) {
        self.current.set(param, value);
        self.send(param);
    }

    fn send(&mut self, param: Param) {
        self.sound.set(param, self.current.get(param));
    }

    /// Saw / square switch for a waveshape param.
    fn waveshape(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let before = self.current.get(param);
        let mut value = before;
        ui.horizontal(|ui| {
            for shape in [0.0, 1.0] {
                ui.selectable_value(&mut value, shape, waveshape_name(shape));
            }
            ui.label(label);
        });
        if value != before {
            self.change(param, value);
        }
    }

    /// −1 / 0 / +1 octave switch.
    fn octave(&mut self, ui: &mut egui::Ui, param: Param, label: &str) {
        let before = self.current.get(param);
        let mut value = before;
        ui.horizontal(|ui| {
            for (octave, name) in [(-1.0, "−1"), (0.0, "0"), (1.0, "+1")] {
                ui.selectable_value(&mut value, octave, name);
            }
            ui.label(label);
        });
        if value != before {
            self.change(param, value);
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
            // The window can be shorter than everything in it.
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Synthbass");
                ui.add_space(8.0);

                if let Some(patch) = self.patches.show(ui, &mut self.current) {
                    self.play(patch);
                }
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
                self.knob(ui, Param::TrackedGlide, "Glide");
                self.knob(ui, Param::TrackedLevel, "Tracked layer level");
                self.voices(ui);
                ui.add_space(8.0);

                ui.label("Mix");
                self.knob(ui, Param::DryLevel, "Dry signal level");
                self.knob(ui, Param::OutputLevel, "Output level");
                ui.add_space(8.0);

                ports_panel::show(ui, &self.connection);
            });
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
