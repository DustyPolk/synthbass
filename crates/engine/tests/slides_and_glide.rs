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

/// Plays A1, mutes it, waits `gap` seconds of silence and picks D2, with
/// `glide`. Returns how long after D2 was detected a voice came within a
/// tenth of the interval (half a semitone) of D2.
fn time_to_reach_d2(glide: f32, gap: f32) -> f32 {
    let (a1, d2) = (33.0, 38.0);
    let mut dry = bass_tone(hz(a1), 0.4, 1.0, 0.5);
    if gap > 0.0 {
        // A 5 ms fade, like a palm or fretting-hand mute.
        let fade = (0.005 * SAMPLE_RATE) as usize;
        let len = dry.len();
        for (i, s) in dry[len - fade..].iter_mut().enumerate() {
            *s *= 1.0 - i as f32 / fade as f32;
        }
        dry.extend(std::iter::repeat_n(0.0, (gap * SAMPLE_RATE) as usize));
    }
    let picked = dry.len() as f32 / SAMPLE_RATE;
    dry.extend(bass_tone(hz(d2), 0.4, 1.0, 0.8));
    let mut engine = engine();
    engine.set_param(Param::TrackedGlide, glide);
    let readouts = readouts(&mut engine, &dry);
    let after = |t: f32| readouts.iter().filter(move |(time, _)| *time >= t);
    let (detected, _) = after(picked)
        .find(|(_, r)| r.notes.iter().any(|n| (n.pitch - d2).abs() < 0.5))
        .expect("D2 never detected");
    let (reached, _) = after(*detected)
        .find(|(_, r)| r.voices.iter().any(|v| (v.pitch - d2).abs() <= 0.5))
        .expect("no voice reached D2");
    reached - detected
}

#[test]
fn glide_zero_follows_a_new_pitch_at_once() {
    let time = time_to_reach_d2(0.0, 0.0);
    assert!(time <= 0.005, "{time:.3} s");
}

#[test]
fn longer_glide_reaches_a_new_pitch_proportionally_later() {
    let short = time_to_reach_d2(0.1, 0.0);
    let long = time_to_reach_d2(0.2, 0.0);
    // Glide is the time to cover nine tenths of the interval.
    assert!((short - 0.1).abs() <= 0.02, "glide 0.1: {short:.3} s");
    assert!((long - 0.2).abs() <= 0.03, "glide 0.2: {long:.3} s");
}

#[test]
fn a_note_shortly_after_the_last_one_ended_glides_from_it() {
    // A mono Moog's portamento: the gap since the last note doesn't stop
    // the glide, as long as it is short.
    let time = time_to_reach_d2(0.1, 0.1);
    assert!((time - 0.1).abs() <= 0.02, "{time:.3} s");
}

#[test]
fn a_note_after_a_long_gap_starts_on_pitch() {
    // A new phrase: the last note ended over half a second ago.
    let time = time_to_reach_d2(0.2, 0.6);
    assert!(time <= 0.005, "{time:.3} s");
}

/// Mixes `tone` into `dry` from `start` seconds, ending it at `end` seconds
/// with a 5 ms mute.
fn add_muted(dry: &mut Vec<f32>, tone: Vec<f32>, start: f32, end: f32) {
    let (start, end) = ((start * SAMPLE_RATE) as usize, (end * SAMPLE_RATE) as usize);
    let fade = (0.005 * SAMPLE_RATE) as usize;
    if dry.len() < end {
        dry.resize(end, 0.0);
    }
    for (i, (s, t)) in dry[start..end].iter_mut().zip(tone).enumerate() {
        let left = end - start - i;
        *s += t * (left.min(fade) as f32 / fade as f32);
    }
}

#[test]
fn a_note_after_several_end_together_glides_from_the_newest_of_them() {
    // A2 sounds alone, then with D2 picked under it; A2 ends, and E2 is picked
    // over the ringing D2. Both are muted together: the next note glides
    // from E2, the note played last, not D2.
    let (g1, d2, e2, a2) = (31.0, 38.0, 40.0, 45.0);
    let mut dry = Vec::new();
    add_muted(&mut dry, bass_tone(hz(a2), 0.3, 1.0, 1.0), 0.0, 0.6);
    add_muted(&mut dry, bass_tone(hz(d2), 0.3, 1.0, 2.0), 0.2, 1.5);
    add_muted(&mut dry, bass_tone(hz(e2), 0.3, 1.0, 1.0), 1.0, 1.5);
    add_muted(&mut dry, bass_tone(hz(g1), 0.3, 1.0, 0.5), 1.7, 2.2);
    let mut engine = engine();
    engine.set_param(Param::TrackedGlide, 0.2);
    let readouts = readouts(&mut engine, &dry);
    let before: Vec<u32> = readouts
        .iter()
        .filter(|(t, _)| *t < 1.7)
        .flat_map(|(_, r)| r.voices.iter().map(|v| v.id))
        .collect();
    let (t, first) = readouts
        .iter()
        .filter(|(t, _)| *t >= 1.7)
        .find_map(|(t, r)| {
            r.voices
                .iter()
                .find(|v| !before.contains(&v.id))
                .map(|v| (t, v.pitch))
        })
        .expect("G1 gets a voice");
    assert!(
        (first - e2).abs() < 1.0,
        "{t:.3} s: G1's voice started at {first:.2}"
    );
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
    use common::clip::{BLOCK_PATTERNS, clip_path, load_clip, run_clip, score_slides};
    let clip = load_clip(&clip_path("slides"));
    let tolerance = clip.expected.cents_tolerance;
    let mut failures = Vec::new();
    for blocks in BLOCK_PATTERNS {
        let scores = score_slides(&clip, &run_clip(&clip, blocks));
        assert_eq!(scores.len(), 3);
        for s in &scores {
            println!("blocks {blocks:?}{s}");
            let near = |c: Option<f32>| c.is_some_and(|c| c.abs() <= tolerance);
            // One voice from start to end; another voice (an attack ghost)
            // only briefly. Fretted slides step from fret to fret: anything
            // bigger is a jump to another note.
            let one_voice = s.carried >= 0.95 && s.extra <= 0.05;
            if !one_voice || !near(s.start_cents) || !near(s.end_cents) || s.largest_step > 1.0 {
                failures.push(format!("blocks {blocks:?}{s}"));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
