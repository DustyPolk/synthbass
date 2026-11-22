//! The noise gate threshold validates itself: it is always finite and inside
//! its range, wherever it comes from.

use synthbass_engine::NoiseGate;

#[test]
fn a_threshold_inside_the_range_is_kept() {
    assert_eq!(
        NoiseGate::new(0.0125).map(NoiseGate::threshold),
        Some(0.0125)
    );
}

#[test]
fn a_threshold_outside_the_range_is_clamped_to_its_nearer_end() {
    assert_eq!(NoiseGate::new(5.0).map(NoiseGate::threshold), Some(0.1));
    assert_eq!(NoiseGate::new(0.0).map(NoiseGate::threshold), Some(0.0001));
    assert_eq!(NoiseGate::new(-1.0).map(NoiseGate::threshold), Some(0.0001));
}

#[test]
fn a_threshold_that_is_not_a_number_is_rejected() {
    for threshold in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(NoiseGate::new(threshold), None, "{threshold}");
    }
}

#[test]
fn the_default_gate_sits_at_about_minus_54_dbfs() {
    assert_eq!(NoiseGate::default().threshold(), 0.002);
    assert_eq!(NoiseGate::RANGE, 0.0001..=0.1);
}
