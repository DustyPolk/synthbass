//! Offline harness: drives the engine over a signal in block-sized chunks.

#![allow(dead_code)]

use synthbass_engine::Engine;

pub const SAMPLE_RATE: f32 = 48_000.0;
pub const MAX_BLOCK: usize = 1024;

pub fn engine() -> Engine {
    Engine::new(SAMPLE_RATE, MAX_BLOCK)
}

pub struct Rendered {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Processes `dry` through `engine`, cycling through `block_sizes` for the
/// chunk lengths.
pub fn render(engine: &mut Engine, dry: &[f32], block_sizes: &[usize]) -> Rendered {
    let mut left = vec![0.0; dry.len()];
    let mut right = vec![0.0; dry.len()];
    let mut start = 0;
    for &size in block_sizes.iter().cycle() {
        if start == dry.len() {
            break;
        }
        let end = (start + size).min(dry.len());
        engine.process(
            &dry[start..end],
            &mut left[start..end],
            &mut right[start..end],
        );
        start = end;
    }
    Rendered { left, right }
}

pub fn sine(freq: f32, amplitude: f32, seconds: f32) -> Vec<f32> {
    let len = (seconds * SAMPLE_RATE) as usize;
    (0..len)
        .map(|n| amplitude * (std::f32::consts::TAU * freq * n as f32 / SAMPLE_RATE).sin())
        .collect()
}

pub fn peak(signal: &[f32]) -> f32 {
    signal.iter().fold(0.0, |max, s| max.max(s.abs()))
}
pub mod clip;
