//! Brings the dry signal down to the analysis rate (≈ 6 kHz): the bank only
//! needs content up to the harmonics of the top note, and its resonators
//! are better conditioned at a low rate.

/// Target analysis rate; the actual rate is the sample rate divided by the
/// nearest whole factor.
const ANALYSIS_RATE: f32 = 6_000.0;
/// Removes rumble below the lowest string.
const HIGHPASS_HZ: f32 = 30.0;
/// Keeps everything up to twice the top note, removes what would alias.
const LOWPASS_HZ: f32 = 1_100.0;

pub struct Decimator {
    factor: usize,
    phase: usize,
    highpass: Biquad,
    lowpass: [Biquad; 2],
}

impl Decimator {
    pub fn new(sample_rate: f32) -> Self {
        // Fourth-order Butterworth as two second-order sections.
        let butterworth_q = [0.541_196_1, 1.306_563];
        Decimator {
            factor: (sample_rate / ANALYSIS_RATE).round().max(1.0) as usize,
            phase: 0,
            highpass: Biquad::highpass(sample_rate, HIGHPASS_HZ, std::f32::consts::FRAC_1_SQRT_2),
            lowpass: butterworth_q.map(|q| Biquad::lowpass(sample_rate, LOWPASS_HZ, q)),
        }
    }

    pub fn factor(&self) -> usize {
        self.factor
    }

    /// Filters one input sample; returns an analysis-rate sample every
    /// `factor` inputs.
    pub fn process(&mut self, sample: f32) -> Option<f32> {
        let mut y = self.highpass.process(sample);
        for section in &mut self.lowpass {
            y = section.process(y);
        }
        self.phase += 1;
        if self.phase == self.factor {
            self.phase = 0;
            Some(y)
        } else {
            None
        }
    }
}

/// Second-order section, transposed direct form II (RBJ cookbook).
struct Biquad {
    b: [f32; 3],
    a: [f32; 2],
    s: [f32; 2],
}

impl Biquad {
    fn lowpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let (cos, alpha) = Self::prewarp(sample_rate, freq, q);
        Self::normalised(
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            cos,
            alpha,
        )
    }

    fn highpass(sample_rate: f32, freq: f32, q: f32) -> Self {
        let (cos, alpha) = Self::prewarp(sample_rate, freq, q);
        Self::normalised(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            cos,
            alpha,
        )
    }

    fn prewarp(sample_rate: f32, freq: f32, q: f32) -> (f32, f32) {
        let w = std::f32::consts::TAU * freq / sample_rate;
        (w.cos(), w.sin() / (2.0 * q))
    }

    fn normalised(b: [f32; 3], cos: f32, alpha: f32) -> Self {
        let a0 = 1.0 + alpha;
        Biquad {
            b: b.map(|c| c / a0),
            a: [-2.0 * cos / a0, (1.0 - alpha) / a0],
            s: [0.0; 2],
        }
    }

    fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.s[0];
        self.s[0] = self.b[1] * x - self.a[0] * y + self.s[1];
        self.s[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}
