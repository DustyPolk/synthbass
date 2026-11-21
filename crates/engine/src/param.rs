use std::ops::RangeInclusive;

/// A sound parameter the engine accepts changes to. Levels are linear gains.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    DryLevel,
    OutputLevel,
}

impl Param {
    pub const ALL: [Param; 2] = [Param::DryLevel, Param::OutputLevel];

    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Param::DryLevel => 0.0..=1.0,
            // Up to +12 dB so a quiet dry signal can still be brought up; the
            // hard limiter catches anything hotter than its ceiling.
            Param::OutputLevel => 0.0..=4.0,
        }
    }

    pub fn default_value(self) -> f32 {
        match self {
            Param::DryLevel => 1.0,
            Param::OutputLevel => 1.0,
        }
    }

    pub fn clamp(self, value: f32) -> f32 {
        let range = self.range();
        value.clamp(*range.start(), *range.end())
    }
}
