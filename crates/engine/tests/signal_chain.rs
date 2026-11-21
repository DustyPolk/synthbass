mod common;

use common::*;
use synthbass_engine::{LIMITER_CEILING, Param};

#[test]
fn silence_in_gives_silence_out() {
    let mut engine = engine();
    let out = render(&mut engine, &vec![0.0; 48_000], &[128]);
    assert_eq!(peak(&out.left), 0.0);
    assert_eq!(peak(&out.right), 0.0);
}

#[test]
fn dry_signal_passes_to_both_outputs_at_default_levels() {
    let mut engine = engine();
    let dry = sine(110.0, 0.25, 0.5);
    let out = render(&mut engine, &dry, &[128]);
    for (i, &d) in dry.iter().enumerate() {
        assert!((out.left[i] - d).abs() < 1e-6, "left differs at {i}");
        assert!((out.right[i] - d).abs() < 1e-6, "right differs at {i}");
    }
}

#[test]
fn dry_signal_level_at_zero_removes_the_dry_signal() {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    let out = render(&mut engine, &sine(110.0, 0.25, 1.0), &[128]);
    let settled = &out.left[24_000..];
    assert!(peak(settled) < 1e-4, "peak {}", peak(settled));
}

#[test]
fn output_level_scales_both_outputs() {
    let mut engine = engine();
    engine.set_param(Param::OutputLevel, 0.5);
    let out = render(&mut engine, &sine(110.0, 0.25, 1.0), &[128]);
    let (left, right) = (&out.left[24_000..], &out.right[24_000..]);
    assert!(
        (peak(left) - 0.125).abs() < 1e-3,
        "left peak {}",
        peak(left)
    );
    assert!(
        (peak(right) - 0.125).abs() < 1e-3,
        "right peak {}",
        peak(right)
    );
}

/// Largest sample-to-sample jump: a click shows up as a jump far larger than
/// the signal's own slope.
fn max_step(signal: &[f32]) -> f32 {
    signal
        .windows(2)
        .fold(0.0, |max, w| max.max((w[1] - w[0]).abs()))
}

#[test]
fn mid_stream_level_changes_do_not_click() {
    // A 110 Hz sine at 0.8 moves at most ~0.0115 per sample.
    let dry = sine(110.0, 0.8, 1.0);
    for param in Param::ALL {
        let mut engine = engine();
        let first = render(&mut engine, &dry[..10_007], &[128]);
        engine.set_param(param, 0.0);
        let second = render(&mut engine, &dry[10_007..], &[128]);
        let joined: Vec<f32> = first.left.into_iter().chain(second.left).collect();
        assert!(
            max_step(&joined) < 0.02,
            "{param:?} step {}",
            max_step(&joined)
        );
    }
}

#[test]
fn loud_input_at_max_output_level_never_exceeds_the_limiter_ceiling() {
    let mut engine = engine();
    engine.set_param(Param::OutputLevel, *Param::OutputLevel.range().end());
    // A hot square wave well past full scale, with a sudden onset.
    let mut dry = vec![0.0; 4_800];
    dry.extend((0..48_000).map(|n| if (n / 200) % 2 == 0 { 2.0 } else { -2.0 }));
    let out = render(&mut engine, &dry, &[128]);
    assert!(
        peak(&out.left) <= LIMITER_CEILING,
        "left peak {}",
        peak(&out.left)
    );
    assert!(
        peak(&out.right) <= LIMITER_CEILING,
        "right peak {}",
        peak(&out.right)
    );
    // Limiting, not muting.
    assert!(peak(&out.left[24_000..]) > 0.8 * LIMITER_CEILING);
}

#[test]
fn varying_and_odd_block_sizes_match_a_single_large_block() {
    // Hot enough to engage the limiter, with a level change partway through.
    let dry = sine(55.0, 1.5, 0.5);
    let run = |block_sizes: &[usize]| {
        let mut engine = engine();
        engine.set_param(Param::OutputLevel, 2.0);
        let first = render(&mut engine, &dry[..9_000], block_sizes);
        engine.set_param(Param::DryLevel, 0.3);
        let second = render(&mut engine, &dry[9_000..], block_sizes);
        (first.left, second.left, first.right, second.right)
    };
    let reference = run(&[MAX_BLOCK]);
    for sizes in [&[1][..], &[7], &[64], &[127], &[3, 128, 1, 1000, 17, 64]] {
        assert_eq!(run(sizes), reference, "block sizes {sizes:?}");
    }
}
