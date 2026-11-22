//! The Synthbass engine: turns blocks of dry signal into output blocks.
//!
//! Pure DSP with no audio-backend or GUI dependencies (ADR 0003). The
//! processing path is realtime-safe: no allocation, locking, or I/O.

mod direct;
mod ladder;
mod limiter;
mod param;
mod patch;
mod readout;
mod smoothed;
mod tracked;
mod tracker;

use direct::DirectLayer;
pub use limiter::LIMITER_CEILING;
use limiter::Limiter;
pub use param::{Format, LEVEL_FLOOR_DB, Octave, Param, ParamInfo, Section, Waveshape, gain_to_db};
pub use patch::{PATCH_VERSION, Patch};
use readout::PeakMeter;
pub use readout::{Readout, TunerReading};
use smoothed::Smoothed;
use tracked::TrackedLayer;
pub use tracked::{MAX_VOICES, VoiceReading, Voices};
use tracker::Tracker;
pub use tracker::{
    DetectedNote, DetectedNotes, MAX_DETECTED_NOTES, NOISE_GATE_DEFAULT, NOISE_GATE_RANGE,
};

/// Signal chain: dry signal → (dry signal level + direct layer + tracked
/// layer) → output level → hard limiter → outputs 1/2 (dual mono).
pub struct Engine {
    max_block_size: usize,
    dry_level: Smoothed,
    output_level: Smoothed,
    direct: DirectLayer,
    limiter: Limiter,
    input_meter: PeakMeter,
    tracker: Tracker,
    tracked: TrackedLayer,
    /// Samples of dry signal until the voices next take the tracker's notes.
    /// Counted across blocks, so voices update on the same samples whatever
    /// the block size.
    until_voice_update: usize,
}

/// Voices pick up what the tracker has heard this often (in samples), so a
/// note starts within this many samples of being detected.
const VOICE_UPDATE_SAMPLES: usize = 32;

impl Engine {
    /// All memory the engine needs is sized here; `process` accepts any block
    /// length up to `max_block_size`.
    pub fn new(sample_rate: f32, max_block_size: usize) -> Self {
        Engine {
            max_block_size,
            dry_level: Smoothed::new(Param::DryLevel.default_value(), sample_rate),
            output_level: Smoothed::new(Param::OutputLevel.default_value(), sample_rate),
            direct: DirectLayer::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            input_meter: PeakMeter::new(sample_rate),
            tracker: Tracker::new(sample_rate),
            tracked: TrackedLayer::new(sample_rate),
            until_voice_update: 0,
        }
    }

    /// Sets a parameter, clamped to its range.
    pub fn set_param(&mut self, param: Param, value: f32) {
        let value = param.clamp(value);
        match param {
            Param::DryLevel => self.dry_level.set_target(value),
            Param::OutputLevel => self.output_level.set_target(value),
            Param::DirectDrive
            | Param::DirectCutoff
            | Param::DirectResonance
            | Param::DirectEnvelopeSensitivity
            | Param::DirectLevel => self.direct.set_param(param, value),
            Param::TrackedOsc1Waveshape
            | Param::TrackedOsc2Waveshape
            | Param::TrackedOsc2Octave
            | Param::TrackedOsc2Detune
            | Param::TrackedOscMix
            | Param::TrackedSubLevel
            | Param::TrackedCutoff
            | Param::TrackedResonance
            | Param::TrackedFilterEnvelopeAmount
            | Param::TrackedFilterEnvelopeDecay
            | Param::TrackedGlide
            | Param::TrackedPickSensitivity
            | Param::TrackedLevel => self.tracked.set_param(param, value),
        }
    }

    /// Sets every sound parameter to the patch's value, smoothed exactly as
    /// the same values set one by one with `set_param`. Realtime-safe.
    pub fn apply_patch(&mut self, patch: &Patch) {
        for param in Param::ALL {
            self.set_param(param, patch.get(param));
        }
    }

    /// Sets the global noise gate threshold (linear amplitude), clamped to
    /// `NOISE_GATE_RANGE`: quieter notes are not detected, and the direct
    /// layer is silent below it. A non-finite threshold is ignored. Kept apart
    /// from sound parameters so switching patches never changes how the app
    /// copes with room noise.
    pub fn set_noise_gate(&mut self, threshold: f32) {
        if !threshold.is_finite() {
            return;
        }
        let threshold = threshold.clamp(*NOISE_GATE_RANGE.start(), *NOISE_GATE_RANGE.end());
        self.tracker.set_gate(threshold);
        self.tracked.set_gate(threshold);
        self.direct.set_gate(threshold);
    }

    /// Processes one block of mono dry signal into the two output blocks.
    /// All three slices must be the same length, at most `max_block_size`.
    pub fn process(&mut self, dry: &[f32], left: &mut [f32], right: &mut [f32]) {
        debug_assert!(
            dry.len() <= self.max_block_size,
            "block exceeds max_block_size"
        );
        assert!(
            left.len() == dry.len() && right.len() == dry.len(),
            "block length mismatch"
        );
        let mut start = 0;
        while start < dry.len() {
            if self.until_voice_update == 0 {
                self.tracked.update(&self.tracker.notes());
                self.until_voice_update = VOICE_UPDATE_SAMPLES;
            }
            let end = dry.len().min(start + self.until_voice_update);
            self.until_voice_update -= end - start;
            let (dry, left, right) = (
                &dry[start..end],
                &mut left[start..end],
                &mut right[start..end],
            );
            start = end;
            self.tracker.process(dry);
            self.tracked.listen(dry);
            for ((&d, l), r) in dry.iter().zip(left.iter_mut()).zip(right.iter_mut()) {
                self.input_meter.process(d);
                let mix =
                    d * self.dry_level.next() + self.direct.process(d) + self.tracked.process();
                let out = self.limiter.process(mix * self.output_level.next());
                *l = out;
                *r = out;
            }
        }
    }

    pub fn readout(&self) -> Readout {
        Readout {
            input_level: self.input_meter.level(),
            tuner: self.tracker.tuner(),
            notes: self.tracker.notes(),
            voices: self.tracked.voices(),
        }
    }
}
