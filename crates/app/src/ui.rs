//! The window: patch section, tuner, input meter and noise gate, direct
//! layer, tracked layer with its active voices, mix knobs, and connection
//! section.

use std::time::Duration;

use eframe::egui;
use rtrb::Consumer;
use synthbass_engine::{
    Format, LEVEL_FLOOR_DB, MAX_VOICES, NoiseGate, Octave, Param, Readout, Section, TunerReading,
    Waveshape, gain_to_db,
};

use crate::audio::{SharedNoiseGate, SoundParams};
use crate::connection::Connection;
use crate::patch_panel::PatchPanel;
use crate::patches::PatchLibrary;
use crate::{ports_panel, settings};
use synthbass_engine::Patch;

pub struct SynthbassApp {
    sound: SoundParams,
    readouts: Consumer<Readout>,
    readout: Readout,
    connection: Connection,
    /// Every sound param as last sent to the engine, named after the patch
    /// it came from.
    current: Patch,
    patches: PatchPanel,
    noise_gate: SharedNoiseGate,
    gate_threshold: f32,
    settings: settings::Store,
}

impl SynthbassApp {
    pub fn new(
        sound: SoundParams,
        readouts: Consumer<Readout>,
        connection: Connection,
        noise_gate: SharedNoiseGate,
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
            gate_threshold: noise_gate.get().threshold(),
            noise_gate,
            settings,
        };
        app.play(patch);
        app
    }

    /// Makes `patch` the current sound and sends all of it to the engine.
    fn play(&mut self, patch: Patch) {
        self.sound.set_patch(&patch);
        self.current = patch;
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
        let slider = egui::Slider::new(&mut self.gate_threshold, NoiseGate::RANGE)
            .logarithmic(true)
            .text("Noise gate")
            .custom_formatter(|gain, _| Format::Level.text(gain as f32));
        let response = ui.add(slider);
        let Some(gate) = NoiseGate::new(self.gate_threshold) else {
            return;
        };
        if response.changed() {
            self.noise_gate.set(gate);
        }
        // Save once a drag ends rather than on every frame of it.
        if response.drag_stopped() || (response.changed() && !response.dragged()) {
            // A failed save only costs the threshold at the next launch.
            let _ = self.settings.update(|s| s.noise_gate = gate);
        }
    }

    /// The control for `param`: a switch for a waveshape or octave, a
    /// slider for anything else.
    fn control(&mut self, ui: &mut egui::Ui, param: Param) {
        let info = param.info();
        match info.format {
            Format::Waveshape => {
                self.switch(ui, param, &Waveshape::ALL.map(Waveshape::value), |v| {
                    Waveshape::from_value(v).value()
                })
            }
            Format::Octave => self.switch(ui, param, &Octave::ALL.map(Octave::value), |v| {
                Octave::from_value(v).value()
            }),
            _ => self.knob(ui, param),
        }
    }

    fn knob(&mut self, ui: &mut egui::Ui, param: Param) {
        let info = param.info();
        let mut value = self.current.get(param);
        let slider = egui::Slider::new(&mut value, param.range())
            .text(info.label)
            .logarithmic(info.format.logarithmic())
            .custom_formatter(|value, _| info.format.text(value as f32));
        if ui.add(slider).changed() {
            self.change(param, value);
        }
    }

    /// One button per choice, e.g. saw / square or −1 / 0 / +1; the one lit
    /// is the choice the current value reads as (`choice_of`).
    fn switch(
        &mut self,
        ui: &mut egui::Ui,
        param: Param,
        choices: &[f32],
        choice_of: fn(f32) -> f32,
    ) {
        let info = param.info();
        let before = choice_of(self.current.get(param));
        let mut value = before;
        ui.horizontal(|ui| {
            for &choice in choices {
                ui.selectable_value(&mut value, choice, info.format.text(choice));
            }
            ui.label(info.label);
        });
        if value != before {
            self.change(param, value);
        }
    }

    fn change(&mut self, param: Param, value: f32) {
        self.current.set(param, value);
        self.sound.set(param, self.current.get(param));
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
                ui.add(egui::ProgressBar::new(meter_fill(level)).text(Format::Level.text(level)));
                self.noise_gate_knob(ui);
                ui.add_space(8.0);

                for section in Section::ALL {
                    ui.label(section.label());
                    for param in Param::ALL {
                        if param.info().section == section {
                            self.control(ui, param);
                        }
                    }
                    if section == Section::Tracked {
                        self.voices(ui);
                    }
                    ui.add_space(8.0);
                }

                ports_panel::show(ui, &self.connection);
            });
        });

        // Keep the meter moving without input events.
        ui.ctx().request_repaint_after(Duration::from_millis(33));
    }
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
    ((gain_to_db(gain) - LEVEL_FLOOR_DB) / -LEVEL_FLOOR_DB).clamp(0.0, 1.0)
}
