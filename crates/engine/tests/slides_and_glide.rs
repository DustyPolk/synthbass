//! Slides, bends, vibrato and glide (stories 19, 20 and 26), with synthetic
//! tones whose pitch moves continuously.

mod common;

use common::*;
use synthbass_engine::{Engine, Param, Readout};

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

/// Worst distance (cents) between the one voice and the played pitch from
/// `from` seconds on, allowing the voice to lag by up to `lag` seconds.
fn worst_tracking_cents(
    readouts: &[(f32, Readout)],
    pitch: impl Fn(f32) -> f32,
    from: f32,
    lag: f32,
) -> f32 {
    let mut worst = 0.0f32;
    for (t, r) in readouts.iter().filter(|(t, _)| *t >= from) {
        let [voice] = &r.voices[..] else {
            panic!("{t:.3}s: voices {:?}", r.voices);
        };
        let best = (0..=20)
            .map(|i| (voice.pitch - pitch(t - lag * i as f32 / 20.0)).abs())
            .fold(f32::MAX, f32::min);
        worst = worst.max(100.0 * best);
    }
    worst
}

#[test]
fn vibrato_is_tracked_within_the_clip_tolerance() {
    for note in [33.0, 40.0] {
        // ±30 cents at 5.5 Hz, a wide finger vibrato.
        let pitch = |t: f32| note + 0.3 * (std::f32::consts::TAU * 5.5 * t).sin();
        let dry = moving_tone(1.5, pitch);
        let readouts = readouts(&mut engine(), &dry);
        let worst = worst_tracking_cents(&readouts, pitch, 0.3, 0.02);
        assert!(worst <= 25.0, "MIDI {note}: off by {worst:.1} c");
    }
}

/// Plays A1, then D2 picked straight after it (no gap), with `glide`, and
/// returns how long after D2 was detected a voice came within a tenth of
/// the interval (half a semitone) of D2.
fn time_to_reach_d2(glide: f32) -> f32 {
    let (a1, d2) = (33.0, 38.0);
    let mut dry = bass_tone(hz(a1), 0.4, 1.0, 0.5);
    dry.extend(bass_tone(hz(d2), 0.4, 1.0, 0.8));
    let mut engine = engine();
    engine.set_param(Param::TrackedGlide, glide);
    let readouts = readouts(&mut engine, &dry);
    let after = |t: f32| readouts.iter().filter(move |(time, _)| *time >= t);
    let (detected, _) = after(0.5)
        .find(|(_, r)| r.notes.iter().any(|n| (n.pitch - d2).abs() < 0.5))
        .expect("D2 never detected");
    let (reached, _) = after(*detected)
        .find(|(_, r)| r.voices.iter().any(|v| (v.pitch - d2).abs() <= 0.5))
        .expect("no voice reached D2");
    reached - detected
}

#[test]
fn glide_zero_follows_a_new_pitch_at_once() {
    let time = time_to_reach_d2(0.0);
    assert!(time <= 0.005, "{time:.3} s");
}

#[test]
fn longer_glide_reaches_a_new_pitch_proportionally_later() {
    let short = time_to_reach_d2(0.1);
    let long = time_to_reach_d2(0.2);
    // Glide is the time to cover nine tenths of the interval.
    assert!((short - 0.1).abs() <= 0.02, "glide 0.1: {short:.3} s");
    assert!((long - 0.2).abs() <= 0.03, "glide 0.2: {long:.3} s");
}

#[test]
fn notes_picked_together_do_not_glide_into_each_other() {
    // A tenth, E1 + G#2: its voices are the same with a long glide as
    // without.
    let low = bass_tone(hz(28.0), 0.3, 1.0, 0.6);
    let high = bass_tone(hz(44.0), 0.3, 1.0, 0.6);
    let dry: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();
    let voices = |glide: f32| {
        let mut engine = engine();
        engine.set_param(Param::TrackedGlide, glide);
        readouts(&mut engine, &dry)
            .into_iter()
            .map(|(t, r)| (t, r.voices.iter().map(|v| v.pitch).collect::<Vec<f32>>()))
            .collect::<Vec<_>>()
    };
    for ((t, without), (_, with)) in voices(0.0).iter().zip(&voices(0.5)) {
        let same = without.len() == with.len()
            && without.iter().zip(with).all(|(a, b)| (a - b).abs() < 0.01);
        assert!(same, "{t:.3}s: {without:?} vs {with:?}");
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
            || now
                .iter()
                .zip(&last)
                .any(|(a, b)| a.0 != b.0 || (a.1 - b.1).abs() > 0.25);
        if changed {
            let n: Vec<String> = r
                .notes
                .iter()
                .map(|n| format!("#{}:{:.2}", n.id, n.pitch))
                .collect();
            let v: Vec<String> = r
                .voices
                .iter()
                .map(|v| format!("v{}:{:.2}@{:.3}", v.id, v.pitch, v.loudness))
                .collect();
            println!("{t:.3} {} | {}", n.join(" "), v.join(" "));
            last = now;
        }
    }
}

/// The bassist's slides clip: three slides on the E string, each picked
/// once (F#1 → B1, B1 → F#1, A1 → E2).
#[test]
fn each_recorded_slide_plays_as_one_voice_moving_from_start_to_end_note() {
    use common::clip::{clip_path, load_clip, run_clip, score_slides};
    let clip = load_clip(&clip_path("slides"));
    let run = run_clip(&clip, &[128]);
    let tolerance = clip.expected.cents_tolerance;
    let scores = score_slides(&clip, &run);
    assert_eq!(scores.len(), 3);
    let mut failures = Vec::new();
    for s in &scores {
        println!("{s}");
        let near = |c: Option<f32>| c.is_some_and(|c| c.abs() <= tolerance);
        // One voice from start to end; another voice (an attack ghost) only
        // briefly. Fretted slides step from fret to fret: anything bigger is
        // a jump to another note.
        let one_voice = s.carried >= 0.95 && s.extra <= 0.05;
        if !one_voice || !near(s.start_cents) || !near(s.end_cents) || s.largest_step > 1.0 {
            failures.push(s.to_string());
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
