//! Slides, bends, vibrato and glide (stories 19, 20 and 26), with synthetic
//! tones whose pitch moves continuously.

mod common;

use common::*;
use synthbass_engine::{Engine, Readout};

/// A plucked tone (eight harmonics at 1/h, decaying) whose pitch follows
/// `pitch(t)` (fractional MIDI) continuously, phase and all.
fn moving_tone(seconds: f32, pitch: impl Fn(f32) -> f32) -> Vec<f32> {
    let len = (seconds * SAMPLE_RATE) as usize;
    let mut phase = 0.0f32;
    (0..len)
        .map(|n| {
            let t = n as f32 / SAMPLE_RATE;
            phase = (phase + hz(pitch(t)) / SAMPLE_RATE).fract();
            let tone: f32 = (1..=8)
                .map(|h| (std::f32::consts::TAU * phase * h as f32).sin() / h as f32)
                .sum();
            0.25 * tone * (-t / 1.5).exp()
        })
        .collect()
}

/// Pitch held at `from` until `start`, then a straight line to `to` over
/// `length` seconds, then held.
fn slide(from: f32, to: f32, start: f32, length: f32) -> impl Fn(f32) -> f32 {
    move |t| from + (to - from) * ((t - start) / length).clamp(0.0, 1.0)
}

/// Runs `dry` through a fresh engine in 64-sample blocks and returns the
/// readout after each block with the time at its end.
fn readouts(engine: &mut Engine, dry: &[f32]) -> Vec<(f32, Readout)> {
    dry.chunks(64)
        .enumerate()
        .map(|(i, chunk)| {
            let mut left = vec![0.0; chunk.len()];
            let mut right = vec![0.0; chunk.len()];
            engine.process(chunk, &mut left, &mut right);
            let t = ((i * 64 + chunk.len()) as f32) / SAMPLE_RATE;
            (t, engine.readout())
        })
        .collect()
}

/// Distinct voice ids seen from `from` seconds on.
fn voice_ids(readouts: &[(f32, Readout)], from: f32) -> Vec<u32> {
    let mut ids: Vec<u32> = readouts
        .iter()
        .filter(|(t, _)| *t >= from)
        .flat_map(|(_, r)| r.voices.iter().map(|v| v.id))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[test]
fn a_slide_across_several_semitones_keeps_one_voice() {
    // A1 to E2 (5 semitones up) in 0.3 s, like a fret 5 → 10 slide.
    let pitch = slide(33.0, 38.0, 0.5, 0.3);
    let dry = moving_tone(1.5, &pitch);
    let mut engine = engine();
    let readouts = readouts(&mut engine, &dry);
    let ids = voice_ids(&readouts, 0.2);
    assert_eq!(ids.len(), 1, "voices {ids:?}");
    for (t, r) in readouts.iter().filter(|(t, _)| *t >= 0.2) {
        assert_eq!(r.voices.len(), 1, "{t:.3}s: {:?}", r.voices);
    }
}

/// `TRACE=clip:t0:t1`: prints the notes and voices whenever a voice or
/// note starts or ends, or a voice moves by a quarter semitone.
#[test]
#[ignore]
fn trace_clip() {
    let spec = std::env::var("TRACE").unwrap_or("slides:5.0:6.0".into());
    let parts: Vec<&str> = spec.split(':').collect();
    let (t0, t1): (f32, f32) = (parts[1].parse().unwrap(), parts[2].parse().unwrap());
    let clip = common::clip::load_clip(&common::clip::clip_path(parts[0]));
    let run = common::clip::run_clip(&clip, &[64]);
    let mut last: Vec<(u32, f32)> = Vec::new();
    for (t, r) in run.readouts.iter().filter(|(t, _)| *t >= t0 && *t <= t1) {
        let now: Vec<(u32, f32)> = r.voices.iter().map(|v| (v.id, v.pitch)).collect();
        let changed = now.len() != last.len()
            || now.iter().zip(&last).any(|(a, b)| a.0 != b.0 || (a.1 - b.1).abs() > 0.25);
        if changed {
            let n: Vec<String> = r.notes.iter().map(|n| format!("#{}:{:.2}", n.id, n.pitch)).collect();
            let v: Vec<String> = r.voices.iter().map(|v| format!("v{}:{:.2}@{:.3}", v.id, v.pitch, v.loudness)).collect();
            println!("{t:.3} {} | {}", n.join(" "), v.join(" "));
            last = now;
        }
    }
}
