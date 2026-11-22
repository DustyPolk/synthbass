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

/// MIDI note number to frequency in Hz.
pub fn hz(note: f32) -> f32 {
    synthbass_engine::midi_to_hz(note)
}

/// A plucked-bass-like tone: eight harmonics at 1/h amplitude with fixed
/// pseudo-random phases, decaying exponentially, scaled so the first peak
/// is close to `amplitude`. `fundamental` scales the first harmonic (real
/// basses often have a weak fundamental).
pub fn bass_tone(freq: f32, amplitude: f32, fundamental: f32, seconds: f32) -> Vec<f32> {
    let mut seed = 0x2545_f491_u32;
    let phases: Vec<f32> = (0..8)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32 * std::f32::consts::TAU
        })
        .collect();
    let gains: Vec<f32> = (1..=8)
        .map(|h| if h == 1 { fundamental } else { 1.0 / h as f32 })
        .collect();
    let len = (seconds * SAMPLE_RATE) as usize;
    let raw: Vec<f32> = (0..len)
        .map(|n| {
            let t = n as f32 / SAMPLE_RATE;
            let tone: f32 = (0..8)
                .filter(|&i| freq * (i + 1) as f32 * 2.0 < SAMPLE_RATE)
                .map(|i| {
                    let w = std::f32::consts::TAU * freq * (i + 1) as f32;
                    gains[i] * (w * t + phases[i]).sin()
                })
                .sum();
            tone * (-t / 1.5).exp()
        })
        .collect();
    let first_period = (SAMPLE_RATE / freq) as usize * 2;
    let scale = amplitude / peak(&raw[..first_period.min(raw.len())]);
    raw.into_iter().map(|s| s * scale).collect()
}
