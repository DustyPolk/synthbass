//! The semitone filterbank: one complex resonator per semitone, each giving
//! a magnitude and an instantaneous frequency (IF).
//!
//! Bands at the bottom are wider than a semitone so they respond within
//! about two periods; exact pitch comes from the IF, which every band near a
//! partial agrees on, not from which band is loudest.

use crate::pitch::{hz_to_midi, midi_to_hz};
use crate::smoothed::one_pole_coefficient;

/// Lowest and highest band centres as MIDI notes: D1 (below low E) to G5
/// (twice the top of the G string, so the sieve sees its harmonics).
pub const LOWEST_BAND: i32 = 26;
pub const BAND_COUNT: usize = 54;
/// Bandwidth floor in Hz: the latency/separation trade-off. Wider bands
/// respond faster on low notes but separate close low notes later.
const MIN_BANDWIDTH_HZ: f32 = 12.0;
/// Above the floor, bands are about a semitone wide.
const BAND_Q: f32 = 17.0;
/// Time constant of each of the two smoothing stages for the slow IF.
const SLOW_IF_SECONDS: f32 = 0.03;
/// A band whose amplitude grows by `RISE` within `RISE_HOPS` hops is
/// hearing a new note: its slow IF restarts.
const RISE: f32 = 1.5;
const RISE_HOPS: usize = 8;
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
    /// The lag-1 products smoothed by two one-poles: the slow IF.
    slow: [Complex; 2],
    /// Amplitude at each of the last `RISE_HOPS` hops.
    recent: [f32; RISE_HOPS],
    /// The slow IF restarted at the latest hop.
    restarted: bool,
}

pub struct Bank {
    rate: f32,
    bands: Vec<Band>,
    /// Lag-1 products `z[n]·conj(z[n−1])` per band, a ring of `ring_len`.
    products: Vec<Complex>,
    ring_len: usize,
    pos: usize,
    slow_coefficient: f32,
    recent_index: usize,
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
                    slow: [Complex::default(); 2],
                    recent: [0.0; RISE_HOPS],
                    restarted: false,
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
            recent_index: 0,
            slow_coefficient: 1.0 - one_pole_coefficient(SLOW_IF_SECONDS, rate),
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
            let product = band.stage_2.mul(previous.conj());
            self.products[k * self.ring_len + self.pos] = product;
            let k = self.slow_coefficient;
            let [first, second] = &mut band.slow;
            *first = first.add(product.add(first.scale(-1.0)).scale(k));
            *second = second.add(first.add(second.scale(-1.0)).scale(k));
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

    /// IF smoothed over about 60 ms, in Hz: slow, but it averages out the
    /// beating between a partial and a neighbour too close for the band to
    /// separate (the partials of a low third).
    pub fn slow_frequency(&self, band: usize) -> f32 {
        let z = self.bands[band].slow[1];
        z.im.atan2(z.re) * self.rate / std::f32::consts::TAU
    }

    /// Once per hop: a band hearing a new note forgets the old one, so its
    /// slow IF doesn't lag behind the note change.
    pub fn hop(&mut self) {
        for (k, band) in self.bands.iter_mut().enumerate() {
            let z = band.stage_2;
            let amp = 2.0 * (z.re * z.re + z.im * z.im).sqrt();
            let old = std::mem::replace(&mut band.recent[self.recent_index], amp);
            band.restarted = amp > RISE * old;
            if band.restarted {
                // From the mean product over the band's own period, which
                // has no ripple from the note's other harmonics.
                let ring = &self.products[k * self.ring_len..(k + 1) * self.ring_len];
                let mut sum = Complex::default();
                for i in 0..band.period {
                    sum = sum.add(ring[(self.pos + self.ring_len - i) % self.ring_len]);
                }
                band.slow = [sum.scale(1.0 / band.period as f32); 2];
            }
        }
        self.recent_index = (self.recent_index + 1) % RISE_HOPS;
    }

    /// True if the band's slow IF restarted at the latest hop.
    pub fn restarted(&self, band: usize) -> bool {
        self.bands[band].restarted
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
