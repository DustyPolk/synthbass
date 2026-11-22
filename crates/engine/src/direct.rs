//! The direct layer: drive → ladder filter (cutoff opened by playing
//! intensity) → noise gate → level. Shaped from the dry signal sample by sample, so it
//! adds no latency and handles chords like any other signal.

use crate::ladder::LadderFilter;
use crate::noise_gate::NoiseGate;
use crate::param::Param;
use crate::smoothed::{Smoothed, one_pole_coefficient};

/// Pre-gain into the waveshaper at full drive (+36 dB).
const MAX_DRIVE_GAIN: f32 = 64.0;

/// The dry signal peak the drive is level-matched at: a firmly played note
/// from a passive bass into an interface at full gain peaks around here. A
/// note this loud comes out of the waveshaper as loud as it went in at any
/// drive, so drive changes the tone, not the level against the other layers.
const DRIVE_REFERENCE_PEAK: f32 = 0.05;

/// Octaves the cutoff opens per unit of dry signal level at full envelope
/// sensitivity. A passive bass at full interface gain peaks around 0.1 when
/// dug in, so about 4 octaves. (Patch version 1 had 8, tuned for peaks
/// around 0.5; `Patch::from_toml` rescales v1 sensitivities to match.)
pub(crate) const ENVELOPE_OCTAVES: f32 = 40.0;

const ENVELOPE_ATTACK_SECONDS: f32 = 0.001;
const ENVELOPE_RELEASE_SECONDS: f32 = 0.1;

pub struct DirectLayer {
    drive_gain: Smoothed,
    log2_cutoff: Smoothed,
    resonance: Smoothed,
    envelope_sensitivity: Smoothed,
    level: Smoothed,
    envelope: Envelope,
    filter: LadderFilter,
    gate: Gate,
}

impl DirectLayer {
    pub fn new(sample_rate: f32) -> Self {
        DirectLayer {
            drive_gain: Smoothed::new(drive_gain(Param::DirectDrive.default_value()), sample_rate),
            log2_cutoff: Smoothed::new(Param::DirectCutoff.default_value().log2(), sample_rate),
            resonance: Smoothed::new(Param::DirectResonance.default_value(), sample_rate),
            envelope_sensitivity: Smoothed::new(
                Param::DirectEnvelopeSensitivity.default_value(),
                sample_rate,
            ),
            level: Smoothed::new(Param::DirectLevel.default_value(), sample_rate),
            envelope: Envelope::new(sample_rate),
            filter: LadderFilter::new(sample_rate),
            gate: Gate::new(sample_rate),
        }
    }

    /// Takes an already-clamped value for one of the direct layer's params.
    pub fn set_param(&mut self, param: Param, value: f32) {
        match param {
            Param::DirectDrive => self.drive_gain.set_target(drive_gain(value)),
            // Smoothed in octaves so sweeps sound even across the range.
            Param::DirectCutoff => self.log2_cutoff.set_target(value.log2()),
            Param::DirectResonance => self.resonance.set_target(value),
            Param::DirectEnvelopeSensitivity => self.envelope_sensitivity.set_target(value),
            Param::DirectLevel => self.level.set_target(value),
            _ => debug_assert!(false, "{param:?} is not a direct layer param"),
        }
    }

    pub fn set_gate(&mut self, threshold: f32) {
        self.gate.threshold = threshold;
    }

    pub fn process(&mut self, dry: f32) -> f32 {
        let envelope = self.envelope.process(dry);
        let octaves = ENVELOPE_OCTAVES * self.envelope_sensitivity.next() * envelope;
        self.filter
            .set_cutoff((self.log2_cutoff.next() + octaves).exp2());
        self.filter.set_resonance(self.resonance.next());
        let drive_gain = self.drive_gain.next();
        let driven = (dry * drive_gain).tanh() * drive_makeup(drive_gain);
        let gain = self.gate.process(envelope);
        self.filter.process(driven) * gain * self.level.next()
    }
}

/// Drive 0..=1 maps exponentially to a pre-gain of 1..=64.
fn drive_gain(drive: f32) -> f32 {
    MAX_DRIVE_GAIN.powf(drive)
}

/// Undoes the waveshaper's gain at `DRIVE_REFERENCE_PEAK`.
fn drive_makeup(drive_gain: f32) -> f32 {
    DRIVE_REFERENCE_PEAK / (drive_gain * DRIVE_REFERENCE_PEAK).tanh()
}

/// Follows the dry signal's level: fast attack so the filter opens on the
/// pick, slower release so it closes smoothly as the note decays.
struct Envelope {
    level: f32,
    attack: f32,
    release: f32,
}

impl Envelope {
    fn new(sample_rate: f32) -> Self {
        Envelope {
            level: 0.0,
            attack: 1.0 - one_pole_coefficient(ENVELOPE_ATTACK_SECONDS, sample_rate),
            release: one_pole_coefficient(ENVELOPE_RELEASE_SECONDS, sample_rate),
        }
    }

    fn process(&mut self, sample: f32) -> f32 {
        let rectified = sample.abs();
        if rectified > self.level {
            self.level += (rectified - self.level) * self.attack;
        } else {
            self.level *= self.release;
            if self.level < 1e-15 {
                self.level = 0.0;
            }
        }
        self.level
    }
}

const GATE_ATTACK_SECONDS: f32 = 0.001;
const GATE_RELEASE_SECONDS: f32 = 0.08;
/// The gate closes this far below its threshold (−3 dB), so a note hovering
/// at the threshold does not chatter.
const GATE_HYSTERESIS: f32 = std::f32::consts::FRAC_1_SQRT_2;

/// Silences the layer while the dry signal is below the global noise gate:
/// full drive would otherwise turn the interface's idle noise floor and
/// pickup hum into audible hiss.
struct Gate {
    threshold: f32,
    open: bool,
    gain: f32,
    attack: f32,
    release: f32,
}

impl Gate {
    fn new(sample_rate: f32) -> Self {
        Gate {
            threshold: NoiseGate::default().threshold(),
            open: false,
            gain: 0.0,
            attack: one_pole_coefficient(GATE_ATTACK_SECONDS, sample_rate),
            release: one_pole_coefficient(GATE_RELEASE_SECONDS, sample_rate),
        }
    }

    /// Takes the dry signal's envelope; returns the gain to apply.
    fn process(&mut self, envelope: f32) -> f32 {
        if envelope >= self.threshold {
            self.open = true;
        } else if envelope < self.threshold * GATE_HYSTERESIS {
            self.open = false;
        }
        if self.open {
            self.gain = 1.0 + (self.gain - 1.0) * self.attack;
        } else {
            self.gain *= self.release;
            if self.gain < 1e-6 {
                self.gain = 0.0;
            }
        }
        self.gain
    }
}
