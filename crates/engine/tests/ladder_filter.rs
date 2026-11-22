//! The ladder filter, heard through the direct layer with its drive and
//! envelope turned off, so the output is the filter alone at a fixed cutoff.

mod common;

use common::*;
use synthbass_engine::{Engine, LIMITER_CEILING, Param};

/// An engine whose output is the direct layer's filter at `cutoff` and
/// `resonance`: no drive, no envelope sweep, no dry signal.
fn filter(cutoff: f32, resonance: f32) -> Engine {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    engine.set_param(Param::DirectLevel, 1.0);
    engine.set_param(Param::DirectDrive, 0.0);
    engine.set_param(Param::DirectEnvelopeSensitivity, 0.0);
    engine.set_param(Param::DirectCutoff, cutoff);
    engine.set_param(Param::DirectResonance, resonance);
    engine
}

/// Peak gain of a quiet tone (quiet so the saturation stays linear),
/// measured after the filter and the parameter smoothing settle.
fn gain_at(cutoff: f32, resonance: f32, freq: f32) -> f32 {
    let out = render(&mut filter(cutoff, resonance), &sine(freq, 0.01, 1.0), &[128]);
    peak(&out.left[24_000..]) / 0.01
}

#[test]
fn passes_lows_and_cuts_highs_at_24_db_per_octave() {
    let passband = gain_at(1_000.0, 0.0, 50.0);
    assert!((passband - 1.0).abs() < 0.02, "gain {passband}");
    // Two octaves above cutoff: about −48 dB.
    let two_octaves_up = gain_at(1_000.0, 0.0, 4_000.0);
    assert!(
        (0.002..0.008).contains(&two_octaves_up),
        "gain {two_octaves_up}"
    );
}

#[test]
fn resonance_emphasises_the_cutoff_frequency() {
    let flat = gain_at(500.0, 0.0, 500.0);
    let resonant = gain_at(500.0, 1.0, 500.0);
    assert!(resonant > 10.0 * flat, "flat {flat}, resonant {resonant}");
}

#[test]
fn stays_stable_at_maximum_resonance_under_a_hot_swept_input() {
    // Full drive into maximum resonance: a ±0.5 square at 41 Hz while the
    // cutoff sweeps its full range every second, then silence. The output
    // level is low enough that the limiter never touches a stable filter.
    const OUTPUT_LEVEL: f32 = 0.25;
    let mut engine = filter(1_000.0, 1.0);
    engine.set_param(Param::DirectDrive, 1.0);
    engine.set_param(Param::OutputLevel, OUTPUT_LEVEL);
    let block = 128;
    let mut dry = Vec::new();
    for n in 0..10 * 48_000 {
        let t = n as f32 / SAMPLE_RATE;
        dry.push(if (t * 41.0).fract() < 0.5 { 0.5 } else { -0.5 });
    }
    let mut out = Vec::new();
    for (i, chunk) in dry.chunks(block).enumerate() {
        let t = (i * block) as f32 / SAMPLE_RATE;
        engine.set_param(Param::DirectCutoff, 40.0 * 200f32.powf(t.fract()));
        out.extend(render(&mut engine, chunk, &[block]).left);
    }
    assert!(out.iter().all(|y| y.is_finite()), "non-finite output");
    let filter_peak = peak(&out) / OUTPUT_LEVEL;
    assert!(filter_peak < 2.0, "filter peak {filter_peak}");
    assert!(peak(&out) < LIMITER_CEILING);

    // The ring dies away to exact silence, never into denormals.
    let tail = render(&mut engine, &vec![0.0; 3 * 48_000], &[block]).left;
    assert!(tail.iter().all(|y| !y.is_subnormal()), "subnormal output");
    assert_eq!(peak(&tail[2 * 48_000..]), 0.0);
}
