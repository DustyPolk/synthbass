//! Per-hop analysis: picks partials out of the bank and groups them into
//! notes with a harmonic sieve (Klapuri-style iterative cancellation).

use super::bank::{BAND_COUNT, Bank, LOWEST_BAND, hz_to_midi, midi_to_hz};
use super::history::History;
use super::{F0_RANGE, MAX_DETECTED_NOTES};

/// Partials quieter than this fraction of the gate are ignored outright.
const PARTIAL_FLOOR: f32 = 0.5;
/// How close (in semitones) a partial must be to a harmonic to count.
const HARMONIC_TOLERANCE: f32 = 0.35;
/// Harmonics a note collects; harmonic h adds its amplitude / √h to the
/// note's salience.
const HARMONICS: usize = 6;
/// A candidate whose signal is less periodic than this at its period is
/// noise. Kept low: in a chord every note's periodicity is disturbed by the
/// others.
const MIN_CLARITY: f32 = 0.1;
/// A candidate with this many settled partials is steady sinusoids, not
/// noise, and needs no periodicity: in a chord a quieter note's periodicity
/// is swamped by the louder one.
const STEADY_PARTIALS: usize = 2;
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
/// A band settles once its fast IF comes this close (semitones) to its slow
/// IF, and stays settled until they are `MOVING` apart: beating with a close
/// neighbour swings the fast IF further than a slide or a new note
/// would let it settle.
const SETTLING: f32 = 0.35;
const MOVING: f32 = 1.0;
/// A neighbouring band whose slow IF agrees with a band's this closely
/// (semitones) hears the same partial: the attack's ringing at each band's
/// own centre has died away.
const NEIGHBOUR_AGREEMENT: f32 = 0.25;
/// A neighbouring band this much quieter than a band hears only its
/// leakage (−14 dB a semitone away) and other notes' partials: above the
/// bandwidth floor bands are a semitone wide, so once the attack's ringing
/// has died down that far, the band has settled whatever its neighbour's IF.
const QUIET_NEIGHBOUR: f32 = 0.3;
/// Adjacent bands whose IFs are closer than this (semitones) hear the same
/// partial.
const SAME_PARTIAL: f32 = 0.5;
/// Harmonics whose slow IF gives a settled note's pitch.
const SLOW_HARMONICS: usize = 3;
/// How much a partial with another partial a semitone away (and so pulled
/// toward it) counts toward a note's pitch.
const CROWDED_WEIGHT: f32 = 0.1;
/// A candidate whose octave below has a fundamental at least this fraction
/// of the candidate's own is that lower note's second harmonic (a low string
/// with a weak fundamental).
const OCTAVE_BELOW_FUNDAMENTAL: f32 = 0.15;
/// So is one whose octave below has its third harmonic and nearly the
/// candidate's salience (a weak or missing fundamental).
const OCTAVE_BELOW_SALIENCE: f32 = 0.8;
/// A sparse candidate (fewer than two of its lowest three harmonics) beside
/// a stronger note (a string ringing on, a third's fundamental pulled off
/// its semitone by beating) must be this loud relative to it to be a new
/// note.
const SPARSE_FLOOR: f32 = 0.5;
/// A sparse candidate this close (semitones) to a sounding note continues
/// it.
const CONTINUES: f32 = 0.5;
/// Intervals (semitones) above a louder note at which a sparse candidate
/// fuses with it rather than continuing a note: fifth, octave, twelfth.
const FUSING_INTERVALS: [f32; 3] = [7.0, 12.0, 19.0];
/// A sparse candidate up to this harmonic of a sounding note is what is
/// left of that note.
const REMNANT_HARMONICS: usize = 4;
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
    /// Each band's slow IF this hop, in Hz.
    slow: [f32; BAND_COUNT],
    /// Whether each band has settled on a partial (kept across hops for
    /// hysteresis).
    settled: [bool; BAND_COUNT],
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
            slow: [0.0; BAND_COUNT],
            settled: [false; BAND_COUNT],
        }
    }

    /// Finds the notes in the bank's current state, strongest first.
    /// `sounding` holds the pitches of the notes reported so far.
    pub fn analyse(
        &mut self,
        bank: &Bank,
        history: &History,
        gate: f32,
        sounding: &[f32],
        out: &mut [RawNote; MAX_DETECTED_NOTES],
    ) -> usize {
        self.pick_partials(bank, gate);
        self.collect_candidates();

        let mut count = 0;
        let mut strongest = 0.0;
        while count < MAX_DETECTED_NOTES {
            let Some((index, mut support)) = self.best_candidate() else {
                break;
            };
            let mut f0 = self.candidates[index];
            self.candidates[index] = 0.0;
            if let Some(lower) = self.octave_below(f0, &support) {
                f0 /= 2.0;
                support = lower;
            }
            let [h1, h2, h3, ..] = support.harmonic;
            // Ghosts an octave or a twelfth below a real note get support from
            // its harmonics but have nothing at their own odd harmonics.
            if h1.is_none() && (h2.is_none() || h3.is_none()) {
                continue;
            }
            if support.salience < RELATIVE_FLOOR * strongest {
                break;
            }
            let sparse = support.harmonic[..3].iter().flatten().count() < 2;
            if sparse {
                let midi = hz_to_midi(f0);
                let fuses = out[..count].iter().any(|n| {
                    FUSING_INTERVALS
                        .iter()
                        .any(|&i| (midi - n.pitch - i).abs() <= HARMONIC_TOLERANCE)
                });
                // What is left of a sounding note can be one of its upper
                // harmonics alone (a string whose lowest partials died first).
                if !fuses && let Some(pitch) = remnant(midi, sounding, &out[..count]) {
                    out[count] = RawNote {
                        pitch,
                        level: self.claim(&support),
                    };
                    count += 1;
                    continue;
                }
                let continues = !fuses
                    && sounding
                        .iter()
                        .any(|&pitch| (pitch - midi).abs() <= CONTINUES);
                if !continues && support.salience < SPARSE_FLOOR * strongest {
                    continue;
                }
            }
            let periodicity = history.periodicity(f0, bank.rate());
            let steady = support
                .harmonic
                .iter()
                .flatten()
                .filter(|&&p| self.settled[self.partials[p].band])
                .count()
                >= STEADY_PARTIALS;
            if !periodicity.judged
                || (periodicity.clarity < MIN_CLARITY && !steady)
                || self.outranked_from_below(bank, history, f0, periodicity.clarity)
            {
                continue;
            }
            let pitch = self.refine(bank, f0, hz_to_midi(periodicity.freq), &support.harmonic);
            // Two candidates can settle on the same pitch.
            let duplicate = out[..count].iter().any(|n| (n.pitch - pitch).abs() < 0.5);
            strongest = f32::max(strongest, support.salience);
            let level = self.claim(&support);
            if duplicate || level < gate {
                continue;
            }
            out[count] = RawNote { pitch, level };
            count += 1;
        }
        count
    }

    /// Marks a note's partials used; returns the note's level.
    fn claim(&mut self, support: &Support) -> f32 {
        let mut energy = 0.0;
        for &p in support.harmonic.iter().flatten() {
            self.partials[p].used = true;
            energy += self.partials[p].amp * self.partials[p].amp;
        }
        energy.sqrt()
    }

    /// During an attack the bands haven't separated a low note's partials,
    /// so its harmonics (and its unsettled neighbourhood) look like notes of
    /// their own. Anything lower with comparable energy (a fundamental this
    /// candidate could be a harmonic of, or unexplained energy lower in the
    /// bank) must first be judged by periodicity; the candidate loses if
    /// that lower note is clearly more periodic, could own it, and is heard.
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
        // The search around a low band can lock onto the period of an
        // unrelated note (a multiple of a louder upper note's period): only
        // a lower note this candidate could belong to (a harmonic of it, or
        // of its octave below) outranks it.
        let related = |lower: f32| {
            let ratio = 2.0 * f0 / lower;
            (12.0 * (ratio / ratio.round()).log2()).abs() <= HARMONIC_TOLERANCE
        };
        owners.chain(lower_bands).any(|probe| {
            let lower = history.periodicity(probe, bank.rate());
            !lower.judged
                || (lower.clarity > clarity + OWNER_MARGIN
                    && related(lower.freq)
                    && self.heard(lower.freq))
        })
    }

    /// True if a partial lies at `freq`. Two notes a semitone apart are
    /// together periodic an octave below the upper one, where nothing
    /// sounds.
    fn heard(&self, freq: f32) -> bool {
        self.partials_near(freq).next().is_some()
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
        let mut fast = [0.0; BAND_COUNT];
        let mut amp = [0.0; BAND_COUNT];
        for band in 0..BAND_COUNT {
            fast[band] = bank.frequency(band);
            self.slow[band] = bank.slow_frequency(band);
            amp[band] = bank.amplitude(band);
        }
        for band in 0..BAND_COUNT {
            let slow = hz_to_midi(self.slow[band]);
            let moving = if self.settled[band] && !bank.restarted(band) {
                MOVING
            } else {
                SETTLING
            };
            self.settled[band] = self.slow[band] > 0.0
                && (hz_to_midi(fast[band]) - slow).abs() <= moving
                && neighbours(band).any(|b| {
                    (hz_to_midi(self.slow[b]) - slow).abs() <= NEIGHBOUR_AGREEMENT
                        || amp[b] <= QUIET_NEIGHBOUR * amp[band]
                });
        }
        let floor = gate * PARTIAL_FLOOR;
        for band in 0..BAND_COUNT {
            if amp[band] < floor {
                continue;
            }
            // A settled band's slow IF doesn't beat with a neighbouring
            // partial too close for the band to separate (a low third's).
            let freq = if self.settled[band] {
                self.slow[band]
            } else {
                fast[band]
            };
            let centre = (LOWEST_BAND + band as i32) as f32;
            if freq <= 0.0 || (hz_to_midi(freq) - centre).abs() > 0.5 {
                continue;
            }
            let partial = Partial {
                band,
                freq,
                amp: amp[band],
                used: false,
            };
            // Two adjacent bands can both hold a partial between them (or
            // both ring at their centres during an attack): keep the louder.
            // Settled partials a semitone apart (a seventh's or a third's
            // harmonics) are both kept.
            if let Some(last) = self.partial_count.checked_sub(1)
                && self.partials[last].band + 1 == band
                && (hz_to_midi(freq) - hz_to_midi(self.partials[last].freq) < SAME_PARTIAL
                    || !self.settled[band]
                    || !self.settled[band - 1])
            {
                if partial.amp > self.partials[last].amp {
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
            if let Some(p) = self
                .partials_near(f0 * h as f32)
                .find(|&p| !self.partials[p].used)
            {
                support.harmonic[h - 1] = Some(p);
                support.salience += self.partials[p].amp / (h as f32).sqrt();
            }
        }
        support
    }

    /// The partials within the harmonic tolerance of `freq`.
    fn partials_near(&self, freq: f32) -> impl Iterator<Item = usize> + '_ {
        let midi = hz_to_midi(freq);
        let nearest = midi.round() as i32 - LOWEST_BAND;
        (nearest - 1..=nearest + 1)
            .filter(|&b| (0..BAND_COUNT as i32).contains(&b))
            .filter_map(|b| self.partial_of_band[b as usize])
            .map(usize::from)
            .filter(move |&p| {
                (hz_to_midi(self.partials[p].freq) - midi).abs() <= HARMONIC_TOLERANCE
            })
    }

    /// The support of `f0 / 2` if that is the note `f0` belongs to: `f0`'s
    /// fundamental is its second harmonic, and it has a fundamental of its
    /// own or its third harmonic. The 1/√h weighting otherwise lets the
    /// octave of a low string with a weak fundamental outscore it.
    fn octave_below(&self, f0: f32, support: &Support) -> Option<Support> {
        let own = support.harmonic[0]?;
        let lower = f0 / 2.0;
        if lower < F0_RANGE.0 {
            return None;
        }
        let below = self.support(lower);
        if below.harmonic[1] != Some(own) {
            return None;
        }
        let fundamental = below.harmonic[0].is_some_and(|p| {
            self.partials[p].amp >= OCTAVE_BELOW_FUNDAMENTAL * self.partials[own].amp
        });
        let missing_fundamental = below.harmonic[2].is_some()
            && below.salience >= OCTAVE_BELOW_SALIENCE * support.salience;
        (fundamental || missing_fundamental).then_some(below)
    }

    /// Exact pitch. During an attack the periodicity estimate is right and
    /// the bands are not; once settled, the IFs of the note's fundamental and
    /// second harmonic (halved), averaged over one period and weighted by
    /// loudness, are more exact. Once a partial's band has settled, the slow
    /// IF of the note's lowest harmonics is the most exact: in a chord
    /// neither the periodicity estimate nor the one-period IF is steady.
    fn refine(
        &self,
        bank: &Bank,
        f0: f32,
        periodic_pitch: f32,
        harmonic: &[Option<usize>; HARMONICS],
    ) -> f32 {
        let mut slow_weighted = 0.0;
        let mut slow_weight = 0.0;
        let mut settled = false;
        for (h, partial) in harmonic[..SLOW_HARMONICS].iter().enumerate() {
            if let Some(p) = *partial {
                let partial = &self.partials[p];
                let crowded = neighbours(partial.band).any(|b| self.partial_of_band[b].is_some());
                let weight = partial.amp * if crowded { CROWDED_WEIGHT } else { 1.0 };
                slow_weighted += weight * self.slow[partial.band] / (h + 1) as f32;
                slow_weight += weight;
                settled |= self.settled[partial.band];
            }
        }
        if settled {
            return hz_to_midi(slow_weighted / slow_weight);
        }
        let period = (bank.rate() / f0).round() as usize;
        let mut weighted = 0.0;
        let mut weight = 0.0;
        for (h, partial) in [(1.0, harmonic[0]), (2.0, harmonic[1])] {
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

/// The bands either side of `band`.
fn neighbours(band: usize) -> impl Iterator<Item = usize> {
    [band.wrapping_sub(1), band + 1]
        .into_iter()
        .filter(|&b| b < BAND_COUNT)
}

/// The pitch of the sounding note that `midi` is a harmonic (2nd to 4th)
/// of, unless a note found this hop already continues it.
fn remnant(midi: f32, sounding: &[f32], found: &[RawNote]) -> Option<f32> {
    sounding
        .iter()
        .filter(|&&s| !found.iter().any(|n| (n.pitch - s).abs() < 0.5))
        .find_map(|&s| {
            (2..=REMNANT_HARMONICS).find_map(|h| {
                let lower = midi - 12.0 * (h as f32).log2();
                ((lower - s).abs() <= HARMONIC_TOLERANCE).then_some(lower)
            })
        })
}
