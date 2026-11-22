//! The semitone filterbank: one complex resonator per semitone, each giving
//! a magnitude and an instantaneous frequency (IF).
//!
//! Bands at the bottom are wider than a semitone so they respond within
//! about two periods; exact pitch comes from the IF, which every band near a
//! partial agrees on, not from which band is loudest.

/// Lowest and highest band centres as MIDI notes: D1 (below low E) to G5
/// (twice the top of the G string, so the sieve sees its harmonics).
pub const LOWEST_BAND: i32 = 26;
pub const BAND_COUNT: usize = 54;
/// Bandwidth floor in Hz: the latency/separation trade-off. Wider bands
/// respond faster on low notes but separate close low notes later.
const MIN_BANDWIDTH_HZ: f32 = 12.0;
/// Above the floor, bands are about a semitone wide.
const BAND_Q: f32 = 17.0;
/// Lowest fundamental whose period the IF can be averaged over.
pub const LOWEST_F0_HZ: f32 = 37.0;

#[derive(Clone, Copy, Default)]
struct Complex {
    re: f32,
    im: f32,
}

impl Complex {
    fn mul(self, o: Complex) -> Complex {
        Complex {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }

    fn conj(self) -> Complex {
        Complex {
            re: self.re,
            im: -self.im,
        }
    }

    fn scale(self, k: f32) -> Complex {
        Complex {
            re: self.re * k,
            im: self.im * k,
        }
    }

    fn add(self, o: Complex) -> Complex {
        Complex {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }
}

struct Band {
    pole: Complex,
    gain: f32,
    stage_1: Complex,
    stage_2: Complex,
    /// Its own period in analysis samples: the IF window for picking.
    period: usize,
}

pub struct Bank {
    rate: f32,
    bands: Vec<Band>,
    /// Lag-1 products `z[n]·conj(z[n−1])` per band, a ring of `ring_len`.
    products: Vec<Complex>,
    ring_len: usize,
    pos: usize,
}

pub fn midi_to_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}

impl Bank {
    pub fn new(rate: f32) -> Self {
        let bands = (0..BAND_COUNT)
            .map(|k| {
                let centre_hz = midi_to_hz((LOWEST_BAND + k as i32) as f32);
                let bandwidth = (centre_hz / BAND_Q).max(MIN_BANDWIDTH_HZ);
                let r = (-std::f32::consts::PI * bandwidth / rate).exp();
                let w = std::f32::consts::TAU * centre_hz / rate;
                Band {
                    pole: Complex {
                        re: r * w.cos(),
                        im: r * w.sin(),
                    },
                    gain: 1.0 - r,
                    stage_1: Complex::default(),
                    stage_2: Complex::default(),
                    period: (rate / centre_hz).round().max(1.0) as usize,
                }
            })
            .collect();
        let ring_len = (rate / LOWEST_F0_HZ).ceil() as usize + 1;
        Bank {
            rate,
            bands,
            products: vec![Complex::default(); BAND_COUNT * ring_len],
            ring_len,
            pos: 0,
        }
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn process(&mut self, x: f32) {
        self.pos = (self.pos + 1) % self.ring_len;
        for (k, band) in self.bands.iter_mut().enumerate() {
            let previous = band.stage_2;
            band.stage_1 = band.pole.mul(band.stage_1).add(Complex {
                re: band.gain * x,
                im: 0.0,
            });
            band.stage_2 = band
                .pole
                .mul(band.stage_2)
                .add(band.stage_1.scale(band.gain));
            self.products[k * self.ring_len + self.pos] = band.stage_2.mul(previous.conj());
        }
    }

    /// Amplitude of a real sinusoid at the band centre that would give this
    /// band's current output (each stage has unity gain at the centre for
    /// the positive-frequency half, so a real sinusoid reads half).
    pub fn amplitude(&self, band: usize) -> f32 {
        let z = self.bands[band].stage_2;
        2.0 * (z.re * z.re + z.im * z.im).sqrt()
    }

    /// `amplitude` of the band nearest `hz`, or 0 outside the bank.
    pub fn amplitude_at(&self, hz: f32) -> f32 {
        let band = hz_to_midi(hz).round() as i32 - LOWEST_BAND;
        if (0..BAND_COUNT as i32).contains(&band) {
            self.amplitude(band as usize)
        } else {
            0.0
        }
    }

    /// IF averaged over the band's own period, in Hz.
    pub fn frequency(&self, band: usize) -> f32 {
        self.frequency_over(band, self.bands[band].period)
    }

    /// IF averaged over the last `window` analysis samples, in Hz. Averaging
    /// over exactly one period of a note's fundamental cancels the ripple its
    /// other harmonics leak into the band.
    pub fn frequency_over(&self, band: usize, window: usize) -> f32 {
        let ring = &self.products[band * self.ring_len..(band + 1) * self.ring_len];
        let window = window.clamp(1, self.ring_len);
        let mut sum = Complex::default();
        for i in 0..window {
            sum = sum.add(ring[(self.pos + self.ring_len - i) % self.ring_len]);
        }
        sum.im.atan2(sum.re) * self.rate / std::f32::consts::TAU
    }
}
