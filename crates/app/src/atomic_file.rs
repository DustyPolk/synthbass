//! Saving files so a crash mid-write never leaves half a file behind.

use std::path::Path;

/// Writes `text` to `path` through a temp file and a rename, creating the
/// directory if needed.
pub fn write(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let temp = path.with_extension("toml.tmp");
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path)
}
