//! Four-voice polyphony against the bassist's recorded clips (ADR 0002).

mod common;

use common::clip::{VoiceScore, clip_path, load_clip, run_clip, score_voices};

fn score(name: &str) -> VoiceScore {
    let clip = load_clip(&clip_path(name));
    let run = run_clip(&clip, &[128]);
    score_voices(&clip, &run)
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
