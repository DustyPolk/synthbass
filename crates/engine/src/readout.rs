use crate::tracker::DetectedNotes;

/// A snapshot of what the engine hears, for the window to show and tests to
/// check.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Readout {
    /// Peak level of the dry signal (linear, 1.0 = full scale), falling back
    /// smoothly after the signal stops.
    pub input_level: f32,
    /// The single note being played and how far it is from equal
    /// temperament, or `None` when there is no clear single note.
    pub tuner: Option<TunerReading>,
    /// Every note the tracker hears right now.
    pub notes: DetectedNotes,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TunerReading {
    /// Nearest equal-tempered note as a MIDI note number.
    pub note: u8,
    /// How far sharp (positive) or flat (negative) of `note`, −50 to +50.
    pub cents: f32,
}

const NOTE_NAMES: [&str; 12] = [
    "C", "C♯", "D", "D♯", "E", "F", "F♯", "G", "G♯", "A", "A♯", "B",
];

impl TunerReading {
    /// The note's name without octave, e.g. "E".
    pub fn name(&self) -> &'static str {
        NOTE_NAMES[self.note as usize % 12]
    }

    /// Scientific pitch octave: low E on a bass is E1.
    pub fn octave(&self) -> i32 {
        self.note as i32 / 12 - 1
    }
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
