//! The direct layer: drive → ladder filter (cutoff opened by playing
//! intensity) → level. Shaped from the dry signal sample by sample, so it
//! adds no latency and handles chords like any other signal.

use crate::ladder::LadderFilter;
use crate::param::Param;
use crate::smoothed::Smoothed;

/// Pre-gain into the waveshaper at full drive (+36 dB).
const MAX_DRIVE_GAIN: f32 = 64.0;

/// Octaves the cutoff opens per unit of dry signal level at full envelope
/// sensitivity. A hard-played bass peaks around 0.5, so about 4 octaves.
const ENVELOPE_OCTAVES: f32 = 8.0;

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

    pub fn process(&mut self, dry: f32) -> f32 {
        let envelope = self.envelope.process(dry);
        let octaves = ENVELOPE_OCTAVES * self.envelope_sensitivity.next() * envelope;
        self.filter
            .set_cutoff((self.log2_cutoff.next() + octaves).exp2());
        self.filter.set_resonance(self.resonance.next());
        let driven = (dry * self.drive_gain.next()).tanh();
        self.filter.process(driven) * self.level.next()
    }
}

/// Drive 0..=1 maps exponentially to a pre-gain of 1..=64.
fn drive_gain(drive: f32) -> f32 {
    MAX_DRIVE_GAIN.powf(drive)
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
            attack: 1.0 - (-1.0 / (ENVELOPE_ATTACK_SECONDS * sample_rate)).exp(),
            release: (-1.0 / (ENVELOPE_RELEASE_SECONDS * sample_rate)).exp(),
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
