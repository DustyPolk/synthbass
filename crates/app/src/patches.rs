//! Patch storage: the built-in patches embedded in the app, and the user's
//! patches, one file each in `~/.config/synthbass/patches/`.

use std::path::{Path, PathBuf};

use synthbass_engine::Patch;

/// The built-in patch files. The first is what a first launch plays.
const BUILT_IN: [(&str, &str); 6] = [
    ("pluck", include_str!("../patches/pluck.toml")),
    ("fat_sub", include_str!("../patches/fat_sub.toml")),
    ("squelch", include_str!("../patches/squelch.toml")),
    ("detuned_saw", include_str!("../patches/detuned_saw.toml")),
    ("soft_round", include_str!("../patches/soft_round.toml")),
    ("octave_lead", include_str!("../patches/octave_lead.toml")),
];

pub struct PatchLibrary {
    built_in: Vec<Patch>,
    /// `None` when the system has no config directory: user patches can't be
    /// kept, built-in ones still play.
    dir: Option<PathBuf>,
    /// Sorted by name; names are unique.
    user: Vec<UserPatch>,
    warnings: Vec<String>,
}

struct UserPatch {
    patch: Patch,
    path: PathBuf,
}

impl PatchLibrary {
    /// Loads the built-in patches and every `*.toml` file in `dir`. A file
    /// that can't be read is skipped with a warning; nothing here fails.
    pub fn open(dir: Option<PathBuf>) -> PatchLibrary {
        let built_in = BUILT_IN
            .iter()
            .map(|(file, text)| {
                Patch::from_toml(text, file).expect("built-in patches are valid TOML")
            })
            .collect();
        let mut library = PatchLibrary {
            built_in,
            dir,
            user: Vec::new(),
            warnings: Vec::new(),
        };
        library.read_dir();
        library
    }

    fn read_dir(&mut self) {
        let Some(dir) = &self.dir else { return };
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return,
            Err(err) => {
                self.warnings.push(format!(
                    "Could not read patches in {}: {err}",
                    dir.display()
                ));
                return;
            }
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(|entry| Some(entry.ok()?.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "toml"))
            .collect();
        paths.sort();
        for path in paths {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            let patch = std::fs::read_to_string(&path)
                .map_err(|err| err.to_string())
                .and_then(|text| Patch::from_toml(&text, &stem));
            let skip = |why: String| format!("Skipped patch file {}: {why}", path.display());
            match patch {
                Err(err) => self.warnings.push(skip(err)),
                Ok(patch) if self.is_built_in(&patch.name) => self.warnings.push(skip(format!(
                    "\"{}\" is the name of a built-in patch",
                    patch.name
                ))),
                Ok(patch) if self.find_user(&patch.name).is_some() => self.warnings.push(skip(
                    format!("another file already holds \"{}\"", patch.name),
                )),
                Ok(patch) => self.insert(UserPatch { patch, path }),
            }
        }
    }

    /// Built-in patches, in the order the picker shows them.
    pub fn built_in(&self) -> &[Patch] {
        &self.built_in
    }

    /// User patches, sorted by name.
    pub fn user(&self) -> impl Iterator<Item = &Patch> {
        self.user.iter().map(|user| &user.patch)
    }

    /// A patch by its exact name, built-in or user.
    pub fn find(&self, name: &str) -> Option<&Patch> {
        self.built_in
            .iter()
            .chain(self.user())
            .find(|patch| patch.name == name)
    }

    /// What a first launch plays.
    pub fn default_patch(&self) -> &Patch {
        &self.built_in[0]
    }

    /// Problems met while reading the patches directory.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Built-in names are reserved whatever their case, so a user patch can
    /// never be mistaken for one.
    pub fn is_built_in(&self, name: &str) -> bool {
        self.built_in
            .iter()
            .any(|patch| patch.name.eq_ignore_ascii_case(name.trim()))
    }

    fn find_user(&self, name: &str) -> Option<usize> {
        self.user.iter().position(|user| user.patch.name == name)
    }

    fn insert(&mut self, user: UserPatch) {
        let at = self
            .user
            .partition_point(|other| sort_key(&other.patch.name) < sort_key(&user.patch.name));
        self.user.insert(at, user);
    }

    /// Writes a user patch, replacing any user patch with the same name.
    pub fn save(&mut self, patch: &Patch) -> Result<(), PatchError> {
        if self.is_built_in(&patch.name) {
            return Err(PatchError::BuiltIn);
        }
        if patch.name.trim().is_empty() || patch.name.trim() != patch.name {
            return Err(PatchError::BadName);
        }
        let Some(dir) = &self.dir else {
            return Err(PatchError::NoConfigDir);
        };
        let existing = self.find_user(&patch.name);
        let path = match existing {
            Some(index) => self.user[index].path.clone(),
            None => self.free_path(dir, &patch.name),
        };
        write_atomically(&path, &patch.to_toml())
            .map_err(|err| PatchError::Io(format!("{}: {err}", path.display())))?;
        let user = UserPatch {
            patch: patch.clone(),
            path,
        };
        match existing {
            Some(index) => self.user[index] = user,
            None => self.insert(user),
        }
        Ok(())
    }

    /// Removes a user patch and its file.
    pub fn delete(&mut self, name: &str) -> Result<(), PatchError> {
        if self.is_built_in(name) {
            return Err(PatchError::BuiltIn);
        }
        let index = self.find_user(name).ok_or(PatchError::NotFound)?;
        let path = &self.user[index].path;
        match std::fs::remove_file(path) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(PatchError::Io(format!("{}: {err}", path.display()))),
        }
        self.user.remove(index);
        Ok(())
    }

    /// A file name for a new patch that no other file uses.
    fn free_path(&self, dir: &Path, name: &str) -> PathBuf {
        let stem = file_stem(name);
        (1..)
            .map(|n| match n {
                1 => dir.join(format!("{stem}.toml")),
                n => dir.join(format!("{stem} {n}.toml")),
            })
            .find(|path| !path.exists() && self.user.iter().all(|user| &user.path != path))
            .expect("some numbered name is free")
    }
}

fn sort_key(name: &str) -> String {
    name.to_lowercase()
}

/// The patch name with anything a file system might dislike replaced.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if stem.is_empty() {
        "patch".into()
    } else {
        stem
    }
}

/// Temp file, then rename, so a crash mid-write never leaves half a patch.
fn write_atomically(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("toml.tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)
}

/// `~/.config/synthbass/patches`, or `None` if the system has no config
/// directory.
pub fn default_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("synthbass").join("patches"))
}

#[derive(Debug, PartialEq)]
pub enum PatchError {
    BuiltIn,
    NotFound,
    /// Empty, or with spaces at either end.
    BadName,
    NoConfigDir,
    Io(String),
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PatchError::BuiltIn => write!(f, "built-in patches can't be changed"),
            PatchError::NotFound => write!(f, "no such patch"),
            PatchError::BadName => write!(f, "a patch needs a name"),
            PatchError::NoConfigDir => write!(f, "no config directory to keep patches in"),
            PatchError::Io(err) => write!(f, "file error: {err}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthbass_engine::Param;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "synthbass-patches-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("patches")
    }

    fn user(library: &PatchLibrary) -> Vec<Patch> {
        library.user().cloned().collect()
    }

    fn my_patch(name: &str) -> Patch {
        let mut patch = Patch::new(name);
        patch.set(Param::TrackedCutoff, 777.0);
        patch.set(Param::TrackedOsc2Octave, 1.0);
        patch
    }

    #[test]
    fn a_saved_patch_is_listed_after_reopening() {
        let dir = temp_dir("save");
        let mut library = PatchLibrary::open(Some(dir.clone()));
        assert!(user(&library).is_empty());
        library.save(&my_patch("Mine")).unwrap();
        library.save(&my_patch("Another / one")).unwrap();
        assert_eq!(
            user(&library),
            [my_patch("Another / one"), my_patch("Mine")]
        );

        let reopened = PatchLibrary::open(Some(dir));
        assert_eq!(
            user(&reopened),
            [my_patch("Another / one"), my_patch("Mine")]
        );
        assert!(reopened.warnings().is_empty());
    }

    #[test]
    fn saving_a_user_patch_again_overwrites_it() {
        let dir = temp_dir("overwrite");
        let mut library = PatchLibrary::open(Some(dir.clone()));
        library.save(&my_patch("Mine")).unwrap();
        let mut refined = my_patch("Mine");
        refined.set(Param::TrackedResonance, 0.9);
        library.save(&refined).unwrap();
        assert_eq!(user(&library), [refined.clone()]);
        assert_eq!(user(&PatchLibrary::open(Some(dir))), [refined]);
    }

    #[test]
    fn a_corrupt_file_is_reported_and_skipped() {
        let dir = temp_dir("corrupt");
        let mut library = PatchLibrary::open(Some(dir.clone()));
        library.save(&my_patch("Good")).unwrap();
        std::fs::write(dir.join("broken.toml"), "[direct\ncutoff_hz = = 3\0").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a patch").unwrap();

        let reopened = PatchLibrary::open(Some(dir));
        assert_eq!(user(&reopened), [my_patch("Good")]);
        assert_eq!(reopened.warnings().len(), 1, "{:?}", reopened.warnings());
        assert!(reopened.warnings()[0].contains("broken.toml"));
    }

    #[test]
    fn built_in_patches_cannot_be_overwritten_or_deleted() {
        let dir = temp_dir("built-in");
        let mut library = PatchLibrary::open(Some(dir.clone()));
        for name in ["Pluck", "pluck", " Fat Sub "] {
            assert_eq!(library.save(&my_patch(name)), Err(PatchError::BuiltIn));
            assert_eq!(library.delete(name), Err(PatchError::BuiltIn));
        }
        assert_eq!(
            library.built_in()[0],
            PatchLibrary::open(None).built_in()[0]
        );
        assert!(user(&library).is_empty());
        assert!(!dir.exists(), "nothing was written");
    }

    #[test]
    fn a_deleted_patch_is_gone_after_reopening() {
        let dir = temp_dir("delete");
        let mut library = PatchLibrary::open(Some(dir.clone()));
        library.save(&my_patch("Keep")).unwrap();
        library.save(&my_patch("Drop")).unwrap();
        library.delete("Drop").unwrap();
        assert_eq!(user(&library), [my_patch("Keep")]);
        assert_eq!(user(&PatchLibrary::open(Some(dir))), [my_patch("Keep")]);
        assert_eq!(library.delete("Drop"), Err(PatchError::NotFound));
    }

    #[test]
    fn patches_are_found_by_name_whether_built_in_or_user() {
        let mut library = PatchLibrary::open(Some(temp_dir("find")));
        library.save(&my_patch("Mine")).unwrap();
        assert_eq!(library.find("Mine"), Some(&my_patch("Mine")));
        assert_eq!(
            library.find("Squelch").map(|p| p.name.as_str()),
            Some("Squelch")
        );
        assert_eq!(library.find("Gone"), None);
        assert_eq!(library.default_patch().name, "Pluck");
    }

    #[test]
    fn a_patch_needs_a_name() {
        let mut library = PatchLibrary::open(Some(temp_dir("names")));
        for name in ["", "   ", " padded"] {
            assert_eq!(library.save(&my_patch(name)), Err(PatchError::BadName));
        }
    }

    #[test]
    fn every_built_in_patch_loads_with_its_own_name_and_audible_layers() {
        let library = PatchLibrary::open(None);
        let names: Vec<&str> = library.built_in().iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Pluck",
                "Fat Sub",
                "Squelch",
                "Detuned Saw",
                "Soft Round",
                "Octave Lead"
            ]
        );
        for patch in library.built_in() {
            // Both layers default to silent, so a patch that forgot a level
            // would play nothing but dry bass.
            assert!(patch.get(Param::DirectLevel) > 0.0, "{}", patch.name);
            assert!(patch.get(Param::TrackedLevel) > 0.0, "{}", patch.name);
        }
    }

    #[test]
    fn every_built_in_file_spells_out_every_parameter() {
        // A misspelt or missing key would silently play the default.
        let library = PatchLibrary::open(None);
        for ((file, text), patch) in BUILT_IN.iter().zip(library.built_in()) {
            let written: toml::Table = toml::from_str(text).unwrap();
            let expected: toml::Table = toml::from_str(&patch.to_toml()).unwrap();
            assert_eq!(written, expected, "{file}.toml");
        }
    }
}
