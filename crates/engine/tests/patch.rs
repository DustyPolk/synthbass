//! The patch record: its file text, and applying a whole patch to the engine.

mod common;

use common::*;
use synthbass_engine::{Format, PATCH_VERSION, Param, Patch, SubWaveshape};

const V1_SQUELCH: &str = include_str!("fixtures/v1_squelch.toml");

#[test]
fn a_patch_file_written_by_v1_loads_sounding_identically() {
    // Version 2 retuned envelope sensitivity to real playing levels (five
    // times the sweep per unit) and added the sub-oscillator waveshape, whose
    // default is a sine; v1 subs were always square.
    let patch = Patch::from_toml(V1_SQUELCH, "fallback").unwrap();
    assert_eq!(patch.name, "Squelch");
    let expected = [
        (Param::DirectDrive, 0.7),
        (Param::DirectCutoff, 250.0),
        (Param::DirectResonance, 0.75),
        (Param::DirectEnvelopeSensitivity, 0.9 / 5.0),
        (Param::DirectLevel, 0.3),
        (Param::TrackedOsc1Waveshape, 0.0),
        (Param::TrackedOsc2Waveshape, 0.0),
        (Param::TrackedOsc2Octave, 0.0),
        (Param::TrackedOsc2Detune, 0.0),
        (Param::TrackedOscMix, 0.0),
        (Param::TrackedSubLevel, 0.2),
        (Param::TrackedSubWaveshape, SubWaveshape::Square.value()),
        (Param::TrackedCutoff, 150.0),
        (Param::TrackedResonance, 0.85),
        (Param::TrackedFilterEnvelopeAmount, 0.9),
        (Param::TrackedFilterEnvelopeDecay, 0.18),
        (Param::TrackedPickSensitivity, 0.7),
        (Param::TrackedGlide, 0.0),
        (Param::TrackedLevel, 0.7),
        (Param::DryLevel, 0.15),
        (Param::OutputLevel, 0.7),
    ];
    assert_eq!(expected.len(), Param::ALL.len());
    for (param, value) in expected {
        assert_eq!(patch.get(param), value, "{param:?}");
    }
}

/// A value inside the param's range that is not its default.
fn unusual(param: Param) -> f32 {
    let range = param.range();
    match param.info().format {
        Format::Waveshape | Format::SubWaveshape => 1.0,
        Format::Octave => -1.0,
        _ => range.start() + 0.37 * (range.end() - range.start()),
    }
}

#[test]
fn saving_then_loading_round_trips_every_parameter() {
    let mut patch = Patch::new("Round trip");
    for param in Param::ALL {
        patch.set(param, unusual(param));
        assert_ne!(patch.get(param), param.default_value(), "{param:?}");
    }
    let loaded = Patch::from_toml(&patch.to_toml(), "fallback").unwrap();
    assert_eq!(loaded, patch);
}

#[test]
fn a_file_missing_newer_fields_loads_with_defaults() {
    // As an older version would have written it: no glide, no osc mix,
    // no mix section; plus a field from some newer version.
    let text = r#"
        version = 1
        name = "Old"
        future_field = "ignored"

        [direct]
        cutoff_hz = 900
        level = 0.5

        [tracked]
        osc1_waveshape = "square"
        resonance = 0.8
        wobble = 3.0
    "#;
    let patch = Patch::from_toml(text, "fallback").unwrap();
    assert_eq!(patch.name, "Old");
    assert_eq!(patch.get(Param::DirectCutoff), 900.0);
    assert_eq!(patch.get(Param::DirectLevel), 0.5);
    assert_eq!(patch.get(Param::TrackedOsc1Waveshape), 1.0);
    assert_eq!(patch.get(Param::TrackedResonance), 0.8);
    for param in [Param::TrackedGlide, Param::TrackedOscMix, Param::DryLevel] {
        assert_eq!(patch.get(param), param.default_value(), "{param:?}");
    }
}

#[test]
fn unreadable_values_fall_back_to_defaults_and_out_of_range_ones_are_clamped() {
    let text = r#"
        [direct]
        cutoff_hz = "loud"
        resonance = nan
        drive = 7.0
        [tracked]
        osc2_waveshape = "triangle"
    "#;
    let patch = Patch::from_toml(text, "My File").unwrap();
    assert_eq!(patch.name, "My File");
    assert_eq!(
        patch.get(Param::DirectCutoff),
        Param::DirectCutoff.default_value()
    );
    assert_eq!(
        patch.get(Param::DirectResonance),
        Param::DirectResonance.default_value()
    );
    assert_eq!(patch.get(Param::DirectDrive), 1.0);
    assert_eq!(patch.get(Param::TrackedOsc2Waveshape), 0.0);
}

#[test]
fn text_that_is_not_toml_is_an_error() {
    assert!(Patch::from_toml("[direct\ncutoff_hz = = 3\0", "x").is_err());
}

#[test]
fn the_file_is_readable_and_carries_a_version() {
    let mut patch = Patch::new("Readable");
    patch.set(Param::DirectResonance, 0.37);
    let text = patch.to_toml();
    assert!(
        text.contains(&format!("version = {PATCH_VERSION}")),
        "{text}"
    );
    assert!(text.contains("name = \"Readable\""), "{text}");
    assert!(text.contains("resonance = 0.37\n"), "{text}");
    assert!(text.contains("osc1_waveshape = \"saw\""), "{text}");
    assert!(text.contains("osc2_octave = 0\n"), "{text}");
    assert!(text.contains("[mix]"), "{text}");
}

/// A held note long enough for both the old and the new patch to settle.
fn held_note() -> Vec<f32> {
    bass_tone(hz(40.0), 0.5, 0.6, 1.5)
}

/// A patch far from `Patch::new`'s defaults in every parameter, with both
/// layers audible.
fn far_patch() -> Patch {
    let mut patch = Patch::new("Far");
    for param in Param::ALL {
        patch.set(param, unusual(param));
    }
    patch.set(Param::DirectLevel, 0.6);
    patch.set(Param::TrackedLevel, 0.8);
    patch
}

const SWITCH: usize = 36_011;

#[test]
fn a_whole_patch_applied_at_once_sounds_the_same_as_its_values_applied_one_by_one() {
    let dry = held_note();
    let patch = Patch::from_toml(V1_SQUELCH, "x").unwrap();
    let mut at_once = engine();
    let mut one_by_one = engine();
    at_once.apply_patch(&far_patch());
    for param in Param::ALL {
        one_by_one.set_param(param, far_patch().get(param));
    }
    let before_a = render(&mut at_once, &dry[..SWITCH], &[128]);
    let before_b = render(&mut one_by_one, &dry[..SWITCH], &[128]);
    at_once.apply_patch(&patch);
    for param in Param::ALL {
        one_by_one.set_param(param, patch.get(param));
    }
    let after_a = render(&mut at_once, &dry[SWITCH..], &[128]);
    let after_b = render(&mut one_by_one, &dry[SWITCH..], &[128]);
    assert!(peak(&after_a.left) > 0.05, "the patch is audible");
    assert_eq!(before_a.left, before_b.left);
    assert_eq!(after_a.left, after_b.left);
    assert_eq!(after_a.right, after_b.right);
}

/// Largest sample-to-sample jump.
fn max_step(signal: &[f32]) -> f32 {
    signal
        .windows(2)
        .fold(0.0, |max, w| max.max((w[1] - w[0]).abs()))
}

#[test]
fn switching_patches_mid_note_does_not_click() {
    // A click is a jump larger than either patch makes once it has settled.
    let dry = held_note();
    let patches = [Patch::from_toml(V1_SQUELCH, "x").unwrap(), far_patch()];
    for (from, to) in [(&patches[0], &patches[1]), (&patches[1], &patches[0])] {
        let mut engine = engine();
        engine.apply_patch(from);
        let before = render(&mut engine, &dry[..SWITCH], &[128]).left;
        engine.apply_patch(to);
        let after = render(&mut engine, &dry[SWITCH..], &[128]).left;
        let settled = max_step(&before[SWITCH - 4_800..]).max(max_step(&after[9_600..19_200]));
        let transition = max_step(&after[..4_800]);
        assert!(settled > 1e-3, "{} → {}: silent", from.name, to.name);
        assert!(
            transition <= 1.5 * settled + 1e-3,
            "{} → {}: transition step {transition} vs settled {settled}",
            from.name,
            to.name
        );
    }
}

#[test]
fn a_current_file_without_a_sub_waveshape_plays_a_sine_sub() {
    let text = "version = 2\nname = \"New\"\n[tracked]\nsub_level = 0.5\n";
    let patch = Patch::from_toml(text, "fallback").unwrap();
    assert_eq!(
        patch.get(Param::TrackedSubWaveshape),
        SubWaveshape::Sine.value()
    );
}

#[test]
fn the_sub_waveshape_is_written_by_name() {
    let mut patch = Patch::new("Named");
    patch.set(Param::TrackedSubWaveshape, SubWaveshape::Square.value());
    assert!(patch.to_toml().contains("sub_waveshape = \"square\""));
    patch.set(Param::TrackedSubWaveshape, SubWaveshape::Sine.value());
    assert!(patch.to_toml().contains("sub_waveshape = \"sine\""));
}
