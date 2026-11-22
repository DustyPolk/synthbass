//! The tracked layer's first voice, driven through the engine's public
//! interface with synthetic bass-like tones.

mod common;

use common::*;
use synthbass_engine::{Engine, Param};

const A1: f32 = 33.0;

/// An engine whose output is the tracked layer alone.
fn tracked_only() -> Engine {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    engine.set_param(Param::TrackedLevel, 1.0);
    engine
}

fn silence(seconds: f32) -> Vec<f32> {
    vec![0.0; (seconds * SAMPLE_RATE) as usize]
}

#[test]
fn a_note_produces_one_voice_at_its_pitch() {
    let mut engine = tracked_only();
    render(&mut engine, &bass_tone(hz(A1), 0.4, 1.0, 0.5), &[128]);
    let voices = engine.readout().voices;
    assert_eq!(voices.len(), 1, "{voices:?}");
    assert!((voices[0].pitch - A1).abs() < 0.05, "{voices:?}");
}

/// Frequency from the spacing of upward zero crossings.
fn frequency(signal: &[f32]) -> f32 {
    let crossings: Vec<usize> = signal
        .windows(2)
        .enumerate()
        .filter(|(_, w)| w[0] < 0.0 && w[1] >= 0.0)
        .map(|(n, _)| n)
        .collect();
    let periods = (crossings.len() - 1) as f32;
    SAMPLE_RATE * periods / (crossings[crossings.len() - 1] - crossings[0]) as f32
}

#[test]
fn the_voice_sounds_at_the_detected_pitch_as_saw_and_square() {
    for waveshape in [0.0, 1.0] {
        let mut engine = tracked_only();
        engine.set_param(Param::TrackedOsc1Waveshape, waveshape);
        let out = render(&mut engine, &bass_tone(hz(A1), 0.4, 1.0, 0.6), &[128]).left;
        let settled = &out[(0.2 * SAMPLE_RATE) as usize..];
        let cents = 1200.0 * (frequency(settled) / hz(A1)).log2();
        assert!(cents.abs() < 5.0, "waveshape {waveshape}: {cents} cents off");
    }
}

/// Renders `dry` and returns the first voice's loudness at each of `times`
/// (seconds), or 0 when no voice sounds.
fn loudness_at(engine: &mut Engine, dry: &[f32], times: &[f32]) -> Vec<f32> {
    let mut start = 0;
    times
        .iter()
        .map(|&t| {
            let end = (t * SAMPLE_RATE) as usize;
            render(engine, &dry[start..end], &[64]);
            start = end;
            engine.readout().voices.first().map_or(0.0, |v| v.loudness)
        })
        .collect()
}

#[test]
fn voice_loudness_follows_the_notes_decay() {
    let mut engine = tracked_only();
    // The tone decays as e^(−t / 1.5 s): 0.3 s → 1.2 s is a factor 0.549.
    let tone = bass_tone(hz(A1), 0.4, 1.0, 1.3);
    let loudness = loudness_at(&mut engine, &tone, &[0.3, 1.2]);
    let ratio = loudness[1] / loudness[0];
    assert!((ratio - 0.549).abs() < 0.05, "{loudness:?}");
    assert!(loudness[0] > 0.1 && loudness[0] < 0.6, "{loudness:?}");
}

#[test]
fn voice_loudness_drops_with_a_sudden_palm_mute() {
    let mut engine = tracked_only();
    let mut tone = bass_tone(hz(A1), 0.4, 1.0, 1.0);
    let drop = (0.5 * SAMPLE_RATE) as usize;
    tone[drop..].iter_mut().for_each(|s| *s *= 0.25);
    // The tracker's note level settles within ~150 ms at A1; the voice
    // follows it. Expect 0.25 for the mute times e^(−0.16 / 1.5) for decay.
    let loudness = loudness_at(&mut engine, &tone, &[0.49, 0.65]);
    let ratio = loudness[1] / loudness[0];
    assert!((ratio - 0.225).abs() < 0.03, "{loudness:?}");
}

#[test]
fn voice_output_scales_with_its_loudness() {
    let mut engine = tracked_only();
    let mut tone = bass_tone(hz(A1), 0.4, 1.0, 1.0);
    let drop = (0.5 * SAMPLE_RATE) as usize;
    tone[drop..].iter_mut().for_each(|s| *s *= 0.25);
    let out = render(&mut engine, &tone, &[128]).left;
    let window = |t: f32| {
        let n = (t * SAMPLE_RATE) as usize;
        peak(&out[n..n + 2_400])
    };
    let ratio = window(0.75) / window(0.4);
    // 0.25 for the mute times e^(−0.35 / 1.5) for the decay.
    assert!((ratio - 0.198).abs() < 0.03, "{ratio}");
}


#[test]
fn muting_the_string_ends_the_voice_and_its_sound() {
    let mut engine = tracked_only();
    let mut dry = bass_tone(hz(A1), 0.4, 1.0, 0.5);
    dry.extend(silence(0.3));
    let out = render(&mut engine, &dry, &[128]).left;
    assert!(engine.readout().voices.is_empty());
    let muted = (0.5 * SAMPLE_RATE) as usize;
    let after = (0.15 * SAMPLE_RATE) as usize;
    assert_eq!(peak(&out[muted + after..]), 0.0);
}

#[test]
fn a_note_decaying_below_the_noise_gate_ends_the_voice() {
    let mut engine = tracked_only();
    engine.set_noise_gate(0.05);
    // 0.4 · e^(−t / 1.5) falls below 0.05 around 3.1 s.
    let out = render(&mut engine, &bass_tone(hz(A1), 0.4, 1.0, 4.0), &[128]).left;
    assert!(engine.readout().voices.is_empty());
    assert_eq!(peak(&out[(3.6 * SAMPLE_RATE) as usize..]), 0.0);
}

