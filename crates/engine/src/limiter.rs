/// Hard limiter at the end of the chain: output never exceeds the ceiling.
///
/// Gain drops instantly to whatever keeps the current sample under the
/// ceiling, then recovers smoothly once the peak passes.
pub struct Limiter {
    gain: f32,
    release: f32,
}

/// The highest output magnitude the engine produces (-1 dBFS).
pub const LIMITER_CEILING: f32 = 0.891;

const RELEASE_SECONDS: f32 = 0.1;

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        Limiter {
            gain: 1.0,
            release: (-1.0 / (RELEASE_SECONDS * sample_rate)).exp(),
        }
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        let recovered = 1.0 - (1.0 - self.gain) * self.release;
        let needed = LIMITER_CEILING / sample.abs().max(f32::MIN_POSITIVE);
        self.gain = recovered.min(needed);
        (sample * self.gain).clamp(-LIMITER_CEILING, LIMITER_CEILING)
    }
}
