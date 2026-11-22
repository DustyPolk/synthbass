//! The app settings file: small state remembered across launches.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ports::Routing;

/// Everything in the settings file. Every field has a default, so a file
/// missing newer fields still loads: add new fields the same way.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ports: Routing,
}

/// The result of reading the settings file. A problem with the file never
/// stops the app: it falls back to defaults and says why.
pub struct Loaded {
    pub settings: Settings,
    pub warning: Option<String>,
}

impl Settings {
    pub fn load(path: &Path) -> Loaded {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Loaded {
                    settings: Settings::default(),
                    warning: None,
                };
            }
            Err(err) => return defaults_because(path, &err),
        };
        match toml::from_str(&text) {
            Ok(settings) => Loaded {
                settings,
                warning: None,
            },
            // The full error spans several lines with a source excerpt; the
            // message alone fits the window.
            Err(err) => defaults_because(path, &err.message()),
        }
    }

    /// Writes the file atomically (temp file, then rename), so a crash mid-write
    /// never leaves a half-written file behind.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, path)
    }
}

/// `~/.config/synthbass/settings.toml`, or `None` if the system has no config
/// directory.
pub fn default_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("synthbass").join("settings.toml"))
}

fn defaults_because(path: &Path, err: &dyn std::fmt::Display) -> Loaded {
    Loaded {
        settings: Settings::default(),
        warning: Some(format!(
            "Could not read settings file {}, using defaults: {err}",
            path.display()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("synthbass-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.toml")
    }

    #[test]
    fn missing_file_loads_defaults_without_warning() {
        let loaded = Settings::load(&temp_path("missing"));
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.warning, None);
    }

    #[test]
    fn corrupt_file_loads_defaults_with_warning() {
        let path = temp_path("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[ports\ninput = 3 = 4\0garbage").unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.warning.unwrap().contains("using defaults"));
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let path = temp_path("partial");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            "future_field = 1\n[ports]\ninput = \"X:capture_FL\"\n",
        )
        .unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.settings.ports.input.as_deref(), Some("X:capture_FL"));
        assert_eq!(loaded.settings.ports.output_1, None);
    }

    #[test]
    fn saved_ports_survive_a_save_and_load() {
        let path = temp_path("round-trip");
        let settings = Settings {
            ports: Routing {
                input: Some("Scarlett:capture_FR".into()),
                output_1: Some("Scarlett:playback_FL".into()),
                output_2: None,
            },
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.settings, settings);
    }
}
