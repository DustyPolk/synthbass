//! The voice's Moog-style sound: oscillator 2, sub-oscillator, and the
//! per-voice ladder filter with its filter envelope, driven through the
//! engine's public interface with synthetic bass-like tones.

mod common;

use common::*;
use synthbass_engine::{Engine, Param};

const A1: f32 = 33.0;

/// An engine whose output is the tracked layer alone, with the filter wide
/// open and no filter envelope, so oscillator tests hear the oscillators.
fn open_voice() -> Engine {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    engine.set_param(Param::TrackedLevel, 1.0);
    engine.set_param(Param::TrackedCutoff, 8_000.0);
    engine.set_param(Param::TrackedResonance, 0.0);
    engine.set_param(Param::TrackedFilterEnvelopeAmount, 0.0);
    engine
}

/// Settled output of `engine` for a plucked note at `note`.
fn settled_output(engine: &mut Engine, note: f32) -> Vec<f32> {
    let out = render(engine, &bass_tone(hz(note), 0.4, 1.0, 0.8), &[128]).left;
    out[(0.3 * SAMPLE_RATE) as usize..].to_vec()
}

/// Fundamental frequency: the shortest lag at which the signal repeats
/// (normalised autocorrelation above 0.95), refined to a fraction of a
/// sample.
fn fundamental(signal: &[f32]) -> f32 {
    let window = 9_600;
    let nsdf = |lag: usize| {
        let (mut r, mut m) = (0.0f64, 0.0f64);
        for n in 0..window {
            let (a, b) = (signal[n] as f64, signal[n + lag] as f64);
            r += a * b;
            m += a * a + b * b;
        }
        2.0 * r / m
    };
    let min_lag = (SAMPLE_RATE / 500.0) as usize;
    let max_lag = (SAMPLE_RATE / 10.0) as usize;
    let mut lag = (min_lag..max_lag)
        .find(|&lag| nsdf(lag) > 0.95)
        .expect("the signal repeats");
    while nsdf(lag + 1) > nsdf(lag) {
        lag += 1;
    }
    let (a, b, c) = (nsdf(lag - 1), nsdf(lag), nsdf(lag + 1));
    let offset = 0.5 * (a - c) / (a - 2.0 * b + c);
    SAMPLE_RATE / (lag as f64 + offset) as f32
}

fn cents(measured: f32, expected: f32) -> f32 {
    1200.0 * (measured / expected).log2()
}

#[test]
fn the_sub_oscillator_sounds_an_octave_below_the_voice() {
    let mut without = open_voice();
    let plain = fundamental(&settled_output(&mut without, A1));
    assert!(cents(plain, hz(A1)).abs() < 5.0, "{plain} Hz");

    let mut with = open_voice();
    with.set_param(Param::TrackedSubLevel, 1.0);
    let sub = fundamental(&settled_output(&mut with, A1));
    assert!(cents(sub, hz(A1 - 12.0)).abs() < 5.0, "{sub} Hz");
}

/// Oscillator 2 alone, at `octave` and `detune` cents.
fn oscillator_2_frequency(octave: f32, detune: f32, waveshape: f32) -> f32 {
    let mut engine = open_voice();
    engine.set_param(Param::TrackedOscMix, 1.0);
    engine.set_param(Param::TrackedOsc2Octave, octave);
    engine.set_param(Param::TrackedOsc2Detune, detune);
    engine.set_param(Param::TrackedOsc2Waveshape, waveshape);
    fundamental(&settled_output(&mut engine, A1))
}

#[test]
fn the_octave_switch_shifts_oscillator_2_by_exactly_an_octave() {
    for waveshape in [0.0, 1.0] {
        let down = oscillator_2_frequency(-1.0, 0.0, waveshape);
        let unison = oscillator_2_frequency(0.0, 0.0, waveshape);
        let up = oscillator_2_frequency(1.0, 0.0, waveshape);
        assert!(cents(unison, hz(A1)).abs() < 5.0, "{unison} Hz");
        assert!(cents(down, unison / 2.0).abs() < 1.0, "{down} vs {unison}");
        assert!(cents(up, unison * 2.0).abs() < 1.0, "{up} vs {unison}");
    }
}

#[test]
fn detune_shifts_oscillator_2_by_its_cents() {
    let unison = oscillator_2_frequency(0.0, 0.0, 0.0);
    for detune in [-30.0, 25.0] {
        let detuned = oscillator_2_frequency(0.0, detune, 0.0);
        assert!(
            (cents(detuned, unison) - detune).abs() < 1.0,
            "{detune}: {detuned} vs {unison}"
        );
    }
}

/// A dark filter that a full-depth envelope opens, scaled fully by how hard
/// the note is picked.
fn plucky_voice(envelope_amount: f32) -> Engine {
    let mut engine = open_voice();
    engine.set_param(Param::TrackedCutoff, 150.0);
    engine.set_param(Param::TrackedFilterEnvelopeAmount, envelope_amount);
    engine.set_param(Param::TrackedFilterEnvelopeDecay, 0.3);
    engine.set_param(Param::TrackedPickSensitivity, 1.0);
    engine
}

/// Share of the signal's energy in its first difference: a loudness-free
/// measure of how bright it is.
fn brightness(signal: &[f32]) -> f32 {
    let energy: f32 = signal.iter().map(|s| s * s).sum();
    let difference: f32 = signal.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
    difference / energy
}

/// A plucked note after enough silence for params set on a fresh engine to
/// finish gliding from their defaults.
fn pluck_after_settling(amplitude: f32, seconds: f32) -> Vec<f32> {
    let mut dry = vec![0.0; (0.3 * SAMPLE_RATE) as usize];
    dry.extend(bass_tone(hz(A1), amplitude, 1.0, seconds));
    dry
}

/// Brightness of the voice over `from..to` seconds after it starts sounding.
fn brightness_after_start(out: &[f32], from: f32, to: f32) -> f32 {
    let start = out
        .iter()
        .position(|&s| s != 0.0)
        .expect("the voice sounds");
    let at = |t: f32| start + (t * SAMPLE_RATE) as usize;
    brightness(&out[at(from)..at(to)])
}

#[test]
fn a_harder_pick_opens_the_filter_envelope_further() {
    let pluck = |amplitude: f32| {
        let mut engine = plucky_voice(1.0);
        let out = render(&mut engine, &pluck_after_settling(amplitude, 0.5), &[128]).left;
        brightness_after_start(&out, 0.01, 0.06)
    };
    let soft = pluck(0.05);
    let hard = pluck(0.5);
    assert!(hard > 2.0 * soft, "hard {hard} vs soft {soft}");
}

#[test]
fn filter_envelope_amount_zero_leaves_the_filter_static() {
    let early_and_late = |amount: f32| {
        let mut engine = plucky_voice(amount);
        let out = render(&mut engine, &pluck_after_settling(0.5, 1.0), &[128]).left;
        (
            brightness_after_start(&out, 0.01, 0.06),
            brightness_after_start(&out, 0.5, 0.55),
        )
    };
    let (early, late) = early_and_late(0.0);
    assert!(
        (early / late - 1.0).abs() < 0.1,
        "early {early} vs late {late}"
    );
    // The same measure does see a sweep when there is one.
    let (early, late) = early_and_late(1.0);
    assert!(early > 2.0 * late, "early {early} vs late {late}");
}

#[test]
fn max_resonance_stays_under_the_limiter_ceiling() {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    engine.set_param(Param::OutputLevel, 4.0);
    engine.set_param(Param::TrackedLevel, 1.0);
    engine.set_param(Param::TrackedResonance, 1.0);
    engine.set_param(Param::TrackedCutoff, 200.0);
    engine.set_param(Param::TrackedFilterEnvelopeAmount, 1.0);
    engine.set_param(Param::TrackedPickSensitivity, 0.0);
    engine.set_param(Param::TrackedSubLevel, 1.0);
    engine.set_param(Param::TrackedOscMix, 0.5);
    engine.set_param(Param::TrackedOsc2Octave, 1.0);
    let mut dry = Vec::new();
    for note in [28.0, 40.0, 52.0, 64.0] {
        dry.extend(bass_tone(hz(note), 0.9, 1.0, 0.6));
    }
    let out = render(&mut engine, &dry, &[128]).left;
    assert!(out.iter().all(|s| s.is_finite()));
    assert!(peak(&out) <= synthbass_engine::LIMITER_CEILING);
    assert!(
        peak(&out) > 0.5,
        "the voice was loud enough to test the ceiling"
    );
}

#[test]
fn string_following_still_sets_voice_loudness() {
    let mut engine = plucky_voice(0.0);
    engine.set_param(Param::TrackedSubLevel, 0.6);
    engine.set_param(Param::TrackedOscMix, 0.5);
    engine.set_param(Param::TrackedResonance, 0.7);
    let mut tone = pluck_after_settling(0.4, 1.0);
    let drop = (0.8 * SAMPLE_RATE) as usize;
    tone[drop..].iter_mut().for_each(|s| *s *= 0.25);
    let out = render(&mut engine, &tone, &[128]).left;
    let window = |t: f32| {
        let n = (t * SAMPLE_RATE) as usize;
        peak(&out[n..n + 2_400])
    };
    // 0.25 for the mute times e^(−0.35 / 1.5) for the decay.
    let ratio = window(1.05) / window(0.7);
    assert!((ratio - 0.198).abs() < 0.03, "{ratio}");
}

/// Brightness of `out` over `from..to` seconds.
fn brightness_between(out: &[f32], from: f32, to: f32) -> f32 {
    let at = |t: f32| (t * SAMPLE_RATE) as usize;
    brightness(&out[at(from)..at(to)])
}

#[test]
fn picking_the_same_note_again_refires_the_filter_envelope() {
    let mut engine = plucky_voice(1.0);
    engine.set_param(Param::TrackedFilterEnvelopeDecay, 0.15);
    // The string is picked again 0.6 s in, without being muted first.
    let mut dry = pluck_after_settling(0.4, 0.6);
    dry.extend(bass_tone(hz(A1), 0.4, 1.0, 0.4));
    let out = render(&mut engine, &dry, &[128]).left;
    assert_eq!(engine.readout().voices.len(), 1);
    let repick = 0.9;
    let before = brightness_between(&out, repick - 0.05, repick);
    let after = brightness_between(&out, repick + 0.03, repick + 0.08);
    assert!(after > 2.0 * before, "after {after} vs before {before}");
}
