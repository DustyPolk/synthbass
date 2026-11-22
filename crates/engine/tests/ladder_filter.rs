mod common;

use common::*;
use synthbass_engine::LadderFilter;

fn filter(cutoff: f32, resonance: f32) -> LadderFilter {
    let mut filter = LadderFilter::new(SAMPLE_RATE);
    filter.set_cutoff(cutoff);
    filter.set_resonance(resonance);
    filter
}

fn run(filter: &mut LadderFilter, input: &[f32]) -> Vec<f32> {
    input.iter().map(|&x| filter.process(x)).collect()
}

/// Peak gain of a quiet tone (quiet so the input saturation stays linear),
/// measured after the filter settles.
fn gain_at(cutoff: f32, resonance: f32, freq: f32) -> f32 {
    let out = run(&mut filter(cutoff, resonance), &sine(freq, 0.01, 1.0));
    peak(&out[24_000..]) / 0.01
}

#[test]
fn passes_lows_and_cuts_highs_at_24_db_per_octave() {
    assert!((gain_at(1_000.0, 0.0, 50.0) - 1.0).abs() < 0.02);
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
    // A ±4 square at 41 Hz while the cutoff sweeps its full range every
    // second, then silence.
    let mut filter = filter(1_000.0, 1.0);
    let mut out = Vec::new();
    for n in 0..10 * 48_000 {
        let t = n as f32 / SAMPLE_RATE;
        filter.set_cutoff(10.0 * 2f32.powf(12.0 * t.fract()));
        let x = if (t * 41.0).fract() < 0.5 { 4.0 } else { -4.0 };
        out.push(filter.process(x));
    }
    assert!(out.iter().all(|y| y.is_finite()), "non-finite output");
    assert!(peak(&out) < 2.0, "peak {}", peak(&out));

    // The ring dies away to exact silence, never into denormals.
    let tail = run(&mut filter, &vec![0.0; 3 * 48_000]);
    assert!(tail.iter().all(|y| !y.is_subnormal()), "subnormal output");
    assert_eq!(peak(&tail[2 * 48_000..]), 0.0);
}
