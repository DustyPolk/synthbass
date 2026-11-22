//! The Synthbass engine: turns blocks of dry signal into output blocks.
//!
//! Pure DSP with no audio-backend or GUI dependencies (ADR 0003). The
//! processing path is realtime-safe: no allocation, locking, or I/O.

mod direct;
mod ladder;
mod limiter;
mod param;
mod readout;
mod smoothed;
mod tracked;
mod tracker;

use direct::DirectLayer;
pub use ladder::LadderFilter;
pub use limiter::LIMITER_CEILING;
use limiter::Limiter;
pub use param::Param;
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
}

/// Voices pick up what the tracker hears this often (in samples), so a note
/// starts close to when it is detected whatever the block size.
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

    /// Sets the global noise gate threshold (linear amplitude): quieter notes
    /// are not detected, and the direct layer is silent below it. Kept apart from sound parameters so switching
    /// patches never changes how the app copes with room noise.
    pub fn set_noise_gate(&mut self, threshold: f32) {
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
        let chunks = dry
            .chunks(VOICE_UPDATE_SAMPLES)
            .zip(left.chunks_mut(VOICE_UPDATE_SAMPLES))
            .zip(right.chunks_mut(VOICE_UPDATE_SAMPLES));
        for ((dry, left), right) in chunks {
            self.tracker.process(dry);
            self.tracked.update(&self.tracker.notes(), dry);
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
