//! Saving files so a crash mid-write never leaves half a file behind.

use std::path::Path;

/// Writes `text` to `path` through a temp file and a rename, creating the
/// directory if needed.
pub fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)
}

#[cfg(test)]
mod tests {
    #[test]
    fn writing_a_file_leaves_other_files_sharing_its_stem_alone() {
        let dir =
            std::env::temp_dir().join(format!("synthbass-test-{}-atomic-stem", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let other = dir.join("a.toml.tmp");
        std::fs::write(&other, "someone else's").unwrap();

        super::write(&dir.join("a.json"), "{}").unwrap();

        assert_eq!(std::fs::read_to_string(dir.join("a.json")).unwrap(), "{}");
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "someone else's");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
