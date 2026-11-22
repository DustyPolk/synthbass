//! Every recorded clip in `clips/` loads: its WAV reads and its `.notes` parses.
mod common;
use common::clip::{clip_path, load_clip};

#[test]
fn every_recorded_clip_loads() {
    let dir = clip_path("x").parent().unwrap().to_path_buf();
    let mut n = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "wav") {
            let clip = load_clip(&path);
            assert!(!clip.expected.events.is_empty(), "{} has no events", path.display());
            n += 1;
        }
    }
    assert!(n >= 7, "expected the recorded clip set, found {n} clips");
}
