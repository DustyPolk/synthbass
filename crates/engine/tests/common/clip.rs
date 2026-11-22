//! Recorded test clips and their expected-notes descriptions. The format is
//! documented in `clips/README.md`.

use std::path::{Path, PathBuf};

use synthbass_engine::{Engine, NOISE_GATE_DEFAULT, Readout};

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
    let text =
        std::fs::read_to_string(&notes).unwrap_or_else(|e| panic!("{}: {e}", notes.display()));
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
                    notes: notes
                        .iter()
                        .map(|n| parse_pitch(n))
                        .collect::<Result<_, _>>()?,
                });
            }
            _ => {
                return Err(format!(
                    "expected `<onset> <offset> <note>...`, got {line:?}"
                ));
            }
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

/// How well a clip run's voices match the clip's expected notes.
#[derive(Debug)]
pub struct VoiceScore {
    pub events: Vec<EventScore>,
    /// Seconds with a voice sounding while no note is expected.
    pub ghost_seconds: f32,
    /// Where those voices were: (start, end) of each stretch, in seconds.
    pub ghost_spans: Vec<(f32, f32)>,
    /// Seconds with a voice sounding although the dry signal has stayed
    /// below the default noise gate for the last `QUIET_SECONDS`.
    pub lingering_seconds: f32,
}

/// How long the string must have been below the gate before a voice that
/// still sounds counts as lingering.
pub const QUIET_SECONDS: f32 = 0.05;

/// One expected event, judged inside its window (onset and offset each
/// pulled in by the timing tolerance).
#[derive(Debug)]
pub struct EventScore {
    pub onset: f32,
    /// Fraction of the window each expected note has a voice within the
    /// cents tolerance, in the order the event lists them.
    pub coverage: Vec<f32>,
    /// Fraction of the window with a voice that matches no expected note.
    pub extra: f32,
    /// Seconds after the offset (plus tolerance) that a voice at one of the
    /// event's notes still sounds, unless the next event plays it again.
    pub overhang: f32,
}

impl EventScore {
    pub fn min_coverage(&self) -> f32 {
        self.coverage.iter().copied().fold(1.0, f32::min)
    }
}

impl std::fmt::Display for VoiceScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for e in &self.events {
            let coverage: Vec<String> = e.coverage.iter().map(|c| format!("{c:.2}")).collect();
            writeln!(
                f,
                "  {:6.2}s cover [{}] extra {:.2} overhang {:.2}s",
                e.onset,
                coverage.join(" "),
                e.extra,
                e.overhang
            )?;
        }
        let spans: Vec<String> = self
            .ghost_spans
            .iter()
            .map(|(a, b)| format!("{a:.2}-{b:.2}"))
            .collect();
        write!(
            f,
            "  ghost {:.2}s at {}; lingering {:.2}s",
            self.ghost_seconds,
            spans.join(" "),
            self.lingering_seconds
        )
    }
}

fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

/// Scores the voices in `run` against `clip`'s steady notes (slides are
/// ticket 08's and are skipped).
pub fn score_voices(clip: &Clip, run: &ClipRun) -> VoiceScore {
    let expected = &clip.expected;
    let tol = expected.timing_tolerance;
    let near = |pitch: f32, target: f32| (pitch - target).abs() * 100.0 <= expected.cents_tolerance;
    let steady = |event: &Event| -> Vec<f32> {
        event
            .notes
            .iter()
            .filter_map(|p| match p {
                Pitch::Steady(hz) => Some(hz_to_midi(*hz)),
                Pitch::Slide { .. } => None,
            })
            .collect()
    };
    let step = |i: usize| {
        let start = if i == 0 { 0.0 } else { run.readouts[i - 1].0 };
        run.readouts[i].0 - start
    };

    let mut events = Vec::new();
    for (index, event) in expected.events.iter().enumerate() {
        let notes = steady(event);
        if notes.is_empty() {
            continue;
        }
        let (from, to) = (event.onset + tol, event.offset - tol);
        let mut covered = vec![0.0; notes.len()];
        let mut extra = 0.0;
        let mut total = 0.0;
        for (i, (t, readout)) in run.readouts.iter().enumerate() {
            if *t < from || *t > to {
                continue;
            }
            let dt = step(i);
            total += dt;
            for (c, &note) in covered.iter_mut().zip(&notes) {
                if readout.voices.iter().any(|v| near(v.pitch, note)) {
                    *c += dt;
                }
            }
            if readout
                .voices
                .iter()
                .any(|v| !notes.iter().any(|&n| near(v.pitch, n)))
            {
                extra += dt;
            }
        }
        let total = f32::max(total, f32::EPSILON);
        // Notes the next event plays again may keep sounding.
        let next = expected.events.get(index + 1);
        let carried: Vec<f32> = next.map(steady).unwrap_or_default();
        let ending: Vec<f32> = notes
            .iter()
            .copied()
            .filter(|&n| !carried.iter().any(|&c| near(c, n)))
            .collect();
        let end = event.offset + tol;
        let limit = next.map_or(f32::MAX, |n| n.onset - tol).max(end);
        let last_sounding = run
            .readouts
            .iter()
            .filter(|(t, _)| *t > end && *t <= limit)
            .filter(|(_, r)| {
                r.voices
                    .iter()
                    .any(|v| ending.iter().any(|&n| near(v.pitch, n)))
            })
            .map(|(t, _)| *t)
            .fold(end, f32::max);
        events.push(EventScore {
            onset: event.onset,
            coverage: covered.iter().map(|c| c / total).collect(),
            extra: extra / total,
            overhang: last_sounding - end,
        });
    }

    let expected_at = |t: f32| {
        expected
            .events
            .iter()
            .any(|e| t >= e.onset - tol && t <= e.offset + tol)
    };
    let ghostly = |i: usize| {
        let (t, readout) = &run.readouts[i];
        !expected_at(*t) && !readout.voices.is_empty()
    };
    let ghost_seconds = (0..run.readouts.len())
        .filter(|&i| ghostly(i))
        .map(step)
        .sum();
    let mut ghost_spans: Vec<(f32, f32)> = Vec::new();
    for i in (0..run.readouts.len()).filter(|&i| ghostly(i)) {
        let t = run.readouts[i].0;
        match ghost_spans.last_mut() {
            Some((_, end)) if t - *end <= 2.0 * step(i) => *end = t,
            _ => ghost_spans.push((t - step(i), t)),
        }
    }
    let quiet = |t: f32| {
        let end = ((t * clip.sample_rate) as usize).min(clip.dry.len());
        let start = end.saturating_sub((QUIET_SECONDS * clip.sample_rate) as usize);
        clip.dry[start..end]
            .iter()
            .all(|s| s.abs() < NOISE_GATE_DEFAULT)
    };
    let lingering_seconds = (0..run.readouts.len())
        .filter(|&i| {
            let (t, readout) = &run.readouts[i];
            !readout.voices.is_empty() && quiet(*t)
        })
        .map(step)
        .sum();
    VoiceScore {
        events,
        ghost_seconds,
        ghost_spans,
        lingering_seconds,
    }
}

/// How one slide event came out, judged inside its window (onset and offset
/// each pulled in by the timing tolerance).
#[derive(Debug)]
pub struct SlideScore {
    pub onset: f32,
    /// Share of the window that the voice sounding at its end (the slide's
    /// voice) sounds: 1 when one voice carries the whole slide, less when it
    /// started partway (a restart).
    pub carried: f32,
    /// Share of the window with any other voice sounding.
    pub extra: f32,
    /// Cents from the start note over the window's first tenth of a second,
    /// and from the arrival note over its last (median of the slide's voice;
    /// `None` when it isn't sounding there).
    pub start_cents: Option<f32>,
    pub end_cents: Option<f32>,
    /// Largest pitch change (semitones) of the slide's voice between
    /// consecutive readouts.
    pub largest_step: f32,
}

impl std::fmt::Display for SlideScore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let cents = |c: Option<f32>| c.map_or("-".into(), |c| format!("{c:+.0}c"));
        write!(
            f,
            "  {:6.2}s carried {:.2} extra {:.2} start {} end {} largest step {:.2} st",
            self.onset,
            self.carried,
            self.extra,
            cents(self.start_cents),
            cents(self.end_cents),
            self.largest_step
        )
    }
}

/// Scores each slide in `clip` against the voices in `run`.
pub fn score_slides(clip: &Clip, run: &ClipRun) -> Vec<SlideScore> {
    const EDGE_SECONDS: f32 = 0.1;
    let tol = clip.expected.timing_tolerance;
    let mut scores = Vec::new();
    for event in &clip.expected.events {
        let [Pitch::Slide { from, to }] = event.notes[..] else {
            continue;
        };
        let (start, end) = (event.onset + tol, event.offset - tol);
        let window: Vec<(f32, &Readout)> = run
            .readouts
            .iter()
            .filter(|(t, _)| *t >= start && *t <= end)
            .map(|(t, r)| (*t, r))
            .collect();
        let total = window.len().max(1) as f32;
        let carrier = window.last().and_then(|(_, r)| {
            r.voices
                .iter()
                .max_by(|a, b| a.loudness.total_cmp(&b.loudness))
                .map(|v| v.id)
        });
        let path: Vec<(f32, f32)> = window
            .iter()
            .filter_map(|(t, r)| {
                r.voices
                    .iter()
                    .find(|v| Some(v.id) == carrier)
                    .map(|v| (*t, v.pitch))
            })
            .collect();
        let extra = window
            .iter()
            .filter(|(_, r)| r.voices.iter().any(|v| Some(v.id) != carrier))
            .count() as f32;
        let median_cents = |from: f32, until: f32, target: f32| {
            let mut pitches: Vec<f32> = path
                .iter()
                .filter(|(t, _)| *t >= from && *t <= until)
                .map(|(_, p)| *p)
                .collect();
            pitches.sort_by(f32::total_cmp);
            (!pitches.is_empty())
                .then(|| 100.0 * (pitches[pitches.len() / 2] - hz_to_midi(target)))
        };
        scores.push(SlideScore {
            onset: event.onset,
            carried: path.len() as f32 / total,
            extra: extra / total,
            start_cents: median_cents(start, start + EDGE_SECONDS, from),
            end_cents: median_cents(end - EDGE_SECONDS, end, to),
            largest_step: path
                .windows(2)
                .map(|w| (w[1].1 - w[0].1).abs())
                .fold(0.0, f32::max),
        });
    }
    scores
}
