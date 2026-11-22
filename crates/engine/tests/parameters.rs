//! Each sound parameter's facts (range, default, label, display format and
//! patch key) come from one definition.

use std::collections::HashSet;

use synthbass_engine::{Octave, Param, Section, Waveshape};

#[test]
fn every_parameter_has_metadata_with_its_default_inside_its_range() {
    for param in Param::ALL {
        let info = param.info();
        assert!(info.min < info.max, "{param:?}");
        assert!(
            param.range().contains(&param.default_value()),
            "{param:?} default {} outside {:?}",
            param.default_value(),
            param.range()
        );
        assert!(!info.label.trim().is_empty(), "{param:?}");
        assert!(!info.key.trim().is_empty(), "{param:?}");
    }
}

#[test]
fn patch_keys_and_labels_are_unique_within_a_section() {
    let mut keys = HashSet::new();
    let mut labels = HashSet::new();
    for param in Param::ALL {
        let info = param.info();
        assert!(keys.insert((info.section, info.key)), "{param:?} key");
        assert!(labels.insert((info.section, info.label)), "{param:?} label");
    }
}

#[test]
fn a_parameters_index_is_its_place_in_the_list_of_all_parameters() {
    for (place, param) in Param::ALL.into_iter().enumerate() {
        assert_eq!(param.index(), place, "{param:?}");
    }
}

#[test]
fn values_read_in_each_parameters_display_format() {
    let cases = [
        (Param::DryLevel, 1.0, "+0.0 dB"),
        (Param::OutputLevel, 2.0, "+6.0 dB"),
        (Param::TrackedLevel, 0.0, "−∞ dB"),
        (Param::DirectCutoff, 400.0, "400 Hz"),
        (Param::DirectResonance, 0.37, "37 %"),
        (Param::TrackedOsc1Waveshape, 0.0, "Saw"),
        (Param::TrackedOsc2Waveshape, 1.0, "Square"),
        (Param::TrackedOsc2Octave, 1.0, "+1"),
        (Param::TrackedOsc2Octave, 0.0, "0"),
        (Param::TrackedOsc2Octave, -1.0, "−1"),
        (Param::TrackedOsc2Detune, -12.0, "-12 ¢"),
        (Param::TrackedFilterEnvelopeDecay, 0.3, "300 ms"),
        (Param::TrackedGlide, 0.03, "30 ms"),
    ];
    for (param, value, text) in cases {
        assert_eq!(param.format(value), text, "{param:?} at {value}");
    }
}

#[test]
fn parameters_are_grouped_into_the_patch_sections() {
    assert_eq!(Param::DirectCutoff.info().section, Section::Direct);
    assert_eq!(Param::TrackedCutoff.info().section, Section::Tracked);
    assert_eq!(Param::DryLevel.info().section, Section::Mix);
    assert_eq!(Section::Tracked.key(), "tracked");
    assert_eq!(Section::Mix.label(), "Mix");
}

#[test]
fn a_waveshape_value_reads_as_saw_below_one_half_and_square_from_it() {
    let cases = [
        (0.0, Waveshape::Saw),
        (0.49, Waveshape::Saw),
        (0.5, Waveshape::Square),
        (1.0, Waveshape::Square),
    ];
    for (value, shape) in cases {
        assert_eq!(Waveshape::from_value(value), shape, "{value}");
    }
    assert_eq!(Waveshape::ALL.map(Waveshape::value), [0.0, 1.0]);
}

#[test]
fn an_octave_value_reads_as_the_nearest_whole_octave() {
    let cases = [
        (-1.0, Octave::Down),
        (-0.6, Octave::Down),
        (-0.4, Octave::Unison),
        (0.4, Octave::Unison),
        (0.6, Octave::Up),
        (1.0, Octave::Up),
    ];
    for (value, octave) in cases {
        assert_eq!(Octave::from_value(value), octave, "{value}");
    }
    assert_eq!(Octave::ALL.map(Octave::value), [-1.0, 0.0, 1.0]);
}

#[test]
fn a_switch_value_between_choices_displays_as_the_choice_it_plays() {
    assert_eq!(Param::TrackedOsc1Waveshape.format(0.7), "Square");
    assert_eq!(Param::TrackedOsc2Waveshape.format(0.3), "Saw");
    assert_eq!(Param::TrackedOsc2Octave.format(-0.6), "−1");
    assert_eq!(Param::TrackedOsc2Octave.format(0.6), "+1");
}
