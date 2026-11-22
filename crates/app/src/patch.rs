//! The patch record: a named, versioned set of every sound parameter, and its
//! human-readable file format.

use synthbass_engine::{Format, Param};

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub name: String,
    /// By `Param::index`.
    values: [f32; Param::ALL.len()],
}

impl Patch {
    /// A patch with every parameter at its default.
    pub fn new(name: impl Into<String>) -> Patch {
        Patch {
            name: name.into(),
            values: Param::ALL.map(Param::default_value),
        }
    }

    pub fn get(&self, param: Param) -> f32 {
        self.values[param.index()]
    }

    /// Sets a parameter, clamped to its range.
    pub fn set(&mut self, param: Param, value: f32) {
        self.values[param.index()] = param.clamp(value);
    }

    /// The file text: `version` and `name`, then one table per section.
    pub fn to_toml(&self) -> String {
        let mut file = toml::Table::new();
        file.insert("version".into(), i64::from(PATCH_VERSION).into());
        file.insert("name".into(), self.name.clone().into());
        for param in Param::ALL {
            let (section, key) = field(param);
            let table = file
                .entry(section)
                .or_insert_with(|| toml::Table::new().into())
                .as_table_mut()
                .expect("sections are tables");
            table.insert(key.into(), encode(param, self.get(param)));
        }
        toml::to_string_pretty(&file).expect("a table of plain values serializes")
    }

    /// Reads a patch file. Missing, unknown, or unreadable fields fall back to
    /// defaults, so files from older and newer versions load; only text that
    /// is not TOML at all is an error. `fallback_name` names a file without
    /// a `name`.
    pub fn from_toml(text: &str, fallback_name: &str) -> Result<Patch, String> {
        let file: toml::Table = toml::from_str(text).map_err(|err| err.message().to_string())?;
        let name = file
            .get("name")
            .and_then(toml::Value::as_str)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(fallback_name);
        let mut patch = Patch::new(name.trim());
        for param in Param::ALL {
            let (section, key) = field(param);
            let value = file
                .get(section)
                .and_then(|table| table.get(key))
                .and_then(|value| decode(param, value));
            if let Some(value) = value {
                patch.set(param, value);
            }
        }
        Ok(patch)
    }
}

/// Bumped when the meaning of a field changes. Files of any version load.
pub const PATCH_VERSION: u32 = 1;

/// Where a param lives in the file: its section and key.
fn field(param: Param) -> (&'static str, &'static str) {
    let info = param.info();
    (info.section.key(), info.key)
}

const WAVESHAPES: [&str; 2] = ["saw", "square"];

/// Waveshapes are written by name and octaves as whole numbers, so a file
/// reads the way the window does.
fn encode(param: Param, value: f32) -> toml::Value {
    match param.info().format {
        Format::Waveshape => WAVESHAPES[usize::from(value >= 0.5)].into(),
        Format::Octave => (value.round() as i64).into(),
        // The shortest decimal that reads back as the same f32 (0.37, not
        // 0.3700000047683716).
        _ => value
            .to_string()
            .parse::<f64>()
            .expect("an f32 prints as a number")
            .into(),
    }
}

/// `None` when the value is the wrong kind or not a finite number.
fn decode(param: Param, value: &toml::Value) -> Option<f32> {
    let number = match value {
        toml::Value::String(name) => match param.info().format {
            Format::Waveshape => WAVESHAPES.iter().position(|shape| shape == name)? as f64,
            _ => return None,
        },
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(x) => *x,
        _ => return None,
    };
    let number = number as f32;
    number.is_finite().then_some(number)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value inside the param's range that is not its default.
    fn unusual(param: Param) -> f32 {
        let range = param.range();
        match param.info().format {
            Format::Waveshape => 1.0,
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
        assert!(text.contains("version = 1"), "{text}");
        assert!(text.contains("resonance = 0.37\n"), "{text}");
        assert!(text.contains("osc1_waveshape = \"saw\""), "{text}");
        assert!(text.contains("[mix]"), "{text}");
    }
}
