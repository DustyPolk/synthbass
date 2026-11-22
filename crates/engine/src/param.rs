use std::ops::RangeInclusive;

/// A sound parameter the engine accepts changes to. Levels are linear gains;
/// drive, resonance and envelope sensitivity are 0..=1; cutoff is in Hz;
/// a waveshape is 0 (saw) or 1 (square).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Param {
    DryLevel,
    OutputLevel,
    DirectDrive,
    DirectCutoff,
    DirectResonance,
    DirectEnvelopeSensitivity,
    DirectLevel,
    TrackedOsc1Waveshape,
    TrackedLevel,
}

impl Param {
    pub const ALL: [Param; 9] = [
        Param::DryLevel,
        Param::OutputLevel,
        Param::DirectDrive,
        Param::DirectCutoff,
        Param::DirectResonance,
        Param::DirectEnvelopeSensitivity,
        Param::DirectLevel,
        Param::TrackedOsc1Waveshape,
        Param::TrackedLevel,
    ];

    pub fn range(self) -> RangeInclusive<f32> {
        match self {
            Param::DryLevel => 0.0..=1.0,
            // Up to +12 dB so a quiet dry signal can still be brought up; the
            // hard limiter catches anything hotter than its ceiling.
            Param::OutputLevel => 0.0..=4.0,
            Param::DirectDrive => 0.0..=1.0,
            Param::DirectCutoff => 40.0..=8_000.0,
            Param::DirectResonance => 0.0..=1.0,
            Param::DirectEnvelopeSensitivity => 0.0..=1.0,
            Param::DirectLevel => 0.0..=1.0,
            Param::TrackedOsc1Waveshape => 0.0..=1.0,
            Param::TrackedLevel => 0.0..=1.0,
        }
    }

    pub fn default_value(self) -> f32 {
        match self {
            Param::DryLevel => 1.0,
            Param::OutputLevel => 1.0,
            Param::DirectDrive => 0.5,
            Param::DirectCutoff => 400.0,
            Param::DirectResonance => 0.3,
            Param::DirectEnvelopeSensitivity => 0.5,
            // Off until turned up (or a patch sets it), so the dry signal
            // alone is what a fresh engine plays.
            Param::DirectLevel => 0.0,
            Param::TrackedOsc1Waveshape => 0.0,
            // Off until turned up, like the direct layer.
            Param::TrackedLevel => 0.0,
        }
    }

    pub fn clamp(self, value: f32) -> f32 {
        let range = self.range();
        value.clamp(*range.start(), *range.end())
    }
}
