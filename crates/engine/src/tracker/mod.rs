//! The tracker: detects the notes being played in the dry signal (ADR 0002).

use std::ops::Deref;

/// The most notes the tracker reports at once: more than the four voices, so
/// voice allocation can see a fifth note arrive and steal for it.
pub const MAX_DETECTED_NOTES: usize = 6;

/// One note the tracker hears.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DetectedNote {
    /// Stable while the note sounds, so a note that moves (a slide, a bend)
    /// stays recognisable as the same note.
    pub id: u32,
    /// Exact pitch as a fractional MIDI note number (69.0 = A4 = 440 Hz).
    pub pitch: f32,
    /// Current loudness of the note's partials (linear, ≈ peak amplitude).
    pub level: f32,
    /// Peak dry signal level around the note's onset (linear): how hard it
    /// was picked.
    pub onset_strength: f32,
}

impl DetectedNote {
    pub fn frequency(&self) -> f32 {
        440.0 * 2f32.powf((self.pitch - 69.0) / 12.0)
    }
}

/// The notes detected right now, loudest first.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DetectedNotes {
    notes: [DetectedNote; MAX_DETECTED_NOTES],
    len: usize,
}

impl Deref for DetectedNotes {
    type Target = [DetectedNote];

    fn deref(&self) -> &[DetectedNote] {
        &self.notes[..self.len]
    }
}

mod bank;
mod decimator;
mod history;
mod sieve;

use bank::Bank;
use decimator::Decimator;
use history::History;
use sieve::{RawNote, Sieve};

use crate::readout::{PeakMeter, TunerReading};

/// Default global noise gate threshold (linear, ≈ −54 dBFS).
pub const NOISE_GATE_DEFAULT: f32 = 0.002;
/// Noise gate thresholds the engine accepts (linear, −80 to −20 dBFS).
pub const NOISE_GATE_RANGE: std::ops::RangeInclusive<f32> = 0.0001..=0.1;

/// Fundamentals the tracker considers, in Hz: just below low E to just
/// above the top of the G string.
const F0_RANGE: (f32, f32) = (38.0, 420.0);

/// Analysis samples between note decisions (8 at 6 kHz ≈ 1.3 ms).
const HOP: usize = 8;
/// A new note must be heard this long before it is reported.
const BIRTH_SECONDS: f32 = 0.004;
/// A note is reported this long after it was last heard.
const RELEASE_SECONDS: f32 = 0.015;
/// A new note must clear the gate by this factor (+3 dB); a sounding note
/// stays until it falls below the gate itself.
const BIRTH_HEADROOM: f32 = std::f32::consts::SQRT_2;
/// How far (semitones) a note can move between hops and stay the same note.
const MATCH_RADIUS: f32 = 1.5;
/// A note not yet reported must stay on its semitone; wandering during the
/// attack is the analysis settling, not a note.
const PENDING_MATCH_RADIUS: f32 = 0.5;
/// The tuner waits this long after a note appears so it reads a settled
/// pitch rather than flickering through the attack.
const TUNER_SETTLE_SECONDS: f32 = 0.06;

#[derive(Clone, Copy, Default)]
struct Tracked {
    note: DetectedNote,
    hops_alive: u32,
    hops_missed: u32,
    reported: bool,
}

pub struct Tracker {
    decimator: Decimator,
    bank: Bank,
    history: History,
    sieve: Sieve,
    gate: f32,
    analysis_phase: usize,
    pick_meter: PeakMeter,
    tracked: [Option<Tracked>; MAX_DETECTED_NOTES],
    next_id: u32,
    birth_hops: u32,
    release_hops: u32,
    tuner_settle_hops: u32,
}

impl Tracker {
    pub fn new(sample_rate: f32) -> Self {
        let decimator = Decimator::new(sample_rate);
        let rate = sample_rate / decimator.factor() as f32;
        let hops = |seconds: f32| (seconds * rate / HOP as f32).ceil() as u32;
        Tracker {
            decimator,
            bank: Bank::new(rate),
            history: History::new(),
            sieve: Sieve::new(),
            gate: NOISE_GATE_DEFAULT,
            analysis_phase: 0,
            pick_meter: PeakMeter::new(sample_rate),
            tracked: [None; MAX_DETECTED_NOTES],
            next_id: 0,
            birth_hops: hops(BIRTH_SECONDS),
            release_hops: hops(RELEASE_SECONDS),
            tuner_settle_hops: hops(TUNER_SETTLE_SECONDS),
        }
    }

    pub fn set_gate(&mut self, threshold: f32) {
        self.gate = threshold.clamp(*NOISE_GATE_RANGE.start(), *NOISE_GATE_RANGE.end());
    }

    pub fn process(&mut self, dry: &[f32]) {
        for &sample in dry {
            self.pick_meter.process(sample);
            let Some(x) = self.decimator.process(sample) else {
                continue;
            };
            self.bank.process(x);
            self.history.push(x);
            self.analysis_phase += 1;
            if self.analysis_phase == HOP {
                self.analysis_phase = 0;
                self.hop();
            }
        }
    }

    fn hop(&mut self) {
        let mut heard = [RawNote::default(); MAX_DETECTED_NOTES];
        let count = self
            .sieve
            .analyse(&self.bank, &self.history, self.gate, &mut heard);
        let mut matched = [false; MAX_DETECTED_NOTES];

        // Each sounding note takes the nearest note heard this hop.
        for slot in self.tracked.iter_mut().flatten() {
            let radius = if slot.reported {
                MATCH_RADIUS
            } else {
                PENDING_MATCH_RADIUS
            };
            let nearest = (0..count)
                .filter(|&i| !matched[i])
                .map(|i| (i, (heard[i].pitch - slot.note.pitch).abs()))
                .filter(|&(_, distance)| distance <= radius)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match nearest {
                Some((i, _)) => {
                    matched[i] = true;
                    slot.note.pitch = heard[i].pitch;
                    slot.note.level = heard[i].level;
                    slot.hops_alive += 1;
                    slot.hops_missed = 0;
                }
                None => slot.hops_missed += 1,
            }
        }

        for slot in &mut self.tracked {
            if let Some(t) = slot {
                let expired = if t.reported {
                    t.hops_missed > self.release_hops
                } else {
                    t.hops_missed > 0
                };
                if expired {
                    *slot = None;
                } else if !t.reported
                    && t.hops_alive >= self.birth_hops
                    && t.note.level >= self.gate * BIRTH_HEADROOM
                {
                    t.reported = true;
                    t.note.onset_strength = self.pick_meter.level();
                }
            }
        }

        for i in (0..count).filter(|&i| !matched[i]) {
            let Some(free) = self.tracked.iter_mut().find(|s| s.is_none()) else {
                break;
            };
            self.next_id = self.next_id.wrapping_add(1);
            *free = Some(Tracked {
                note: DetectedNote {
                    id: self.next_id,
                    pitch: heard[i].pitch,
                    level: heard[i].level,
                    onset_strength: 0.0,
                },
                hops_alive: 1,
                hops_missed: 0,
                reported: false,
            });
        }
    }

    /// The reported notes, loudest first.
    pub fn notes(&self) -> DetectedNotes {
        let mut notes = DetectedNotes::default();
        for t in self.tracked.iter().flatten().filter(|t| t.reported) {
            notes.notes[notes.len] = t.note;
            notes.len += 1;
        }
        notes.notes[..notes.len].sort_unstable_by(|a, b| b.level.total_cmp(&a.level));
        notes
    }

    /// A reading only when exactly one note is sounding and has settled.
    pub fn tuner(&self) -> Option<TunerReading> {
        let mut reported = self.tracked.iter().flatten().filter(|t| t.reported);
        let only = reported.next()?;
        if reported.next().is_some() || only.hops_alive < self.birth_hops + self.tuner_settle_hops {
            return None;
        }
        let nearest = only.note.pitch.round();
        Some(TunerReading {
            note: nearest.clamp(0.0, 127.0) as u8,
            cents: (only.note.pitch - nearest) * 100.0,
        })
    }
}
