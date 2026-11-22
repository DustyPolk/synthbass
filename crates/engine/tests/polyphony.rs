//! Four-voice polyphony (ADR 0002), driven through the engine's public
//! interface with synthetic bass-like tones.

mod common;

use common::*;
use synthbass_engine::{Engine, Param};

/// Sums tones that start together.
fn chord(tones: &[Vec<f32>]) -> Vec<f32> {
    let len = tones.iter().map(Vec::len).max().unwrap_or(0);
    (0..len)
        .map(|n| tones.iter().map(|t| t.get(n).copied().unwrap_or(0.0)).sum())
        .collect()
}

/// The sounding voices' pitches, low to high.
fn voice_pitches(engine: &Engine) -> Vec<f32> {
    let mut pitches: Vec<f32> = engine.readout().voices.iter().map(|v| v.pitch).collect();
    pitches.sort_by(f32::total_cmp);
    pitches
}

fn sorted(mut pitches: Vec<f32>) -> Vec<f32> {
    pitches.sort_by(f32::total_cmp);
    pitches
}

fn assert_pitches(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "voices at {actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!(
            (a - e).abs() < 0.1,
            "voices at {actual:?}, expected {expected:?}"
        );
    }
}

#[test]
fn a_tenth_plays_as_two_voices_at_their_pitches() {
    let mut engine = engine();
    let tenth = chord(&[
        bass_tone(hz(28.0), 0.3, 1.0, 0.5),
        bass_tone(hz(44.0), 0.3, 1.0, 0.5),
    ]);
    render(&mut engine, &tenth, &[128]);
    assert_pitches(&voice_pitches(&engine), &[28.0, 44.0]);
}

/// Five notes whose partials stay clear of each other's bands (E1 under a
/// whole-tone cluster), the third quietest by far.
const FIVE: [(f32, f32); 5] = [
    (28.0, 0.3),
    (61.0, 0.25),
    (63.0, 0.06),
    (65.0, 0.22),
    (67.0, 0.2),
];

#[test]
fn five_simultaneous_tones_give_four_voices_without_the_quietest() {
    let mut engine = engine();
    let tones: Vec<Vec<f32>> = FIVE
        .iter()
        .map(|&(note, amplitude)| bass_tone(hz(note), amplitude, 1.0, 0.6))
        .collect();
    let dry = chord(&tones);
    let mut most = 0;
    for block in dry.chunks(128) {
        render(&mut engine, block, &[128]);
        most = most.max(engine.readout().voices.len());
    }
    assert!(most <= 4, "{most} voices at once");
    assert_pitches(&voice_pitches(&engine), &[28.0, 61.0, 65.0, 67.0]);
}

/// Runs `dry` through `engine` in 64-sample blocks and returns each block's
/// end time and voices as (id, pitch).
fn voice_trace(engine: &mut Engine, dry: &[f32]) -> Vec<(f32, Vec<(u32, f32)>)> {
    dry.chunks(64)
        .enumerate()
        .map(|(i, block)| {
            render(engine, block, &[64]);
            let voices = engine
                .readout()
                .voices
                .iter()
                .map(|v| (v.id, v.pitch))
                .collect();
            ((i * 64 + block.len()) as f32 / SAMPLE_RATE, voices)
        })
        .collect()
}

/// `tone` silent for its first `start` seconds.
fn delayed(start: f32, tone: Vec<f32>) -> Vec<f32> {
    let mut delayed = vec![0.0; (start * SAMPLE_RATE) as usize];
    delayed.extend(tone);
    delayed
}

/// E1 alone, quiet; then three louder notes clear of its bands; then, with
/// four voices sounding, a loud fifth note at 0.9 s.
fn fifth_note_over_four(fifth: f32, amplitude: f32, glide: f32) -> Vec<(f32, Vec<(u32, f32)>)> {
    let mut engine = engine();
    engine.set_param(Param::TrackedGlide, glide);
    let dry = chord(&[
        bass_tone(hz(28.0), 0.12, 1.0, 1.5),
        delayed(0.3, bass_tone(hz(61.0), 0.25, 1.0, 1.2)),
        delayed(0.3, bass_tone(hz(65.0), 0.22, 1.0, 1.2)),
        delayed(0.3, bass_tone(hz(67.0), 0.2, 1.0, 1.2)),
        delayed(0.9, bass_tone(hz(fifth), amplitude, 1.0, 0.6)),
    ]);
    voice_trace(&mut engine, &dry)
}

/// The voices just before `time`.
fn before_the(time: f32, trace: &[(f32, Vec<(u32, f32)>)]) -> &[(u32, f32)] {
    &trace
        .iter()
        .rfind(|(t, _)| *t < time)
        .expect("no readouts")
        .1
}

#[test]
fn a_loud_fifth_note_takes_the_quietest_voice() {
    let trace = fifth_note_over_four(63.0, 0.3, 0.0);
    let before: Vec<f32> = before_the(0.9, &trace).iter().map(|v| v.1).collect();
    assert_pitches(&sorted(before), &[28.0, 61.0, 65.0, 67.0]);
    let (_, after) = trace.last().expect("no readouts");
    let after: Vec<f32> = after.iter().map(|v| v.1).collect();
    assert_pitches(&sorted(after), &[61.0, 63.0, 65.0, 67.0]);
}

#[test]
fn a_stolen_voice_glides_from_the_newest_voice_not_its_own_old_note() {
    let trace = fifth_note_over_four(63.0, 0.3, 0.1);
    let before = before_the(0.9, &trace);
    let &(quiet, _) = before
        .iter()
        .find(|v| (v.1 - 28.0).abs() < 0.1)
        .expect("no E1 voice");
    let &(_, newest) = before.iter().max_by_key(|v| v.0).expect("no voices");
    // The E1 voice takes the fifth note: where does its glide start?
    let stolen = |voices: &Vec<(u32, f32)>| voices.iter().find(|v| v.0 == quiet).map(|v| v.1);
    let (t, start) = trace
        .iter()
        .find_map(|(t, voices)| {
            stolen(voices)
                .filter(|pitch| (pitch - 28.0).abs() > 1.0)
                .map(|pitch| (t, pitch))
        })
        .expect("the E1 voice never changed note");
    assert!(
        (start - newest).abs() < 0.5,
        "{t:.3}s: the stolen voice starts its glide at {start:.2}, the newest voice plays {newest:.2}"
    );
    let end = stolen(&trace.last().expect("no readouts").1);
    assert!(
        end.is_some_and(|p| (p - 63.0).abs() < 0.1),
        "ends at {end:?}"
    );
}

#[test]
fn a_fifth_note_about_as_loud_as_the_quietest_does_not_trade_voices_with_it() {
    // The fifth note's level comes and goes around the E1 voice's: without
    // a margin the two would swap the voice back and forth.
    let trace = fifth_note_over_four(63.0, 0.06, 0.0);
    for (t, voices) in trace.iter().filter(|(t, _)| *t >= 0.85) {
        let pitches: Vec<f32> = voices.iter().map(|v| v.1).collect();
        assert_eq!(
            sorted(pitches.iter().map(|p| p.round()).collect()),
            [28.0, 61.0, 65.0, 67.0],
            "{t:.3}s: {pitches:?}"
        );
    }
}

#[test]
fn a_new_note_leaves_the_last_ones_voice_to_fade_out() {
    // A1 muted, then D2 picked a moment later: the A1 voice is still fading
    // when D2 arrives, and D2 takes a silent voice instead of cutting it
    // short. Several gaps, since which update finds the A1 voice fading
    // depends on the timing.
    let (a1, d2) = (33.0, 38.0);
    for gap in [0.0, 0.01, 0.02, 0.03, 0.04, 0.05] {
        let mut dry = bass_tone(hz(a1), 0.4, 1.0, 0.5);
        let fade = (0.005 * SAMPLE_RATE) as usize;
        let len = dry.len();
        for (i, s) in dry[len - fade..].iter_mut().enumerate() {
            *s *= 1.0 - i as f32 / fade as f32;
        }
        dry.extend(std::iter::repeat_n(0.0, (gap * SAMPLE_RATE) as usize));
        dry.extend(bass_tone(hz(d2), 0.4, 1.0, 0.5));
        let trace = voice_trace(&mut engine(), &dry);
        let before = before_the(0.5, &trace);
        let [(a1_voice, _)] = before[..] else {
            panic!("gap {gap}: voices before D2: {before:?}");
        };
        let mut overlapped = false;
        for (t, voices) in &trace {
            for &(id, pitch) in voices {
                assert!(
                    id != a1_voice || (pitch - a1).abs() < 1.0,
                    "gap {gap}, {t:.3}s: the A1 voice plays {pitch:.2}"
                );
            }
            overlapped |= voices.len() == 2;
        }
        assert!(overlapped, "gap {gap}: the A1 voice never overlapped D2's");
    }
}
