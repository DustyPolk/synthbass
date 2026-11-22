mod common;

use common::*;

#[test]
fn dry_signal_peak_reads_the_dry_signal() {
    let mut engine = engine();
    render(&mut engine, &sine(110.0, 0.5, 0.5), &[128]);
    let level = engine.readout().dry_signal_peak;
    assert!((level - 0.5).abs() < 0.01, "level {level}");
}

#[test]
fn dry_signal_peak_ignores_dry_and_output_levels() {
    let mut engine = engine();
    engine.set_param(synthbass_engine::Param::DryLevel, 0.0);
    engine.set_param(synthbass_engine::Param::OutputLevel, 0.0);
    render(&mut engine, &sine(110.0, 0.5, 0.5), &[128]);
    assert!((engine.readout().dry_signal_peak - 0.5).abs() < 0.01);
}

#[test]
fn dry_signal_peak_falls_back_to_zero_after_the_signal_stops() {
    let mut engine = engine();
    render(&mut engine, &sine(110.0, 0.5, 0.5), &[128]);
    render(&mut engine, &vec![0.0; 96_000], &[128]);
    let level = engine.readout().dry_signal_peak;
    assert!(level < 0.001, "level {level}");
}
