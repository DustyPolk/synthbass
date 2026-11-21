/// A snapshot of what the engine hears, for the window to show and tests to
/// check.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Readout {
    /// Peak level of the dry signal (linear, 1.0 = full scale), falling back
    /// smoothly after the signal stops.
    pub input_level: f32,
}

/// Peak follower with instant attack and exponential fall.
pub struct PeakMeter {
    level: f32,
    fall: f32,
}

const FALL_TIME_CONSTANT_SECONDS: f32 = 0.25;

impl PeakMeter {
    pub fn new(sample_rate: f32) -> Self {
        PeakMeter {
            level: 0.0,
            fall: (-1.0 / (FALL_TIME_CONSTANT_SECONDS * sample_rate)).exp(),
        }
    }

    pub fn process(&mut self, sample: f32) {
        self.level = sample.abs().max(self.level * self.fall);
    }

    pub fn level(&self) -> f32 {
        self.level
    }
}
