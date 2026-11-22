//! Recorded test clips and their expected-notes descriptions. The format is
//! documented in `clips/README.md`.

use std::path::{Path, PathBuf};

use synthbass_engine::{Engine, Readout};

use super::Rendered;

/// A mono recording of dry signal and what it should be heard as.
pub struct Clip {
    pub dry: Vec<f32>,
    pub sample_rate: f32,
    pub expected: ExpectedNotes,
}

/// Path of a clip in the repo's `clips/` directory, e.g. `clip_path("open-strings")`.
pub fn clip_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../clips")
        .join(format!("{name}.wav"))
}

/// Loads a mono WAV and the `.notes` description beside it. Panics with the
/// file name on any problem, since a broken clip is a broken test.
pub fn load_clip(wav: &Path) -> Clip {
    let mut reader =
        hound::WavReader::open(wav).unwrap_or_else(|e| panic!("{}: {e}", wav.display()));
    let spec = reader.spec();
    assert_eq!(spec.channels, 1, "{}: clips must be mono", wav.display());
    let dry: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| s as f32 * scale))
                .collect()
        }
    };
    let notes = wav.with_extension("notes");
    let text = std::fs::read_to_string(&notes)
        .unwrap_or_else(|e| panic!("{}: {e}", notes.display()));
    Clip {
        dry: dry.unwrap_or_else(|e| panic!("{}: {e}", wav.display())),
        sample_rate: spec.sample_rate as f32,
        expected: ExpectedNotes::parse(&text)
            .unwrap_or_else(|e| panic!("{}: {e}", notes.display())),
    }
}

/// The engine after a clip, its output, and the readout after every block
/// with the time (seconds) at the end of that block.
pub struct ClipRun {
    pub engine: Engine,
    pub output: Rendered,
    pub readouts: Vec<(f32, Readout)>,
}

/// Runs a clip through a fresh engine at the clip's sample rate, cycling
/// through `block_sizes` for the chunk lengths.
pub fn run_clip(clip: &Clip, block_sizes: &[usize]) -> ClipRun {
    let max_block = block_sizes.iter().copied().max().expect("no block sizes");
    let mut engine = Engine::new(clip.sample_rate, max_block);
    let len = clip.dry.len();
    let mut output = Rendered {
        left: vec![0.0; len],
        right: vec![0.0; len],
    };
    let mut readouts = Vec::new();
    let mut start = 0;
    for &size in block_sizes.iter().cycle() {
        if start == len {
            break;
        }
        let end = (start + size).min(len);
        engine.process(
            &clip.dry[start..end],
            &mut output.left[start..end],
            &mut output.right[start..end],
        );
        readouts.push((end as f32 / clip.sample_rate, engine.readout()));
        start = end;
    }
    ClipRun {
        engine,
        output,
        readouts,
    }
}

/// One expected note.
#[derive(Debug, Clone, PartialEq)]
pub enum Pitch {
    /// Holds one pitch, in Hz.
    Steady(f32),
    /// Moves continuously from one pitch to another over the event.
    Slide { from: f32, to: f32 },
}

/// One stretch of playing: the notes that sound together from `onset` to
/// `offset` (seconds from the start of the clip).
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub onset: f32,
    pub offset: f32,
    pub notes: Vec<Pitch>,
}

/// What a clip should be heard as, with how far detection may stray.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectedNotes {
    pub events: Vec<Event>,
    /// Allowed onset/offset error, in seconds.
    pub timing_tolerance: f32,
    /// Allowed pitch error, in cents.
    pub cents_tolerance: f32,
}

impl ExpectedNotes {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut expected = ExpectedNotes {
            events: Vec::new(),
            timing_tolerance: 0.05,
            cents_tolerance: 20.0,
        };
        for (index, line) in text.lines().enumerate() {
            expected
                .parse_line(line)
                .map_err(|err| format!("line {}: {err}", index + 1))?;
        }
        Ok(expected)
    }

    fn parse_line(&mut self, line: &str) -> Result<(), String> {
        // A comment starts at a field beginning with `#`, so `C#2` is a note.
        let fields: Vec<&str> = line
            .split_whitespace()
            .take_while(|field| !field.starts_with('#'))
            .collect();
        match fields.as_slice() {
            [] => {}
            ["timing_tolerance", seconds] => self.timing_tolerance = number(seconds)?,
            ["cents_tolerance", cents] => self.cents_tolerance = number(cents)?,
            [onset, offset, notes @ ..] if !notes.is_empty() => {
                let (onset, offset) = (number(onset)?, number(offset)?);
                if offset <= onset {
                    return Err(format!("offset {offset} is not after onset {onset}"));
                }
                self.events.push(Event {
                    onset,
                    offset,
                    notes: notes.iter().map(|n| parse_pitch(n)).collect::<Result<_, _>>()?,
                });
            }
            _ => return Err(format!("expected `<onset> <offset> <note>...`, got {line:?}")),
        }
        Ok(())
    }
}

fn number(text: &str) -> Result<f32, String> {
    text.parse().map_err(|_| format!("not a number: {text:?}"))
}

fn parse_pitch(text: &str) -> Result<Pitch, String> {
    match text.split_once("->") {
        Some((from, to)) => Ok(Pitch::Slide {
            from: hz(from)?,
            to: hz(to)?,
        }),
        None => hz(text).map(Pitch::Steady),
    }
}

/// A note name or a frequency such as `98Hz`.
fn hz(text: &str) -> Result<f32, String> {
    match text.strip_suffix("Hz") {
        Some(hz) => number(hz),
        None => note_hz(text),
    }
}

/// Scientific pitch notation with A4 = 440 Hz: `E1`, `F#1`, `Bb0`.
fn note_hz(name: &str) -> Result<f32, String> {
    let mut chars = name.chars();
    let semitone = match chars.next() {
        Some('C') => 0,
        Some('D') => 2,
        Some('E') => 4,
        Some('F') => 5,
        Some('G') => 7,
        Some('A') => 9,
        Some('B') => 11,
        _ => return Err(format!("not a note: {name:?}")),
    };
    let rest = chars.as_str();
    let (accidental, octave) = match rest.strip_prefix('#') {
        Some(octave) => (1, octave),
        None => match rest.strip_prefix('b') {
            Some(octave) => (-1, octave),
            None => (0, rest),
        },
    };
    let octave: i32 = octave
        .parse()
        .map_err(|_| format!("not a note: {name:?}"))?;
    let midi = 12 * (octave + 1) + semitone + accidental;
    Ok(440.0 * 2f32.powf((midi - 69) as f32 / 12.0))
}
