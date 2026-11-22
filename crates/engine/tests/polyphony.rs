//! Four-voice polyphony (ADR 0002), driven through the engine's public
//! interface with synthetic bass-like tones.

mod common;

use common::*;
use synthbass_engine::Engine;

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

fn assert_pitches(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len(), "voices at {actual:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < 0.1, "voices at {actual:?}, expected {expected:?}");
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

#[test]
fn five_notes_play_as_four_voices_dropping_the_quietest() {
    let mut engine = engine();
    let tones: Vec<Vec<f32>> = [
        (28.0, 0.3),
        (61.0, 0.25),
        (63.0, 0.06),
        (65.0, 0.22),
        (67.0, 0.2),
    ]
    .iter()
    .map(|&(note, amp)| bass_tone(hz(note), amp, 1.0, 0.6))
    .collect();
    render(&mut engine, &chord(&tones), &[128]);
    assert_pitches(&voice_pitches(&engine), &[28.0, 61.0, 65.0, 67.0]);
}

#[test]
#[ignore]
fn explore_five() {
    for set in [
        [28.0, 61.0, 63.0, 65.0, 67.0],
        [61.0, 28.0, 63.0, 65.0, 67.0],
        [67.0, 63.0, 61.0, 28.0, 65.0],
        [28.0, 44.0, 61.0, 63.0, 67.0],
        [28.0, 44.0, 54.0, 61.0, 67.0],
    ] {
        let amps = [0.3, 0.27, 0.24, 0.21, 0.08];
        let tones: Vec<Vec<f32>> = set
            .iter()
            .zip(amps)
            .map(|(&n, a)| bass_tone(hz(n), a, 1.0, 0.6))
            .collect();
        let mut engine = engine();
        render(&mut engine, &chord(&tones), &[128]);
        let r = engine.readout();
        let notes: Vec<(f32, f32)> = r.notes.iter().map(|n| (n.pitch, n.level)).collect();
        println!("{set:?}: notes {notes:?} voices {:?}", voice_pitches(&engine));
    }
}
