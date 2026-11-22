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
