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

/// Puts silence before `signal`, long enough for the levels set on a fresh
/// engine to finish gliding (the dry signal level to reach exactly zero).
fn after_settling(signal: Vec<f32>) -> Vec<f32> {
    let mut dry = silence(0.3);
    dry.extend(signal);
    dry
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
        assert!(
            cents.abs() < 5.0,
            "waveshape {waveshape}: {cents} cents off"
        );
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

#[test]
fn tracked_layer_level_at_zero_removes_its_contribution() {
    let mut engine = tracked_only();
    engine.set_param(Param::TrackedLevel, 0.0);
    let dry = after_settling(bass_tone(hz(A1), 0.4, 1.0, 0.5));
    let out = render(&mut engine, &dry, &[128]).left;
    assert_eq!(engine.readout().voices.len(), 1);
    assert_eq!(peak(&out), 0.0);
}

#[test]
fn the_voice_fades_in_without_a_click() {
    for waveshape in [0.0, 1.0] {
        let mut engine = tracked_only();
        engine.set_param(Param::TrackedOsc1Waveshape, waveshape);
        let dry = after_settling(bass_tone(hz(A1), 0.4, 1.0, 0.5));
        let out = render(&mut engine, &dry, &[128]).left;
        let start = out
            .iter()
            .position(|&s| s != 0.0)
            .expect("the voice sounds");
        // A click is a step out of silence: the first samples must be a
        // small fraction of the settled level, and the first millisecond
        // still well below it.
        let first_samples = peak(&out[start..start + 5]);
        let first_ms = peak(&out[start..start + 48]);
        let settled = peak(&out[start + 4_800..start + 9_600]);
        assert!(
            first_samples < 0.01 * settled && first_ms < 0.3 * settled,
            "waveshape {waveshape}: {first_samples}, {first_ms} vs {settled}"
        );
    }
}

#[test]
fn dry_signal_direct_layer_and_tracked_layer_mix_by_their_levels() {
    let dry = after_settling(bass_tone(hz(A1), 0.4, 1.0, 0.8));
    let render_with = |dry_level: f32, direct: f32, tracked: f32| {
        let mut engine = engine();
        // Keeps the sum under the limiter's ceiling, so mixing is linear.
        engine.set_param(Param::OutputLevel, 0.25);
        engine.set_param(Param::DryLevel, dry_level);
        engine.set_param(Param::DirectLevel, direct);
        engine.set_param(Param::TrackedLevel, tracked);
        render(&mut engine, &dry, &[128]).left
    };
    let all = render_with(0.8, 0.6, 0.7);
    let parts = [
        render_with(0.8, 0.0, 0.0),
        render_with(0.0, 0.6, 0.0),
        render_with(0.0, 0.0, 0.7),
    ];
    for part in &parts {
        assert!(peak(part) > 0.02, "every layer contributes");
    }
    let worst = (0..all.len())
        .map(|n| (all[n] - parts.iter().map(|p| p[n]).sum::<f32>()).abs())
        .fold(0.0, f32::max);
    assert!(worst < 1e-5, "{worst}");
    assert!(peak(&all) < synthbass_engine::LIMITER_CEILING);
}

#[test]
fn an_out_of_range_or_nan_noise_gate_does_not_silence_the_tracked_layer() {
    for threshold in [5.0, f32::NAN, f32::INFINITY] {
        let mut engine = tracked_only();
        engine.set_noise_gate(threshold);
        render(&mut engine, &bass_tone(hz(A1), 0.4, 1.0, 0.5), &[128]);
        let voices = engine.readout().voices;
        assert_eq!(voices.len(), 1, "gate {threshold}: {voices:?}");
    }
}

/// Adds a short burst of a note `sharp` semitones above `held`, starting at
/// `at` seconds and fading out over `seconds`: an attack whose analysis
/// briefly hears a candidate note beside the held one.
fn add_attack(dry: &mut [f32], held: f32, sharp: f32, at: f32, seconds: f32) {
    let start = (at * SAMPLE_RATE) as usize;
    let burst = bass_tone(hz(held + sharp), 0.6, 1.0, seconds);
    let len = burst.len() as f32;
    for (i, (s, b)) in dry[start..].iter_mut().zip(burst).enumerate() {
        *s += b * (1.0 - i as f32 / len);
    }
}

#[test]
fn a_held_note_keeps_its_voice_through_an_attack_beside_it() {
    let held = 28.0;
    let mut dry = after_settling(bass_tone(hz(held), 0.3, 1.0, 1.3));
    add_attack(&mut dry, held, 0.55, 0.8, 0.03);
    add_attack(&mut dry, held, 0.45, 1.2, 0.05);
    let mut engine = engine();
    let mut held_voice = None;
    for (i, block) in dry.chunks(128).enumerate() {
        render(&mut engine, block, &[128]);
        let voices = engine.readout().voices;
        let at_held = voices.iter().find(|v| (v.pitch - held).abs() < 1.0);
        match (held_voice, at_held) {
            (None, Some(voice)) => held_voice = Some(voice.id),
            (None, None) => {}
            (Some(id), _) => assert!(
                voices.iter().any(|v| v.id == id),
                "{:.3} s: the held note's voice {id} was replaced by {:?}",
                (i * 128) as f32 / SAMPLE_RATE,
                &voices[..]
            ),
        }
    }
    assert!(held_voice.is_some(), "the held note never got a voice");
}
