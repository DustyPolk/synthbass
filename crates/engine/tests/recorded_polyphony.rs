//! Four-voice polyphony against the bassist's recorded clips (ADR 0002).

mod common;

use common::clip::{VoiceScore, clip_path, load_clip, run_clip, score_tuner, score_voices};

fn score(name: &str) -> VoiceScore {
    let clip = load_clip(&clip_path(name));
    let run = run_clip(&clip, &[128]);
    score_voices(&clip, &run)
}

/// What a clip's voices must achieve, per event.
struct Bar {
    /// Each expected note has a voice within the cents tolerance for at
    /// least this share of its window.
    coverage: f32,
    /// A voice matching no expected note sounds for at most this share.
    extra: f32,
    /// Seconds a voice may sound, over the whole clip, after the string
    /// has fallen below the noise gate. The drafted offsets mark where a
    /// mute starts, and strings often ring on above the gate after them, so
    /// voice endings are judged against the signal rather than the offsets.
    lingering: f32,
}

const SINGLE_NOTES: Bar = Bar {
    coverage: 0.85,
    extra: 0.1,
    lingering: 0.05,
};

/// Low thirds need about 100 ms to separate; the windows already start
/// 150 ms in, so the bar for coverage is a little lower than for single notes.
const DOUBLE_STOPS: Bar = Bar {
    coverage: 0.8,
    extra: 0.2,
    lingering: 0.05,
};

/// Events (by onset) that miss the bar today, each with the reason. They are
/// the follow-up ticket's work (`.scratch/synthbass/issues/12-polyphony-gaps.md`);
/// every other event must pass.
type KnownGaps<'a> = &'a [(f32, &'a str)];

fn check(name: &str, bar: Bar, known: KnownGaps) {
    let score = score(name);
    let mut failures = Vec::new();
    let exempt = |onset: f32| known.iter().any(|(k, _)| (k - onset).abs() < 0.005);
    for e in score.events.iter().filter(|e| !exempt(e.onset)) {
        if e.min_coverage() < bar.coverage {
            failures.push(format!("{:.2}s coverage {:?}", e.onset, e.coverage));
        }
        if e.extra > bar.extra {
            failures.push(format!("{:.2}s extra {:.2}", e.onset, e.extra));
        }
    }
    if score.lingering_seconds > bar.lingering {
        failures.push(format!("lingering {:.2}s", score.lingering_seconds));
    }
    assert!(failures.is_empty(), "{name}: {}", failures.join("; "));
}

#[test]
fn open_strings_play_one_voice_each_at_their_pitch() {
    check("open-strings", SINGLE_NOTES, &[]);
}

#[test]
fn a_chromatic_run_plays_one_voice_per_note() {
    check("chromatic-run", SINGLE_NOTES, &[]);
}

/// The tuner names each event's note within the clip's cents tolerance for
/// the same share of its window that a voice must cover it, and shows any
/// other reading no longer than a voice may sound off the notes. Where no
/// clear single note is expected (a string ringing on), it may stay blank.
fn check_tuner(name: &str, bar: Bar) {
    let clip = load_clip(&clip_path(name));
    let run = run_clip(&clip, &[128]);
    let mut failures = Vec::new();
    for s in score_tuner(&clip, &run) {
        if (s.expected.is_some() && s.on_note < bar.coverage) || s.wrong > bar.extra {
            failures.push(s.to_string());
        }
    }
    assert!(failures.is_empty(), "{name}:\n{}", failures.join("\n"));
}

#[test]
fn the_tuner_reads_each_open_string() {
    check_tuner("open-strings", SINGLE_NOTES);
}

#[test]
fn the_tuner_reads_each_note_of_a_chromatic_run() {
    check_tuner("chromatic-run", SINGLE_NOTES);
}

#[test]
fn palm_mutes_end_their_voices_promptly() {
    check("palm-mutes", SINGLE_NOTES, &[]);
}

#[test]
fn double_stop_thirds_play_as_two_voices() {
    check(
        "double-stop-thirds",
        DOUBLE_STOPS,
        &[(
            15.65,
            "F2 falls under the noise gate at 21.2 s, 5 s before the event ends",
        )],
    );
}

#[test]
fn double_stop_tenths_play_as_two_voices() {
    check("double-stop-tenths", DOUBLE_STOPS, &[]);
}

/// Times (seconds) at which a note the tracker already reports counts a
/// repick, away from any onset in the clip's notes: beating between the
/// strings of a held chord read as picks, retriggering the filter envelope.
fn misfired_repicks(name: &str) -> Vec<f32> {
    let clip = load_clip(&clip_path(name));
    let run = run_clip(&clip, &[128]);
    let tol = clip.expected.timing_tolerance;
    let near_onset = |t: f32| {
        clip.expected
            .events
            .iter()
            .any(|e| t > e.onset - tol && t < e.onset + tol)
    };
    let mut seen = std::collections::HashMap::<u32, u32>::new();
    let mut misfires = Vec::new();
    for (t, readout) in &run.readouts {
        for n in readout.notes.iter() {
            let before = seen.insert(n.id, n.repicks).unwrap_or(0);
            if n.repicks > before && !near_onset(*t) {
                misfires.push(*t);
            }
        }
    }
    misfires
}

#[test]
fn held_double_stops_are_not_picked_again() {
    for name in [
        "double-stop-thirds",
        "double-stop-tenths",
        "double-stop-sevenths",
    ] {
        let misfires = misfired_repicks(name);
        assert!(
            misfires.is_empty(),
            "{name}: {} repicks away from onsets: {misfires:.2?}",
            misfires.len()
        );
    }
}

#[test]
#[ignore]
fn print_scores() {
    for name in [
        "open-strings",
        "chromatic-run",
        "double-stop-thirds",
        "double-stop-tenths",
        "double-stop-sevenths",
        "palm-mutes",
    ] {
        println!("{name}\n{}", score(name));
        let clip = load_clip(&clip_path(name));
        for s in score_tuner(&clip, &run_clip(&clip, &[128])) {
            println!("{s}");
        }
    }
}

/// Per event: which semitones the voices sit on (and the note list), as a
/// share of the event's window.
#[test]
#[ignore]
fn print_voice_histograms() {
    let names = std::env::var("CLIPS").unwrap_or("double-stop-thirds".into());
    for name in names.split(',') {
        let clip = load_clip(&clip_path(name));
        let run = run_clip(&clip, &[128]);
        println!("{name}");
        for e in &clip.expected.events {
            let mut hist = std::collections::BTreeMap::<i32, (f32, f32)>::new();
            let mut n = 0.0;
            for (t, r) in &run.readouts {
                if *t < e.onset || *t > e.offset {
                    continue;
                }
                n += 1.0;
                for v in r.voices.iter() {
                    let k = v.pitch.round() as i32;
                    let entry = hist.entry(k).or_default();
                    entry.0 += 1.0;
                    entry.1 += (v.pitch - k as f32) * 100.0;
                }
            }
            let s: Vec<String> = hist
                .iter()
                .map(|(k, (c, cents))| format!("{k}:{:.0}%({:+.0}c)", 100.0 * c / n, cents / c))
                .collect();
            println!("  {:6.2} {:?} -> {}", e.onset, e.notes.len(), s.join(" "));
        }
    }
}

/// The voices whenever their set changes, for `DUMP=clip:t0:t1`.
#[test]
#[ignore]
fn print_voices() {
    let spec = std::env::var("DUMP").unwrap_or("palm-mutes:12.84:13.24".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let (t0, t1): (f32, f32) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
    let clip = load_clip(&clip_path(parts[0]));
    let run = run_clip(&clip, &[128]);
    let mut last = String::new();
    for (t, r) in run.readouts.iter().filter(|(t, _)| *t >= t0 && *t <= t1) {
        let key: Vec<String> = r
            .voices
            .iter()
            .map(|v| format!("{:.2}#{}", v.pitch, v.id))
            .collect();
        let key = key.join(" ");
        let rounded: String = r
            .voices
            .iter()
            .map(|v| format!("{}#{} ", v.pitch.round(), v.id))
            .collect();
        if rounded != last {
            println!("{t:.3} => {key}");
            last = rounded;
        }
    }
}
