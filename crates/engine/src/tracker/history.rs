//! Recent analysis-rate signal, for checking a candidate note's periodicity
//! in the time domain. Periodicity uses every harmonic at once, so it pins a
//! low note down within about two periods, sooner than any one band can
//! separate its partials.

use super::F0_RANGE;

/// Holds two periods of the lowest note plus slack.
const CAPACITY: usize = 512;
/// Lags searched either side of a candidate's period, in semitones.
const SEARCH_SEMITONES: f32 = 1.5;
/// Shortest comparison window in samples (10 ms at 6 kHz), so high notes
/// still average over several periods.
const MIN_WINDOW: usize = 60;

pub struct History {
    samples: [f32; CAPACITY],
    pos: usize,
}

pub struct Periodicity {
    /// Refined fundamental in Hz.
    pub freq: f32,
    /// Normalised square difference function peak: 1.0 is perfectly
    /// periodic at `freq`, 0 is no relation.
    pub clarity: f32,
    /// False until the signal has been there for the whole comparison span
    /// (about two periods): before that, nothing can be said.
    pub judged: bool,
}

/// The older half of the span must carry at least this fraction of the
/// newer half's energy for the comparison to mean anything.
const JUDGED_ENERGY: f32 = 0.25;

impl History {
    pub fn new() -> Self {
        History {
            samples: [0.0; CAPACITY],
            pos: 0,
        }
    }

    pub fn push(&mut self, x: f32) {
        self.pos = (self.pos + 1) % CAPACITY;
        self.samples[self.pos] = x;
    }

    fn at(&self, age: usize) -> f32 {
        self.samples[(self.pos + CAPACITY - age) % CAPACITY]
    }

    /// Finds the period near `f0` over one period's window (McLeod & Wyvill's
    /// NSDF, restricted to lags within ±1.5 semitones of `f0`).
    pub fn periodicity(&self, f0: f32, rate: f32) -> Periodicity {
        let period = rate / f0;
        let spread = 2f32.powf(SEARCH_SEMITONES / 12.0);
        let shortest = ((period / spread).floor() as usize).max(2);
        let longest = (period * spread).ceil() as usize;
        let window = (period.round() as usize).max(MIN_WINDOW);
        let longest = longest
            .min(CAPACITY - window - 2)
            .min((rate / F0_RANGE.0).floor() as usize)
            .max(shortest);

        let energies = |lag: usize| {
            let (mut diff, mut recent, mut older) = (0.0, 0.0, 0.0);
            for j in 0..window {
                let a = self.at(j);
                let b = self.at(j + lag);
                diff += (a - b) * (a - b);
                recent += a * a;
                older += b * b;
            }
            (diff, recent, older)
        };
        let nsdf = |lag: usize| {
            let (diff, recent, older) = energies(lag);
            let energy = recent + older;
            if energy > 0.0 {
                1.0 - diff / energy
            } else {
                0.0
            }
        };

        let mut best_lag = shortest;
        let mut best = f32::MIN;
        for lag in shortest..=longest {
            let value = nsdf(lag);
            if value > best {
                best = value;
                best_lag = lag;
            }
        }
        // Parabolic interpolation for a fractional period.
        let mut lag = best_lag as f32;
        if best_lag > shortest && best_lag < longest {
            let (left, right) = (nsdf(best_lag - 1), nsdf(best_lag + 1));
            let curvature = left - 2.0 * best + right;
            if curvature < 0.0 {
                lag += 0.5 * (left - right) / curvature;
            }
        }
        let (_, recent, older) = energies(best_lag);
        Periodicity {
            freq: rate / lag,
            clarity: best,
            judged: recent > 0.0 && older >= JUDGED_ENERGY * recent,
        }
    }
}
