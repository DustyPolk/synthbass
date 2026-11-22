//! Per-hop analysis: picks partials out of the bank and groups them into
//! notes with a harmonic sieve (Klapuri-style iterative cancellation).

use super::bank::{BAND_COUNT, Bank, LOWEST_BAND};
use super::history::{History, Periodicity};
use super::{F0_RANGE, MAX_DETECTED_NOTES};
use crate::pitch::{hz_to_midi, midi_to_hz};

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
/// An unjudged sounding note's periodicity must still be this clear.
const UNJUDGED_CLARITY: f32 = 0.3;
/// Lower fundamentals a candidate could be a harmonic of: f0/2 … f0/8.
const OWNER_HARMONICS: usize = 8;
/// A possible owner with at least this fraction of the candidate's
/// fundamental level in the bank is worth checking.
const OWNER_ENERGY: f32 = 0.3;
/// A lower fundamental lies under this fraction of the candidate's: more than
/// a semitone below.
const SEMITONE_BELOW_RATIO: f32 = 0.943;
/// A candidate is a harmonic if its owner is this much more periodic.
const OWNER_MARGIN: f32 = 0.15;
/// Once the bands' IF agrees with the periodicity estimate this closely
/// (semitones) the bands have settled, and their IF is the more exact.
const PERIODICITY_AGREEMENT: f32 = 0.5;
/// A band settles once its fast IF comes this close (semitones) to its slow
/// IF, and stays settled until they are `UNSETTLE_DISTANCE` apart: beating
/// with a close neighbour swings the fast IF further than a slide or a new
/// note would let it settle.
const SETTLE_DISTANCE: f32 = 0.35;
const UNSETTLE_DISTANCE: f32 = 1.0;
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
const SAME_PARTIAL_DISTANCE: f32 = 0.5;
/// A settled note whose periodicity estimate is at least this clear is a
/// note on its own, where that estimate is exact to a few cents.
const FOLLOW_CLARITY: f32 = 0.9;
/// Crossing a fret blurs the periodicity for a moment: a note already
/// moving keeps following down to this clarity.
const KEEP_FOLLOWING_CLARITY: f32 = 0.75;
/// The slow IF lags a slide or a bend by about 60 ms (more with the bank's
/// own delay on low notes). Once a clear note's periodicity estimate leads
/// its slow IF by more than `FOLLOW_FROM` semitones the note is moving, and
/// its pitch blends toward the periodicity estimate, wholly at `FOLLOW_FULL`.
const FOLLOW_FROM: f32 = 0.2;
const FOLLOW_FULL: f32 = 0.4;
/// Hops (about 27 ms) a moving note's bands stay unsettled after it was
/// last heard moving.
const MOVING_HOLD_HOPS: u32 = 20;
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
/// Right after a pick, while a new note's partials are still growing, the
/// lagging bands of the note that rang before can hold one of its partials
/// alone: a sparse candidate then must be louder still to be a new note.
const SPARSE_FLOOR_AT_PICK: f32 = 0.75;
/// A sparse candidate this close (semitones) to a sounding note continues
/// it.
const CONTINUATION_DISTANCE: f32 = 0.5;
/// Intervals (semitones) above a louder note at which a sparse candidate
/// fuses with it rather than continuing a note: fifth, octave, twelfth.
const FUSING_INTERVALS: [f32; 3] = [7.0, 12.0, 19.0];
/// A sparse candidate up to this harmonic of a sounding note is what is
/// left of that note.
const REMNANT_HARMONICS: usize = 4;
/// A still note whose refined pitch lies this far (semitones) from the
/// partials that made it is not the note they belong to.
const STRAY_DISTANCE: f32 = 1.0;
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
    /// How periodic the signal is at the note's pitch: 0 when the
    /// periodicity estimate settled on another pitch, 1 for what is left of
    /// a sounding note.
    pub clarity: f32,
}

/// What the tracker knows going into a hop.
pub struct Context<'a> {
    /// The pitches of the notes reported so far.
    pub sounding: &'a [f32],
    /// At most one note is being heard.
    pub alone: bool,
    /// A pick happened within the pick window.
    pub picked: bool,
}

/// A note's exact pitch, and whether it is moving faster than the slow IF
/// can follow.
struct Refined {
    pitch: f32,
    moving: bool,
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
    /// The pitches (low, high) a note on its own moved across. For
    /// `moving_hops` more hops a candidate on their harmonics isn't steady
    /// whatever its bands say: their slow IF lags the slide.
    moving_span: (f32, f32),
    moving_hops: u32,
    /// The moving note's pitch when last heard moving.
    moving_pitch: f32,
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
            moving_span: (0.0, 0.0),
            moving_hops: 0,
            moving_pitch: 0.0,
        }
    }

    /// Finds the notes in the bank's current state, strongest first.
    pub fn analyse(
        &mut self,
        bank: &Bank,
        history: &History,
        gate: f32,
        context: &Context,
        out: &mut [RawNote; MAX_DETECTED_NOTES],
    ) -> usize {
        let Context {
            sounding,
            alone,
            picked,
        } = *context;
        self.pick_partials(bank, gate);
        self.collect_candidates();

        let mut count = 0;
        let mut strongest = 0.0;
        let mut moving = false;
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
                    let level = self.claim(&support);
                    let duplicate = out[..count].iter().any(|n| (n.pitch - pitch).abs() < 0.5);
                    if !duplicate && level >= gate {
                        out[count] = RawNote {
                            pitch,
                            level,
                            clarity: 1.0,
                        };
                        count += 1;
                    }
                    continue;
                }
                let continues = !fuses
                    && sounding
                        .iter()
                        .any(|&pitch| (pitch - midi).abs() <= CONTINUATION_DISTANCE);
                let floor = if picked {
                    SPARSE_FLOOR_AT_PICK
                } else {
                    SPARSE_FLOOR
                };
                if !continues && support.salience < floor * strongest {
                    continue;
                }
            }
            let periodicity = history.periodicity(f0, bank.rate());
            let settled_partials = support
                .harmonic
                .iter()
                .flatten()
                .filter(|&&p| self.settled[self.partials[p].band])
                .count();
            let steady = settled_partials >= STEADY_PARTIALS
                // A moving note's partials lag behind it in bands that still
                // look settled.
                && !(self.moving_hops > 0 && self.on_moving_span(hz_to_midi(f0)));
            // Beside a louder note a sounding note's periodicity is swamped
            // and one of its bands can come unsettled for a moment as they
            // beat; dropping it then lets its upper harmonics start a note of
            // their own (a third's D#2 leaving A#3). One settled partial is
            // enough to carry it on.
            let continues_sounding = sounding
                .iter()
                .any(|&pitch| (pitch - hz_to_midi(f0)).abs() <= CONTINUATION_DISTANCE);
            let carried_on = count > 0 && settled_partials > 0 && continues_sounding;
            // A louder attack beside a sounding note leaves too little of the
            // note's energy in the older half of any periodicity span for a
            // judgement; its settled partials still show it carries on.
            let carries_on_unjudged =
                steady && continues_sounding && periodicity.clarity >= UNJUDGED_CLARITY;
            if (!periodicity.judged && !carries_on_unjudged)
                || (periodicity.clarity < MIN_CLARITY && !steady && !carried_on)
                || self.outranked_from_below(
                    bank,
                    history,
                    f0,
                    periodicity.clarity,
                    !carries_on_unjudged,
                )
            {
                continue;
            }
            // Only a note on its own can be followed by its periodicity,
            // and not right after a pick: a new note's attack would pull the
            // ringing one toward it.
            let refined = self.refine(bank, f0, history, &periodicity, alone, &support.harmonic);
            let pitch = refined.pitch;
            // Two candidates can settle on the same pitch.
            let duplicate = out[..count].iter().any(|n| (n.pitch - pitch).abs() < 0.5);
            strongest = f32::max(strongest, support.salience);
            let mut level = self.claim(&support);
            // A sliding note leaves its partials ringing behind it in the
            // bands it passed; it owns the partials where it is now as well,
            // or they would start notes of their own.
            if refined.moving {
                let (span, passed) = self.claim_passed(hz_to_midi(f0), pitch);
                level = level.hypot(passed);
                self.moving_span = span;
                self.moving_pitch = pitch;
                moving = true;
            }
            // A candidate whose pitch came from a periodicity estimate more
            // than a semitone off its own partials took the period of a note
            // beside it: between a third's fundamentals a band hears both and
            // holds a partial halfway, nobody's note. Right after a pick the
            // disturbed analysis of a sounding note jumps like this too, and
            // that jump is how its repick is found.
            let strayed =
                !picked && !refined.moving && (pitch - hz_to_midi(f0)).abs() > STRAY_DISTANCE;
            if duplicate || level < gate || strayed {
                continue;
            }
            // The periodicity search spans ±1.5 semitones and can lock onto
            // a note beside this one (a new note's attack beside the lagging
            // bands of the note before it).
            let own_clarity =
                if (hz_to_midi(periodicity.freq) - pitch).abs() <= PERIODICITY_AGREEMENT {
                    periodicity.clarity
                } else {
                    0.0
                };
            out[count] = RawNote {
                pitch,
                level,
                clarity: own_clarity,
            };
            count += 1;
        }
        if moving {
            self.moving_hops = MOVING_HOLD_HOPS;
        } else {
            self.moving_hops = self.moving_hops.saturating_sub(1);
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

    /// Marks used every unused partial lying, as one of the first
    /// `HARMONICS` harmonics, between pitches `from` and `to` (a moving
    /// note's lagging and current pitch) or trailing behind by up to as far
    /// again: narrow bands lag more than the fundamental's. Returns that
    /// span of fundamentals and the partials' level.
    fn claim_passed(&mut self, from: f32, to: f32) -> ((f32, f32), f32) {
        let behind = from - (to - from);
        let (low, high) = (
            behind.min(to) - HARMONIC_TOLERANCE,
            behind.max(to) + HARMONIC_TOLERANCE,
        );
        let mut energy = 0.0;
        for partial in &mut self.partials[..self.partial_count] {
            let midi = hz_to_midi(partial.freq);
            let passed = (1..=HARMONICS).any(|h| {
                let fundamental = midi - 12.0 * (h as f32).log2();
                (low..=high).contains(&fundamental)
            });
            if !partial.used && passed {
                partial.used = true;
                energy += partial.amp * partial.amp;
            }
        }
        ((low, high), energy.sqrt())
    }

    /// True if `midi` is within half a semitone of one of the first
    /// `HARMONICS` harmonics of a pitch in the moving span.
    fn on_moving_span(&self, midi: f32) -> bool {
        let (low, high) = self.moving_span;
        (1..=HARMONICS).any(|h| {
            let fundamental = midi - 12.0 * (h as f32).log2();
            fundamental >= low - 0.5 && fundamental <= high + 0.5
        })
    }

    /// During an attack the bands haven't separated a low note's partials,
    /// so its harmonics (and its unsettled neighbourhood) look like notes of
    /// their own. Anything lower with comparable energy (a fundamental this
    /// candidate could be a harmonic of, or unexplained energy lower in the
    /// bank) must first be judged by periodicity; the candidate loses if
    /// that lower note is clearly more periodic, could own it, and is heard.
    /// A lower probe that cannot be judged outranks the candidate only when
    /// `unjudged_outranks`.
    fn outranked_from_below(
        &self,
        bank: &Bank,
        history: &History,
        f0: f32,
        clarity: f32,
        unjudged_outranks: bool,
    ) -> bool {
        let level = bank.amplitude_at(f0);
        let owners = (2..=OWNER_HARMONICS)
            .map(|h| f0 / h as f32)
            .take_while(|&owner| owner >= F0_RANGE.0)
            .filter(|&owner| bank.amplitude_at(owner) >= OWNER_ENERGY * level);
        let top = (hz_to_midi(f0 * SEMITONE_BELOW_RATIO).floor() as i32 - LOWEST_BAND)
            .clamp(0, BAND_COUNT as i32) as usize;
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
            (!lower.judged && unjudged_outranks)
                || (lower.judged
                    && lower.clarity > clarity + OWNER_MARGIN
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
            let max_distance = if self.settled[band] && !bank.restarted(band) {
                UNSETTLE_DISTANCE
            } else {
                SETTLE_DISTANCE
            };
            self.settled[band] = self.slow[band] > 0.0
                && (hz_to_midi(fast[band]) - slow).abs() <= max_distance
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
                && (hz_to_midi(freq) - hz_to_midi(self.partials[last].freq) < SAME_PARTIAL_DISTANCE
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
    /// A note on its own that is moving (a slide, a bend, vibrato) follows
    /// its periodicity estimate, which lags least.
    fn refine(
        &self,
        bank: &Bank,
        f0: f32,
        history: &History,
        periodicity: &Periodicity,
        alone: bool,
        harmonic: &[Option<usize>; HARMONICS],
    ) -> Refined {
        let mut periodic_pitch = hz_to_midi(periodicity.freq);
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
        let slow_pitch = (slow_weight > 0.0).then(|| hz_to_midi(slow_weighted / slow_weight));
        let banded = if settled {
            slow_pitch.unwrap_or(periodic_pitch)
        } else {
            self.banded_pitch(bank, f0, periodic_pitch, harmonic)
        };
        let still = Refined {
            pitch: banded,
            moving: false,
        };
        let Some(slow_pitch) = slow_pitch else {
            return still;
        };
        if !alone || !(settled || self.moving_hops > 0) {
            return still;
        }
        let mut clarity = periodicity.clarity;
        // A fretted slide steps a semitone at a time, too fast for a search
        // around the lagging candidate: look where the note was last hop.
        if self.moving_hops > 0 {
            let near = history.periodicity(midi_to_hz(self.moving_pitch), bank.rate());
            if near.clarity > clarity {
                periodic_pitch = hz_to_midi(near.freq);
                clarity = near.clarity;
            }
        }
        let min_clarity = if self.moving_hops > 0 {
            KEEP_FOLLOWING_CLARITY
        } else {
            FOLLOW_CLARITY
        };
        if clarity < min_clarity {
            return still;
        }
        let lead = periodic_pitch - slow_pitch;
        let follow = ((lead.abs() - FOLLOW_FROM) / (FOLLOW_FULL - FOLLOW_FROM)).clamp(0.0, 1.0);
        Refined {
            pitch: banded + (periodic_pitch - banded) * follow,
            moving: follow > 0.0,
        }
    }

    /// An unsettled note's pitch: its fundamental's and second harmonic's
    /// IF over one period when that agrees with the periodicity estimate.
    fn banded_pitch(
        &self,
        bank: &Bank,
        f0: f32,
        periodic_pitch: f32,
        harmonic: &[Option<usize>; HARMONICS],
    ) -> f32 {
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
        if (banded_pitch - periodic_pitch).abs() <= PERIODICITY_AGREEMENT {
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
