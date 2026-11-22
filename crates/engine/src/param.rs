use std::ops::RangeInclusive;

/// A sound parameter the engine accepts changes to. Levels are linear gains;
/// drive, resonance and envelope sensitivity are 0..=1; cutoff is in Hz;
/// a waveshape is 0 (saw) or 1 (square), and the sub-oscillator's 0 (sine)
/// or 1 (square); an octave is −1, 0 or +1; detune
/// is in cents; a decay or a glide is in seconds.
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
    TrackedOsc2Waveshape,
    TrackedOsc2Octave,
    TrackedOsc2Detune,
    /// Balance of the two oscillators: 0 is oscillator 1 alone, 1 is
    /// oscillator 2 alone.
    TrackedOscMix,
    TrackedSubLevel,
    TrackedSubWaveshape,
    TrackedCutoff,
    TrackedResonance,
    TrackedFilterEnvelopeAmount,
    TrackedFilterEnvelopeDecay,
    TrackedPickSensitivity,
    /// Time a voice takes to cover nine tenths of the way to a new note's
    /// pitch (Moog portamento); 0 jumps at once.
    TrackedGlide,
    TrackedLevel,
}

impl Param {
    /// Every parameter, in declaration order, so `ALL[p.index()] == p`.
    pub const ALL: [Param; 21] = [
        Param::DryLevel,
        Param::OutputLevel,
        Param::DirectDrive,
        Param::DirectCutoff,
        Param::DirectResonance,
        Param::DirectEnvelopeSensitivity,
        Param::DirectLevel,
        Param::TrackedOsc1Waveshape,
        Param::TrackedOsc2Waveshape,
        Param::TrackedOsc2Octave,
        Param::TrackedOsc2Detune,
        Param::TrackedOscMix,
        Param::TrackedSubLevel,
        Param::TrackedSubWaveshape,
        Param::TrackedCutoff,
        Param::TrackedResonance,
        Param::TrackedFilterEnvelopeAmount,
        Param::TrackedFilterEnvelopeDecay,
        Param::TrackedPickSensitivity,
        Param::TrackedGlide,
        Param::TrackedLevel,
    ];

    /// The parameter's place in `ALL`, for indexing per-parameter arrays.
    pub const fn index(self) -> usize {
        self as usize
    }

    /// Everything known about the parameter, defined once here.
    pub const fn info(self) -> ParamInfo {
        use Format::*;
        use Section::*;
        let (section, key, label, format, min, max, default) = match self {
            Param::DryLevel => (Mix, "dry_level", "Dry signal level", Level, 0.0, 1.0, 1.0),
            // Up to +12 dB so a quiet dry signal can still be brought up; the
            // hard limiter catches anything hotter than its ceiling.
            Param::OutputLevel => (Mix, "output_level", "Output level", Level, 0.0, 4.0, 1.0),
            Param::DirectDrive => (Direct, "drive", "Drive", Amount, 0.0, 1.0, 0.5),
            Param::DirectCutoff => (
                Direct,
                "cutoff_hz",
                "Cutoff",
                Frequency,
                40.0,
                8_000.0,
                400.0,
            ),
            Param::DirectResonance => (Direct, "resonance", "Resonance", Amount, 0.0, 1.0, 0.3),
            Param::DirectEnvelopeSensitivity => (
                Direct,
                "envelope_sensitivity",
                "Envelope sensitivity",
                Amount,
                0.0,
                1.0,
                0.1,
            ),
            // Off until turned up (or a patch sets it), so the dry signal
            // alone is what a fresh engine plays.
            Param::DirectLevel => (Direct, "level", "Direct layer level", Level, 0.0, 1.0, 0.0),
            Param::TrackedOsc1Waveshape => (
                Tracked,
                "osc1_waveshape",
                "Oscillator 1",
                Waveshape,
                0.0,
                1.0,
                0.0,
            ),
            Param::TrackedOsc2Waveshape => (
                Tracked,
                "osc2_waveshape",
                "Oscillator 2",
                Waveshape,
                0.0,
                1.0,
                0.0,
            ),
            Param::TrackedOsc2Octave => (
                Tracked,
                "osc2_octave",
                "Oscillator 2 octave",
                Octave,
                -1.0,
                1.0,
                0.0,
            ),
            Param::TrackedOsc2Detune => (
                Tracked,
                "osc2_detune_cents",
                "Oscillator 2 detune",
                Cents,
                -50.0,
                50.0,
                // A slight detune, so the default even blend beats slowly
                // instead of two same-pitch oscillators cancelling at random
                // phase.
                6.0,
            ),
            // An even blend, so oscillator 2's waveshape, octave and detune
            // are heard without first finding this knob.
            Param::TrackedOscMix => (
                Tracked,
                "osc_mix",
                "Oscillator mix (1 → 2)",
                Amount,
                0.0,
                1.0,
                0.5,
            ),
            Param::TrackedSubLevel => (
                Tracked,
                "sub_level",
                "Sub-oscillator level",
                Amount,
                0.0,
                1.0,
                0.0,
            ),
            // A sine unless a patch asks for the buzzier square.
            Param::TrackedSubWaveshape => (
                Tracked,
                "sub_waveshape",
                "Sub-oscillator",
                SubWaveshape,
                0.0,
                1.0,
                0.0,
            ),
            // A plain Moog pluck: a dark filter that the envelope opens.
            Param::TrackedCutoff => (
                Tracked,
                "cutoff_hz",
                "Cutoff",
                Frequency,
                40.0,
                8_000.0,
                300.0,
            ),
            Param::TrackedResonance => (Tracked, "resonance", "Resonance", Amount, 0.0, 1.0, 0.3),
            Param::TrackedFilterEnvelopeAmount => (
                Tracked,
                "filter_envelope_amount",
                "Filter envelope amount",
                Amount,
                0.0,
                1.0,
                0.5,
            ),
            Param::TrackedFilterEnvelopeDecay => (
                Tracked,
                "filter_envelope_decay_s",
                "Filter envelope decay",
                Duration { logarithmic: true },
                0.02,
                2.0,
                0.3,
            ),
            Param::TrackedPickSensitivity => (
                Tracked,
                "pick_sensitivity",
                "Pick sensitivity",
                Amount,
                0.0,
                1.0,
                0.5,
            ),
            Param::TrackedGlide => (
                Tracked,
                "glide_s",
                "Glide",
                Duration { logarithmic: false },
                0.0,
                1.0,
                0.0,
            ),
            // Off until turned up, like the direct layer.
            Param::TrackedLevel => (
                Tracked,
                "level",
                "Tracked layer level",
                Level,
                0.0,
                1.0,
                0.0,
            ),
        };
        ParamInfo {
            section,
            key,
            label,
            format,
            min,
            max,
            default,
        }
    }

    pub fn range(self) -> RangeInclusive<f32> {
        let info = self.info();
        info.min..=info.max
    }

    pub fn default_value(self) -> f32 {
        self.info().default
    }

    /// The value as the window shows it, e.g. "400 Hz" or "−∞ dB".
    pub fn format(self, value: f32) -> String {
        self.info().format.text(value)
    }

    pub fn clamp(self, value: f32) -> f32 {
        let range = self.range();
        value.clamp(*range.start(), *range.end())
    }
}

const _: () = {
    let mut place = 0;
    while place < Param::ALL.len() {
        assert!(
            Param::ALL[place].index() == place,
            "Param::ALL is in declaration order"
        );
        place += 1;
    }
};

/// The facts about one parameter. A value is stored in the patch file under
/// `section.key()` and `key`, and the window shows it under `section.label()`
/// with `label`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamInfo {
    pub section: Section,
    /// The field name in a patch file; it carries its unit where the value
    /// is not 0..=1.
    pub key: &'static str,
    pub label: &'static str,
    pub format: Format,
    pub min: f32,
    pub max: f32,
    pub default: f32,
}

/// Where a parameter belongs: a layer, or the final mix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Section {
    Direct,
    Tracked,
    Mix,
}

impl Section {
    /// In the order the window shows them.
    pub const ALL: [Section; 3] = [Section::Direct, Section::Tracked, Section::Mix];

    /// The table name in a patch file.
    pub const fn key(self) -> &'static str {
        match self {
            Section::Direct => "direct",
            Section::Tracked => "tracked",
            Section::Mix => "mix",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Section::Direct => "Direct",
            Section::Tracked => "Tracked",
            Section::Mix => "Mix",
        }
    }
}

/// How a parameter's value reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// A linear gain shown in dB.
    Level,
    /// Hz, on a logarithmic scale.
    Frequency,
    /// 0..=1 shown as a percentage.
    Amount,
    /// 0 is saw, 1 is square.
    Waveshape,
    /// 0 is sine, 1 is square.
    SubWaveshape,
    /// Whole octaves.
    Octave,
    Cents,
    /// Seconds shown in milliseconds.
    Duration {
        logarithmic: bool,
    },
}

/// Levels at or below this read as silence.
pub const LEVEL_FLOOR_DB: f32 = -60.0;

impl Format {
    pub fn text(self, value: f32) -> String {
        match self {
            Format::Level => {
                let db = gain_to_db(value);
                if db <= LEVEL_FLOOR_DB {
                    "−∞ dB".into()
                } else {
                    format!("{db:+.1} dB")
                }
            }
            Format::Frequency => format!("{value:.0} Hz"),
            Format::Amount => format!("{:.0} %", value * 100.0),
            Format::Waveshape => Waveshape::from_value(value).label().into(),
            Format::SubWaveshape => SubWaveshape::from_value(value).label().into(),
            Format::Octave => Octave::from_value(value).label().into(),
            Format::Cents => format!("{value:+.0} ¢"),
            Format::Duration { .. } => format!("{:.0} ms", value * 1000.0),
        }
    }

    /// Whether a knob for it moves on a logarithmic scale.
    pub const fn logarithmic(self) -> bool {
        matches!(
            self,
            Format::Frequency | Format::Duration { logarithmic: true }
        )
    }
}

/// An oscillator's waveshape. Its parameter value is 0 (saw) or 1 (square);
/// `from_value` is the only reading of that value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Waveshape {
    Saw,
    Square,
}

impl Waveshape {
    /// In switch order.
    pub const ALL: [Waveshape; 2] = [Waveshape::Saw, Waveshape::Square];

    /// The shape a parameter value plays: square from one half up.
    pub fn from_value(value: f32) -> Waveshape {
        if value < 0.5 {
            Waveshape::Saw
        } else {
            Waveshape::Square
        }
    }

    pub const fn value(self) -> f32 {
        match self {
            Waveshape::Saw => 0.0,
            Waveshape::Square => 1.0,
        }
    }

    /// The name in a patch file.
    pub const fn name(self) -> &'static str {
        match self {
            Waveshape::Saw => "saw",
            Waveshape::Square => "square",
        }
    }

    pub fn from_name(name: &str) -> Option<Waveshape> {
        Waveshape::ALL
            .into_iter()
            .find(|shape| shape.name() == name)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Waveshape::Saw => "Saw",
            Waveshape::Square => "Square",
        }
    }
}

/// The sub-oscillator's waveshape. Its parameter value is 0 (sine) or 1
/// (square); `from_value` is the only reading of that value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SubWaveshape {
    Sine,
    Square,
}

impl SubWaveshape {
    /// In switch order.
    pub const ALL: [SubWaveshape; 2] = [SubWaveshape::Sine, SubWaveshape::Square];

    /// The shape a parameter value plays: square from one half up.
    pub fn from_value(value: f32) -> SubWaveshape {
        if value < 0.5 {
            SubWaveshape::Sine
        } else {
            SubWaveshape::Square
        }
    }

    pub const fn value(self) -> f32 {
        match self {
            SubWaveshape::Sine => 0.0,
            SubWaveshape::Square => 1.0,
        }
    }

    /// The name in a patch file.
    pub const fn name(self) -> &'static str {
        match self {
            SubWaveshape::Sine => "sine",
            SubWaveshape::Square => "square",
        }
    }

    pub fn from_name(name: &str) -> Option<SubWaveshape> {
        SubWaveshape::ALL
            .into_iter()
            .find(|shape| shape.name() == name)
    }

    pub const fn label(self) -> &'static str {
        match self {
            SubWaveshape::Sine => "Sine",
            SubWaveshape::Square => "Square",
        }
    }
}

/// Oscillator 2's octave switch. Its parameter value is −1, 0 or +1;
/// `from_value` is the only reading of that value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Octave {
    Down,
    Unison,
    Up,
}

impl Octave {
    /// In switch order.
    pub const ALL: [Octave; 3] = [Octave::Down, Octave::Unison, Octave::Up];

    /// The nearest whole octave to a parameter value.
    pub fn from_value(value: f32) -> Octave {
        match value.round() {
            octave if octave < 0.0 => Octave::Down,
            octave if octave > 0.0 => Octave::Up,
            _ => Octave::Unison,
        }
    }

    pub const fn octaves(self) -> i8 {
        match self {
            Octave::Down => -1,
            Octave::Unison => 0,
            Octave::Up => 1,
        }
    }

    pub const fn value(self) -> f32 {
        self.octaves() as f32
    }

    pub const fn label(self) -> &'static str {
        match self {
            Octave::Down => "−1",
            Octave::Unison => "0",
            Octave::Up => "+1",
        }
    }
}

pub fn gain_to_db(gain: f32) -> f32 {
    20.0 * gain.max(1e-6).log10()
}
