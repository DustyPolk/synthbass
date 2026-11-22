//! The patch record: a named, versioned set of every sound parameter, and its
//! human-readable file format.

use crate::{Format, Octave, Param, Waveshape};

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

/// Waveshapes are written by name and octaves as whole numbers, so a file
/// reads the way the window does.
fn encode(param: Param, value: f32) -> toml::Value {
    match param.info().format {
        Format::Waveshape => Waveshape::from_value(value).name().into(),
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
            _ => return None,
        },
        toml::Value::Integer(n) => *n as f64,
        toml::Value::Float(x) => *x,
        _ => return None,
    };
    let number = number as f32;
    number.is_finite().then_some(number)
}
