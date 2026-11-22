//! Per-hop analysis: picks partials out of the bank and groups them into
//! notes with a harmonic sieve (Klapuri-style iterative cancellation).

use super::bank::{BAND_COUNT, Bank, LOWEST_BAND, hz_to_midi, midi_to_hz};
use super::history::History;
use super::{F0_RANGE, MAX_DETECTED_NOTES};

/// Partials quieter than this fraction of the gate are ignored outright.
const PARTIAL_FLOOR: f32 = 0.5;
/// How close (in semitones) a partial must be to a harmonic to count.
const HARMONIC_TOLERANCE: f32 = 0.4;
/// Harmonics a note collects; harmonic h adds its amplitude / √h to the
/// note's salience.
const HARMONICS: usize = 6;
/// A candidate whose signal is less periodic than this at its period is
/// noise. Kept low: in a chord every note's periodicity is disturbed by the
/// others.
const MIN_CLARITY: f32 = 0.1;
/// Lower fundamentals a candidate could be a harmonic of: f0/2 … f0/8.
const OWNER_HARMONICS: usize = 8;
/// A possible owner with at least this fraction of the candidate's
/// fundamental level in the bank is worth checking.
const OWNER_ENERGY: f32 = 0.3;
/// "Lower" means more than a semitone below.
const LOWER: f32 = 0.943;
/// A candidate is a harmonic if its owner is this much more periodic.
const OWNER_MARGIN: f32 = 0.15;
/// Once the bands' IF agrees with the periodicity estimate this closely
/// (semitones) the bands have settled, and their IF is the more exact.
const SETTLED: f32 = 0.5;
/// A note weaker than this fraction of the strongest one is a ghost.
const RELATIVE_FLOOR: f32 = 0.15;

#[derive(Clone, Copy, Default)]
struct Partial {
    band: usize,
    freq: f32,
    amp: f32,
    used: bool,
}

/// A note as heard in one hop, before tracking over time.
#[derive(Clone, Copy, Default, Debug)]
pub struct RawNote {
    /// Fractional MIDI pitch.
    pub pitch: f32,
    pub level: f32,
}

pub struct Sieve {
    partials: [Partial; BAND_COUNT],
    partial_count: usize,
    partial_of_band: [Option<u8>; BAND_COUNT],
    candidates: [f32; 2 * BAND_COUNT],
    candidate_count: usize,
}

struct Support {
    salience: f32,
    harmonic: [Option<usize>; HARMONICS],
}

impl Sieve {
    pub fn new() -> Self {
        Sieve {
            partials: [Partial::default(); BAND_COUNT],
            partial_count: 0,
            partial_of_band: [None; BAND_COUNT],
            candidates: [0.0; 2 * BAND_COUNT],
            candidate_count: 0,
        }
    }

    /// Finds the notes in the bank's current state, strongest first.
    pub fn analyse(
        &mut self,
        bank: &Bank,
        history: &History,
        gate: f32,
        out: &mut [RawNote; MAX_DETECTED_NOTES],
    ) -> usize {
        self.pick_partials(bank, gate);
        self.collect_candidates();

        let mut count = 0;
        let mut strongest = 0.0;
        while count < MAX_DETECTED_NOTES {
            let Some((index, support)) = self.best_candidate() else {
                break;
            };
            let f0 = self.candidates[index];
            self.candidates[index] = 0.0;
            let [h1, h2, h3, ..] = support.harmonic;
            // Ghosts an octave or a twelfth below a real note get support from
            // its harmonics but have nothing at their own odd harmonics.
            if h1.is_none() && (h2.is_none() || h3.is_none()) {
                continue;
            }
            if support.salience < RELATIVE_FLOOR * strongest {
                break;
            }
            let periodicity = history.periodicity(f0, bank.rate());
            if !periodicity.judged
                || periodicity.clarity < MIN_CLARITY
                || self.outranked_from_below(bank, history, f0, periodicity.clarity)
            {
                continue;
            }
            let pitch = hz_to_midi(periodicity.freq);
            // Two candidates can settle on the same period.
            let duplicate = out[..count].iter().any(|n| (n.pitch - pitch).abs() < 0.5);
            strongest = f32::max(strongest, support.salience);

            let mut energy = 0.0;
            for &p in support.harmonic.iter().flatten() {
                self.partials[p].used = true;
                energy += self.partials[p].amp * self.partials[p].amp;
            }
            let level = energy.sqrt();
            if duplicate || level < gate {
                continue;
            }
            out[count] = RawNote {
                pitch: self.refine(bank, pitch, h1, h2),
                level,
            };
            count += 1;
        }
        count
    }

    /// During an attack the bands haven't separated a low note's partials,
    /// so its harmonics (and its unsettled neighbourhood) look like notes of
    /// their own. Anything lower with comparable energy (a fundamental this
    /// candidate could be a harmonic of, or unexplained energy lower in the
    /// bank)
    /// must first be judged by periodicity; the candidate loses if that
    /// lower note is clearly more periodic.
    fn outranked_from_below(&self, bank: &Bank, history: &History, f0: f32, clarity: f32) -> bool {
        let level = bank.amplitude_at(f0);
        let owners = (2..=OWNER_HARMONICS)
            .map(|h| f0 / h as f32)
            .take_while(|&owner| owner >= F0_RANGE.0)
            .filter(|&owner| bank.amplitude_at(owner) >= OWNER_ENERGY * level);
        let top = (hz_to_midi(f0 * LOWER).floor() as i32 - LOWEST_BAND).clamp(0, BAND_COUNT as i32)
            as usize;
        // Each periodicity search spans ±1.5 semitones, so every third band
        // covers the range below.
        let lower_bands = (0..top)
            .rev()
            .step_by(3)
            .filter(move |&b| bank.amplitude(b) >= OWNER_ENERGY * level && !self.explained(b))
            .map(|b| midi_to_hz((LOWEST_BAND + b as i32) as f32));
        owners.chain(lower_bands).any(|lower| {
            let lower = history.periodicity(lower, bank.rate());
            !lower.judged || lower.clarity > clarity + OWNER_MARGIN
        })
    }

    /// True if a note already found claims a partial in or next to `band`.
    fn explained(&self, band: usize) -> bool {
        (band.saturating_sub(1)..=(band + 1).min(BAND_COUNT - 1))
            .any(|b| self.partial_of_band[b].is_some_and(|p| self.partials[usize::from(p)].used))
    }

    /// A partial is where a band's IF falls within its own semitone: every
    /// band near a partial reports the partial's frequency, and only the
    /// band it falls in claims it.
    fn pick_partials(&mut self, bank: &Bank, gate: f32) {
        self.partial_count = 0;
        self.partial_of_band = [None; BAND_COUNT];
        let floor = gate * PARTIAL_FLOOR;
        for band in 0..BAND_COUNT {
            let amp = bank.amplitude(band);
            if amp < floor {
                continue;
            }
            let freq = bank.frequency(band);
            let centre = (LOWEST_BAND + band as i32) as f32;
            if freq <= 0.0 || (hz_to_midi(freq) - centre).abs() > 0.5 {
                continue;
            }
            let partial = Partial {
                band,
                freq,
                amp,
                used: false,
            };
            // Two adjacent bands can both hold a partial between them (or
            // both ring at their centres during an attack): keep the louder.
            if let Some(last) = self.partial_count.checked_sub(1)
                && self.partials[last].band + 1 == band
            {
                if amp > self.partials[last].amp {
                    self.partial_of_band[band - 1] = None;
                    self.partials[last] = partial;
                    self.partial_of_band[band] = Some(last as u8);
                }
                continue;
            }
            self.partials[self.partial_count] = partial;
            self.partial_of_band[band] = Some(self.partial_count as u8);
            self.partial_count += 1;
        }
    }

    /// Every partial could be a fundamental, and so could half of every
    /// partial (a weak or missing fundamental).
    fn collect_candidates(&mut self) {
        self.candidate_count = 0;
        for p in &self.partials[..self.partial_count] {
            for f0 in [p.freq, p.freq / 2.0] {
                if (F0_RANGE.0..=F0_RANGE.1).contains(&f0) {
                    self.candidates[self.candidate_count] = f0;
                    self.candidate_count += 1;
                }
            }
        }
    }

    fn best_candidate(&self) -> Option<(usize, Support)> {
        let mut best: Option<(usize, Support)> = None;
        for (index, &f0) in self.candidates[..self.candidate_count].iter().enumerate() {
            if f0 <= 0.0 {
                continue;
            }
            let support = self.support(f0);
            if support.salience > 0.0
                && best
                    .as_ref()
                    .is_none_or(|(_, b)| support.salience > b.salience)
            {
                best = Some((index, support));
            }
        }
        best
    }

    fn support(&self, f0: f32) -> Support {
        let mut support = Support {
            salience: 0.0,
            harmonic: [None; HARMONICS],
        };
        for h in 1..=HARMONICS {
            if let Some(p) = self.unused_partial_near(f0 * h as f32) {
                support.harmonic[h - 1] = Some(p);
                support.salience += self.partials[p].amp / (h as f32).sqrt();
            }
        }
        support
    }

    fn unused_partial_near(&self, freq: f32) -> Option<usize> {
        let midi = hz_to_midi(freq);
        let nearest = midi.round() as i32 - LOWEST_BAND;
        (nearest - 1..=nearest + 1)
            .filter(|&b| (0..BAND_COUNT as i32).contains(&b))
            .filter_map(|b| self.partial_of_band[b as usize])
            .map(usize::from)
            .find(|&p| {
                let partial = &self.partials[p];
                !partial.used && (hz_to_midi(partial.freq) - midi).abs() <= HARMONIC_TOLERANCE
            })
    }

    /// Exact pitch. During an attack the periodicity estimate is right and
    /// the bands are not; once settled, the IFs of the note's fundamental and
    /// second harmonic (halved), averaged over one period and weighted by
    /// loudness, are more exact.
    fn refine(
        &self,
        bank: &Bank,
        periodic_pitch: f32,
        h1: Option<usize>,
        h2: Option<usize>,
    ) -> f32 {
        let period = (bank.rate() / midi_to_hz(periodic_pitch)).round() as usize;
        let mut weighted = 0.0;
        let mut weight = 0.0;
        for (h, partial) in [(1.0, h1), (2.0, h2)] {
            if let Some(p) = partial {
                let partial = &self.partials[p];
                weighted += partial.amp * bank.frequency_over(partial.band, period) / h;
                weight += partial.amp;
            }
        }
        if weight == 0.0 {
            return periodic_pitch;
        }
        let banded_pitch = hz_to_midi(weighted / weight);
        if (banded_pitch - periodic_pitch).abs() <= SETTLED {
            banded_pitch
        } else {
            periodic_pitch
        }
    }
}
