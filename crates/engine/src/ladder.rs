//! Moog-style 4-pole ladder lowpass, reusable wherever the engine needs one
//! (the direct layer, and each tracked voice).
//!
//! Zero-delay-feedback (TPT) ladder after Zavalishin, *The Art of VA Filter
//! Design*, ch. 5: four trapezoidal one-pole stages with the feedback loop
//! solved in closed form, and one `tanh` on the stage input that bounds
//! self-oscillation and gives the Moog "squash" when hit hard.

use std::f32::consts::PI;

/// Feedback gain at resonance 1.0. Just below the self-oscillation threshold
/// of 4, so a resonant ring always dies away when the playing stops.
const MAX_FEEDBACK: f32 = 3.9;

/// How much of the ladder's passband loss at high resonance is made up
/// (0 = none, the vintage thinning; 1 = flat passband).
const RESONANCE_COMPENSATION: f32 = 0.5;

/// State magnitudes below this are flushed to zero so a decaying filter
/// never lingers in the slow denormal range.
const FLUSH_BELOW: f32 = 1e-15;

pub struct LadderFilter {
    sample_rate: f32,
    stages: [f32; 4],
    /// Prewarped integrator gain `tan(π·fc/fs)`.
    g: f32,
    feedback: f32,
}

impl LadderFilter {
    pub fn new(sample_rate: f32) -> Self {
        let mut filter = LadderFilter {
            sample_rate,
            stages: [0.0; 4],
            g: 0.0,
            feedback: 0.0,
        };
        filter.set_cutoff(1_000.0);
        filter
    }

    /// Cutoff in Hz, clamped to 10 Hz – 0.45 × sample rate. Cheap enough to
    /// call every sample for audio-rate modulation.
    pub fn set_cutoff(&mut self, hz: f32) {
        let hz = hz.clamp(10.0, 0.45 * self.sample_rate);
        self.g = (PI * hz / self.sample_rate).tan();
    }

    /// Resonance 0..=1; 1 is the edge of self-oscillation.
    pub fn set_resonance(&mut self, resonance: f32) {
        self.feedback = MAX_FEEDBACK * resonance.clamp(0.0, 1.0);
    }

    /// Clears the filter's memory (e.g. when a silent voice starts a note).
    pub fn reset(&mut self) {
        self.stages = [0.0; 4];
    }

    pub fn process(&mut self, input: f32) -> f32 {
        let big_g = self.g / (1.0 + self.g);
        let one_minus_g = 1.0 - big_g;
        let [s1, s2, s3, s4] = self.stages;
        // The cascade's output is affine in its input u: y4 = G⁴·u + sigma.
        let sigma =
            (big_g * big_g * big_g * s1 + big_g * big_g * s2 + big_g * s3 + s4) * one_minus_g;
        let g4 = big_g * big_g * big_g * big_g;
        let k = self.feedback;
        let x = input * (1.0 + RESONANCE_COMPENSATION * k);
        let mut u = ((x - k * sigma) / (1.0 + k * g4)).tanh();
        for s in &mut self.stages {
            let v = (u - *s) * big_g;
            let y = v + *s;
            *s = y + v;
            if s.abs() < FLUSH_BELOW {
                *s = 0.0;
            }
            u = y;
        }
        if !u.is_finite() {
            self.reset();
            return 0.0;
        }
        u
    }
}
