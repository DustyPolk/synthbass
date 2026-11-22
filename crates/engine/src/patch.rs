//! The patch record: a named, versioned set of every sound parameter, and its
//! human-readable file format.

use crate::direct::ENVELOPE_OCTAVES;
use crate::{Format, Octave, Param, SubWaveshape, Waveshape};

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
        file.to_string()
    }

    /// Reads a patch file. Missing, unknown, or unreadable fields fall back to
    /// defaults, so files from older and newer versions load; only text that
    /// is not TOML at all is an error. Files from older versions are brought
    /// up to date so they sound as they did (`upgrade_from_v1`).
    /// `fallback_name` names a file without a `name`.
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
        // A file without a version predates versioning: version 1.
        let version = file.get("version").and_then(toml::Value::as_integer);
        if version.unwrap_or(1) < 2 {
            patch.upgrade_from_v1();
        }
        Ok(patch)
    }

    /// Version 2 opened the direct layer's envelope five times wider per unit
    /// of sensitivity, and added the sub-oscillator waveshape with a sine by
    /// default where version 1 always played a square.
    fn upgrade_from_v1(&mut self) {
        let sensitivity = Param::DirectEnvelopeSensitivity;
        self.set(
            sensitivity,
            self.get(sensitivity) / (ENVELOPE_OCTAVES / V1_ENVELOPE_OCTAVES),
        );
        self.set(Param::TrackedSubWaveshape, SubWaveshape::Square.value());
    }
}

/// The direct layer envelope's octaves per unit of level in version 1.
const V1_ENVELOPE_OCTAVES: f32 = 8.0;

/// Bumped when the meaning of a field changes. Files of any version load.
pub const PATCH_VERSION: u32 = 2;

/// Where a param lives in the file: its section and key.
fn field(param: Param) -> (&'static str, &'static str) {
    let info = param.info();
    (info.section.key(), info.key)
}

/// Waveshapes are written by name and octaves as whole numbers, so a file
/// reads the way the window does.
fn encode(param: Param, value: f32) -> toml::Value {
    match param.info().format {
        Format::Waveshape => Waveshape::from_value(value).name().into(),
        Format::SubWaveshape => SubWaveshape::from_value(value).name().into(),
        Format::Octave => i64::from(Octave::from_value(value).octaves()).into(),
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
            Format::Waveshape => Waveshape::from_name(name)?.value().into(),
            Format::SubWaveshape => SubWaveshape::from_name(name)?.value().into(),
            _ => return None,
        },
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(x) => *x,
        _ => return None,
    };
    let number = number as f32;
    number.is_finite().then_some(number)
}
