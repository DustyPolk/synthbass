//! The app settings file: small state remembered across launches.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use synthbass_engine::{NOISE_GATE_DEFAULT, NOISE_GATE_RANGE};

use crate::ports::Routing;

/// Everything in the settings file. Every field has a default, so a file
/// missing newer fields still loads: add new fields the same way.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub ports: Routing,
    /// The patch loaded when the app last ran; `None` on a first launch.
    pub last_patch: Option<String>,
    /// The global noise gate threshold (linear amplitude). Not part of any
    /// patch: it belongs to the room, not the sound.
    pub noise_gate: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            ports: Routing::default(),
            last_patch: None,
            noise_gate: NOISE_GATE_DEFAULT,
        }
    }
}

/// The settings file shared by everyone who changes it (the connection
/// thread saves ports, the window saves the patch and gate). Every clone
/// sees and saves one copy, so no holder overwrites another's change.
#[derive(Clone)]
pub struct Store {
    settings: Arc<Mutex<Settings>>,
    /// `None` when the system has no config directory: changes are kept for
    /// this run only.
    path: Option<PathBuf>,
}

impl Store {
    /// Reads the file; a warning says why defaults were used instead.
    pub fn open(path: Option<PathBuf>) -> (Store, Option<String>) {
        let loaded = path.as_deref().map(Settings::load);
        let warning = loaded.as_ref().and_then(|l| l.warning.clone());
        let settings = loaded.map(|l| l.settings).unwrap_or_default();
        let store = Store {
            settings: Arc::new(Mutex::new(settings)),
            path,
        };
        (store, warning)
    }

    pub fn get(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    /// Changes the settings and saves the file.
    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> std::io::Result<()> {
        let mut settings = self.settings.lock().unwrap();
        change(&mut settings);
        match &self.path {
            Some(path) => settings.save(path),
            None => Ok(()),
        }
    }
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
        match toml::from_str::<Settings>(&text) {
            Ok(settings) => Loaded {
                settings: settings.validated(),
                warning: None,
            },
            // The full error spans several lines with a source excerpt; the
            // message alone fits the window.
            Err(err) => defaults_because(path, &err.message()),
        }
    }

    /// Brings hand-edited values back into range: a noise gate outside its
    /// range is clamped, and one that is not a number becomes the default.
    fn validated(mut self) -> Settings {
        self.noise_gate = if self.noise_gate.is_finite() {
            self.noise_gate
                .clamp(*NOISE_GATE_RANGE.start(), *NOISE_GATE_RANGE.end())
        } else {
            NOISE_GATE_DEFAULT
        };
        self
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
    fn a_file_from_before_patches_loads_the_default_gate_and_no_patch() {
        let path = temp_path("pre-patches");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "[ports]\ninput = \"X:capture_FL\"\n").unwrap();
        let loaded = Settings::load(&path).settings;
        assert_eq!(loaded.last_patch, None);
        assert_eq!(loaded.noise_gate, NOISE_GATE_DEFAULT);
        assert_eq!(Settings::default().noise_gate, NOISE_GATE_DEFAULT);
    }

    #[test]
    fn a_noise_gate_outside_its_range_loads_clamped_and_a_non_number_loads_the_default() {
        let path = temp_path("bad-gate");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for (written, expected) in [
            ("5.0", *NOISE_GATE_RANGE.end()),
            ("0.0", *NOISE_GATE_RANGE.start()),
            ("nan", NOISE_GATE_DEFAULT),
            ("inf", NOISE_GATE_DEFAULT),
        ] {
            std::fs::write(&path, format!("noise_gate = {written}\n")).unwrap();
            let loaded = Settings::load(&path).settings;
            assert_eq!(loaded.noise_gate, expected, "noise_gate = {written}");
        }
    }

    #[test]
    fn last_patch_and_noise_gate_survive_a_save_and_load() {
        let path = temp_path("patch-and-gate");
        let settings = Settings {
            last_patch: Some("Mine".into()),
            noise_gate: 0.0125,
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        assert_eq!(Settings::load(&path).settings, settings);
    }

    #[test]
    fn changes_through_any_clone_of_a_store_all_reach_the_file() {
        // The connection thread saves ports while the window saves the patch
        // and gate; neither may undo the other's change.
        let path = temp_path("store");
        let (store, warning) = Store::open(Some(path.clone()));
        assert_eq!(warning, None);
        let other = store.clone();
        store
            .update(|s| s.last_patch = Some("Squelch".into()))
            .unwrap();
        other
            .update(|s| s.ports.input = Some("X:capture_FR".into()))
            .unwrap();
        store.update(|s| s.noise_gate = 0.02).unwrap();

        let saved = Settings::load(&path).settings;
        assert_eq!(saved.last_patch.as_deref(), Some("Squelch"));
        assert_eq!(saved.ports.input.as_deref(), Some("X:capture_FR"));
        assert_eq!(saved.noise_gate, 0.02);
        assert_eq!(other.get(), saved);
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
            ..Settings::default()
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.warning, None);
        assert_eq!(loaded.settings, settings);
    }
}
