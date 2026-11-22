mod common;

use std::path::PathBuf;

use common::clip::{ExpectedNotes, Pitch, load_clip, run_clip};
use common::{peak, sine};

fn assert_hz(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 0.01,
        "expected {expected} Hz, got {actual} Hz"
    );
}

#[test]
fn description_names_a_note_with_onset_and_offset() {
    let expected = ExpectedNotes::parse("0.50 2.40 E1\n").unwrap();
    let [event] = expected.events.as_slice() else {
        panic!("expected one event, got {:?}", expected.events);
    };
    assert_eq!(event.onset, 0.5);
    assert_eq!(event.offset, 2.4);
    let [Pitch::Steady(hz)] = event.notes.as_slice() else {
        panic!("expected one steady note, got {:?}", event.notes);
    };
    assert_hz(*hz, 41.203);
}

#[test]
fn description_lists_simultaneous_notes_by_name_or_frequency() {
    let expected = ExpectedNotes::parse("1.0 2.0 C#2 F2 98Hz Bb0\n").unwrap();
    let hz: Vec<f32> = expected.events[0]
        .notes
        .iter()
        .map(|p| match p {
            Pitch::Steady(hz) => *hz,
            other => panic!("expected steady notes, got {other:?}"),
        })
        .collect();
    let want = [69.296, 87.307, 98.0, 29.135];
    assert_eq!(hz.len(), want.len());
    for (a, w) in hz.iter().zip(want) {
        assert_hz(*a, w);
    }
}

#[test]
fn description_marks_a_slide_between_two_pitches() {
    let expected = ExpectedNotes::parse("0.5 2.0 A1->C2\n").unwrap();
    let [Pitch::Slide { from, to }] = expected.events[0].notes.as_slice() else {
        panic!("expected one slide, got {:?}", expected.events[0].notes);
    };
    assert_hz(*from, 55.0);
    assert_hz(*to, 65.406);
}

#[test]
fn description_marks_a_note_left_ringing_from_before() {
    let expected = ExpectedNotes::parse("8.3 10.7 A1 (E1)\n").unwrap();
    let [Pitch::Steady(played), Pitch::Ringing(ringing)] = expected.events[0].notes.as_slice()
    else {
        panic!("expected a played and a ringing note, got {:?}", expected.events[0].notes);
    };
    assert_hz(*played, 55.0);
    assert_hz(*ringing, 41.203);
}

#[test]
fn description_skips_comments_and_blank_lines_and_reads_tolerances() {
    let text = "\
# Open strings, let ring
timing_tolerance 0.1
cents_tolerance 15

0.5 2.0 E1   # low E
2.5 4.0 A1
";
    let expected = ExpectedNotes::parse(text).unwrap();
    assert_eq!(expected.timing_tolerance, 0.1);
    assert_eq!(expected.cents_tolerance, 15.0);
    assert_eq!(expected.events.len(), 2);
}

#[test]
fn description_has_default_tolerances() {
    let expected = ExpectedNotes::parse("0.5 2.0 E1\n").unwrap();
    assert_eq!(expected.timing_tolerance, 0.05);
    assert_eq!(expected.cents_tolerance, 20.0);
}

#[test]
fn description_errors_name_the_line() {
    for bad in [
        "0.5 2.0 E1\n0.5 2.0 H1\n",
        "0.5 2.0 E1\n0.5 oops E1\n",
        "0.5 2.0 E1\n0.5 2.0\n",
        "0.5 2.0 E1\n2.0 0.5 E1\n",
        "0.5 2.0 E1\nloudness 3\n",
    ] {
        let err = ExpectedNotes::parse(bad).unwrap_err();
        assert!(err.starts_with("line 2:"), "{bad:?} gave {err:?}");
    }
}

/// Writes `samples` as a mono float WAV plus its `.notes` description in the
/// test scratch directory, and returns the WAV path.
fn write_clip(name: &str, sample_rate: u32, samples: &[f32], notes: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let wav = dir.join(format!("{name}.wav"));
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
    for &s in samples {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    std::fs::write(wav.with_extension("notes"), notes).unwrap();
    wav
}

#[test]
fn clip_loads_with_its_description_and_runs_through_the_engine_in_chunks() {
    // One second of A1 starting at 0.25 s, at a rate other than the default.
    let sample_rate = 44_100;
    let mut dry = vec![0.0; sample_rate as usize / 4];
    dry.extend(sine(55.0, 0.3, 1.0).into_iter().take(sample_rate as usize));
    let wav = write_clip("synthetic-a1", sample_rate, &dry, "0.25 1.25 A1\n");

    let clip = load_clip(&wav);
    assert_eq!(clip.sample_rate, 44_100.0);
    assert_eq!(clip.dry, dry);
    assert_eq!(clip.expected.events.len(), 1);

    let run = run_clip(&clip, &[100, 37, 512]);
    // The dry signal passes through at default levels, so the output matches
    // the clip sample for sample, which proves every chunk was processed.
    assert_eq!(run.output.left.len(), dry.len());
    for (i, (&out, &d)) in run.output.left.iter().zip(&dry).enumerate() {
        assert!((out - d).abs() < 1e-6, "output differs at sample {i}");
    }
    // One readout per block, timed at the end of the block.
    assert_eq!(run.readouts.first().unwrap().0, 100.0 / 44_100.0);
    let (end, last) = run.readouts.last().unwrap();
    assert!((end - dry.len() as f32 / 44_100.0).abs() < 1e-6);
    assert!(last.input_level > 0.0);
    assert!(peak(&run.output.right) > 0.29);
}

#[test]
fn clip_in_integer_format_loads_scaled_to_plus_minus_one() {
    let wav = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("int24.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48_000,
        bits_per_sample: 24,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(&wav, spec).unwrap();
    for s in [0, 4_194_304, -8_388_608] {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();
    std::fs::write(wav.with_extension("notes"), "").unwrap();

    assert_eq!(load_clip(&wav).dry, [0.0, 0.5, -1.0]);
}

#[test]
fn format_example_in_the_clips_readme_parses() {
    let readme = include_str!("../../../clips/README.md");
    let (_, after) = readme
        .split_once("```text\n")
        .expect("README has a text block");
    let (example, _) = after.split_once("```").unwrap();
    let expected = ExpectedNotes::parse(example).unwrap();
    assert_eq!(expected.events.len(), 4);
    assert_eq!(expected.timing_tolerance, 0.1);
}
