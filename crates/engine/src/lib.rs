//! The Synthbass engine: turns blocks of dry signal into output blocks.
//!
//! Pure DSP with no audio-backend or GUI dependencies (ADR 0003). The
//! processing path is realtime-safe: no allocation, locking, or I/O.

mod limiter;
mod param;
mod readout;
mod smoothed;

pub use limiter::LIMITER_CEILING;
use limiter::Limiter;
pub use param::Param;
use readout::PeakMeter;
pub use readout::Readout;
use smoothed::Smoothed;

/// Signal chain in this slice: dry signal → dry signal level → output level
/// → hard limiter → outputs 1/2 (dual mono).
pub struct Engine {
    max_block_size: usize,
    dry_level: Smoothed,
    output_level: Smoothed,
    limiter: Limiter,
    input_meter: PeakMeter,
}

impl Engine {
    /// All memory the engine needs is sized here; `process` accepts any block
    /// length up to `max_block_size`.
    pub fn new(sample_rate: f32, max_block_size: usize) -> Self {
        Engine {
            max_block_size,
            dry_level: Smoothed::new(Param::DryLevel.default_value(), sample_rate),
            output_level: Smoothed::new(Param::OutputLevel.default_value(), sample_rate),
            limiter: Limiter::new(sample_rate),
            input_meter: PeakMeter::new(sample_rate),
        }
    }

    /// Sets a parameter, clamped to its range.
    pub fn set_param(&mut self, param: Param, value: f32) {
        let value = param.clamp(value);
        match param {
            Param::DryLevel => self.dry_level.set_target(value),
            Param::OutputLevel => self.output_level.set_target(value),
        }
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
        for ((&d, l), r) in dry.iter().zip(left.iter_mut()).zip(right.iter_mut()) {
            self.input_meter.process(d);
            let out = self
                .limiter
                .process(d * self.dry_level.next() * self.output_level.next());
            *l = out;
            *r = out;
        }
    }

    pub fn readout(&self) -> Readout {
        Readout {
            input_level: self.input_meter.level(),
        }
    }
}
