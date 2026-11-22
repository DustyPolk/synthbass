//! Offline rendering: plays a patch over a recorded dry signal, as the app
//! would live, and measures the result against the dry signal.

use synthbass_engine::{Engine, NoiseGate, Patch};

/// The block size the app processes in at its requested latency.
const BLOCK: usize = 128;

/// Silence played before the clip, so the patch's levels finish gliding in
/// from the engine's defaults, as they have live long before a note.
const SETTLE_SECONDS: f32 = 0.5;

/// `patch` played over `dry` with the default noise gate, after the patch
/// has settled: one output channel (the two are identical).
pub fn render(patch: &Patch, dry: &[f32], sample_rate: f32) -> Vec<f32> {
    let mut engine = Engine::new(sample_rate, BLOCK);
    engine.apply_patch(patch);
    engine.set_noise_gate(NoiseGate::default());
    let (mut left, mut right) = ([0.0; BLOCK], [0.0; BLOCK]);
    let silence = [0.0; BLOCK];
    for _ in 0..(SETTLE_SECONDS * sample_rate) as usize / BLOCK {
        engine.process(&silence, &mut left, &mut right);
    }
    let mut out = vec![0.0; dry.len()];
    for (dry_block, out_block) in dry.chunks(BLOCK).zip(out.chunks_mut(BLOCK)) {
        engine.process(dry_block, out_block, &mut right[..dry_block.len()]);
    }
    out
}

/// How a rendered signal compares with the dry signal it was made from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Measures {
    /// RMS level against the dry signal's, in dB.
    pub loudness_db: f32,
    /// Largest absolute sample.
    pub peak: f32,
    /// How closely the waveform follows the dry signal's, from 0 (unrelated)
    /// to 1 (the dry signal, scaled). Above about 0.8 it sounds like the dry
    /// signal.
    pub similarity_to_dry: f32,
}

pub fn measure(dry: &[f32], out: &[f32], sample_rate: f32) -> Measures {
    Measures {
        loudness_db: 20.0 * (rms(out) / rms(dry).max(1e-12)).log10(),
        peak: out.iter().fold(0.0, |max, s| max.max(s.abs())),
        similarity_to_dry: similarity(dry, out, (MAX_LAG_SECONDS * sample_rate) as usize),
    }
}

/// How far behind the dry signal the output may run and still count as the
/// same waveform: filters shift phase by a few milliseconds at bass pitches.
const MAX_LAG_SECONDS: f32 = 0.005;

/// The best normalised correlation of `out` against `dry` over lags from 0
/// to `max_lag` samples.
fn similarity(dry: &[f32], out: &[f32], max_lag: usize) -> f32 {
    let len = dry.len().min(out.len());
    (0..max_lag.min(len))
        .map(|lag| {
            let (out, dry) = (&out[lag..len], &dry[..len - lag]);
            let dot: f32 = out.iter().zip(dry).map(|(a, b)| a * b).sum();
            dot / (rms(out) * rms(dry) * out.len() as f32).max(1e-12)
        })
        .fold(0.0, f32::max)
}

fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|s| s * s).sum::<f32>() / signal.len().max(1) as f32).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use synthbass_engine::Param;

    fn sine(freq: f32, amplitude: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|n| amplitude * (std::f32::consts::TAU * freq * n as f32 / 48_000.0).sin())
            .collect()
    }

    #[test]
    fn twice_the_dry_signal_is_six_db_louder_with_twice_the_peak() {
        let dry = sine(55.0, 0.1, 48_000);
        let out: Vec<f32> = dry.iter().map(|s| 2.0 * s).collect();
        let measures = measure(&dry, &out, 48_000.0);
        assert!((measures.loudness_db - 6.02).abs() < 0.01, "{measures:?}");
        assert!((measures.peak - 0.2).abs() < 1e-4, "{measures:?}");
    }

    #[test]
    fn a_delayed_quieter_copy_of_the_dry_signal_is_fully_similar_to_it() {
        // Filters shift phase, so a few milliseconds of lag still count.
        let dry: Vec<f32> = sine(55.0, 0.1, 48_000)
            .iter()
            .zip(sine(165.0, 0.05, 48_000))
            .map(|(a, b)| a + b)
            .collect();
        let mut out = vec![0.0; 100];
        out.extend(dry.iter().map(|s| 0.3 * s));
        out.truncate(dry.len());
        let similarity = measure(&dry, &out, 48_000.0).similarity_to_dry;
        assert!(similarity > 0.99, "{similarity}");
    }

    #[test]
    fn a_signal_unrelated_to_the_dry_signal_is_not_similar_to_it() {
        let dry = sine(55.0, 0.1, 48_000);
        let out = sine(97.0, 0.1, 48_000);
        let similarity = measure(&dry, &out, 48_000.0).similarity_to_dry;
        assert!(similarity < 0.05, "{similarity}");
    }

    #[test]
    fn a_patch_with_only_the_dry_signal_at_half_level_renders_half_the_dry_signal() {
        let dry = sine(55.0, 0.1, 48_000);
        let mut patch = Patch::new("Half dry");
        patch.set(Param::DryLevel, 0.5);
        patch.set(Param::DirectLevel, 0.0);
        patch.set(Param::TrackedLevel, 0.0);
        let out = render(&patch, &dry, 48_000.0);
        assert_eq!(out.len(), dry.len());
        let measures = measure(&dry, &out, 48_000.0);
        assert!((measures.loudness_db + 6.02).abs() < 0.1, "{measures:?}");
        assert!(measures.similarity_to_dry > 0.99, "{measures:?}");
    }
}
