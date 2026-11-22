//! The global noise gate threshold: quieter notes are not detected, and the
//! direct layer is silent below it. A setting of the room, not of a patch.

use std::ops::RangeInclusive;

/// A noise gate threshold (linear amplitude), always finite and inside
/// `RANGE`. `new` is the only way to make one from a number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NoiseGate(f32);

impl NoiseGate {
    /// Thresholds the engine accepts (linear, −80 to −20 dBFS).
    pub const RANGE: RangeInclusive<f32> = 0.0001..=0.1;

    /// ≈ −54 dBFS.
    const DEFAULT: NoiseGate = NoiseGate(0.002);

    /// Clamps a threshold into `RANGE`; `None` when it is not a finite number.
    pub fn new(threshold: f32) -> Option<NoiseGate> {
        threshold
            .is_finite()
            .then(|| NoiseGate(threshold.clamp(*Self::RANGE.start(), *Self::RANGE.end())))
    }

    pub const fn threshold(self) -> f32 {
        self.0
    }
}

impl Default for NoiseGate {
    fn default() -> Self {
        NoiseGate::DEFAULT
    }
}
