mod common;

use common::*;
use synthbass_engine::{Engine, LIMITER_CEILING, Param};

/// An engine whose output is the direct layer alone.
fn direct_only() -> Engine {
    let mut engine = engine();
    engine.set_param(Param::DryLevel, 0.0);
    engine.set_param(Param::DirectLevel, 1.0);
    engine
}

/// Every sound param at the hottest end of its range.
fn pathological(engine: &mut Engine) {
    for param in [
        Param::DirectDrive,
        Param::DirectResonance,
        Param::DirectEnvelopeSensitivity,
        Param::DirectLevel,
    ] {
        engine.set_param(param, *param.range().end());
    }
}

fn mix(a: &[f32], b: &[f32]) -> Vec<f32> {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

#[test]
fn an_onset_produces_direct_layer_output_within_the_same_block() {
    // The onset lands 16 samples into a 128-sample block.
    let onset = 8 * 128 + 16;
    let block_end = 9 * 128;
    let mut dry = vec![0.0; onset];
    dry.extend(sine(110.0, 0.5, 0.5));
    let mut engine = direct_only();
    // Let the level smoothing settle before the note.
    let out = render(&mut engine, &dry, &[128]);

    assert_eq!(peak(&out.left[..onset]), 0.0, "output before the onset");
    let settled = peak(&out.left[onset + 4_800..]);
    let first_block = peak(&out.left[onset..block_end]);
    assert!(settled > 0.05, "settled peak {settled}");
    assert!(
        first_block > 0.25 * settled,
        "first block peak {first_block} vs settled {settled}"
    );
}

#[test]
fn direct_layer_level_at_zero_removes_its_contribution() {
    let dry = sine(110.0, 0.5, 1.0);
    let mut engine = direct_only();
    let playing = render(&mut engine, &dry[..24_000], &[128]);
    assert!(
        peak(&playing.left[12_000..]) > 0.05,
        "layer is audible first"
    );

    engine.set_param(Param::DirectLevel, 0.0);
    let removed = render(&mut engine, &dry[24_000..], &[128]);
    let settled = &removed.left[12_000..];
    assert!(peak(settled) < 1e-4, "peak {}", peak(settled));
}

#[test]
fn a_two_note_chord_stays_finite_bounded_and_rings_out_to_silence() {
    // A major third (A1 + C#2) at full drive, resonance and sensitivity,
    // with the output level low enough that the limiter stays out of it.
    // Silence first so the level settings settle before the chord.
    let mut dry = vec![0.0; 4_800];
    dry.extend(mix(&sine(55.0, 0.4, 2.0), &sine(69.3, 0.4, 2.0)));
    dry.extend(vec![0.0; 3 * 48_000]);
    let mut engine = direct_only();
    pathological(&mut engine);
    engine.set_param(Param::OutputLevel, 0.1);
    let out = render(&mut engine, &dry, &[64]);

    assert!(out.left.iter().all(|s| s.is_finite()), "non-finite output");
    assert!(
        out.left.iter().all(|s| !s.is_subnormal()),
        "subnormal output"
    );
    // Pre-limiter level stays sane: no runaway resonance.
    assert!(
        peak(&out.left) < 0.5 * LIMITER_CEILING,
        "peak {}",
        peak(&out.left)
    );
    assert!(peak(&out.left[48_000..96_000]) > 0.01, "chord is audible");
    // Exactly silent once the ring has died away.
    let tail = &out.left[out.left.len() - 48_000..];
    assert_eq!(peak(tail), 0.0, "tail never fell silent");
}

#[test]
fn max_drive_and_max_resonance_stay_under_the_limiter_ceiling() {
    // A hot onset, then a sweep through the bass range to excite resonance.
    let mut dry = vec![0.0; 4_800];
    dry.extend((0..96_000).map(|n| {
        let t = n as f32 / SAMPLE_RATE;
        1.5 * (std::f32::consts::TAU * (41.0 + 200.0 * t) * t).sin()
    }));
    for cutoff in [40.0, 400.0, 8_000.0] {
        let mut engine = engine();
        pathological(&mut engine);
        engine.set_param(Param::DirectCutoff, cutoff);
        engine.set_param(Param::OutputLevel, *Param::OutputLevel.range().end());
        let out = render(&mut engine, &dry, &[128]);
        for side in [&out.left, &out.right] {
            assert!(
                side.iter().all(|s| s.is_finite()),
                "cutoff {cutoff}: non-finite"
            );
            assert!(
                peak(side) <= LIMITER_CEILING,
                "cutoff {cutoff}: peak {}",
                peak(side)
            );
        }
    }
}

/// Largest sample-to-sample jump.
fn max_step(signal: &[f32]) -> f32 {
    signal
        .windows(2)
        .fold(0.0, |max, w| max.max((w[1] - w[0]).abs()))
}

#[test]
fn mid_stream_direct_control_changes_do_not_click() {
    // A click is a jump larger than the sound makes on either side of the
    // change once it has settled.
    let dry = sine(110.0, 0.5, 1.0);
    let change = 24_007;
    for param in [
        Param::DirectDrive,
        Param::DirectCutoff,
        Param::DirectResonance,
        Param::DirectEnvelopeSensitivity,
        Param::DirectLevel,
    ] {
        // Level goes from audible to zero (`direct_only` turns it up); the
        // rest jump from their default to the far end of their range.
        let range = param.range();
        let default = param.default_value();
        let target = match param {
            Param::DirectLevel => 0.0,
            _ if default - range.start() > range.end() - default => *range.start(),
            _ => *range.end(),
        };
        let mut engine = direct_only();
        let before = render(&mut engine, &dry[..change], &[128]).left;
        engine.set_param(param, target);
        let after = render(&mut engine, &dry[change..], &[128]).left;
        let settled = max_step(&before[change - 4_800..]).max(max_step(&after[9_600..]));
        let transition = max_step(&after[..4_800]);
        assert!(
            transition <= 1.5 * settled + 1e-3,
            "{param:?} → {target}: transition step {transition} vs settled {settled}"
        );
    }
}
