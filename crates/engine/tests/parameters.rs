//! Each sound parameter's facts (range, default, label, display format and
//! patch key) come from one definition.

use std::collections::HashSet;

use synthbass_engine::{Param, Section};

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
