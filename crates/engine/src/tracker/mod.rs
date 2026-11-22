//! The tracker: detects the notes being played in the dry signal (ADR 0002).

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
    /// Peak dry signal level around the note's latest onset (linear): how
    /// hard it was picked.
    pub onset_strength: f32,
    /// How many times the note was picked again while it still sounded
    /// (0 when it appears). Each repick also updates `onset_strength`.
    pub repicks: u32,
}

/// The notes detected right now, loudest first.
pub type DetectedNotes = FixedList<DetectedNote, MAX_DETECTED_NOTES>;

mod bank;
mod decimator;
mod history;
#[cfg(test)]
mod probe;
mod sieve;

use bank::Bank;
use decimator::Decimator;
use history::History;
use sieve::{Context, RawNote, Sieve};

use crate::fixed_list::FixedList;
use crate::readout::{PeakMeter, TunerReading};

/// Fundamentals the tracker considers, in Hz: just below low E to just
/// above the top of the G string.
const F0_RANGE: (f32, f32) = (38.0, 420.0);

/// Analysis samples between note decisions (8 at 6 kHz ≈ 1.3 ms).
const HOP: usize = 8;
/// A new note must be heard this long before it is reported.
const BIRTH_SECONDS: f32 = 0.004;
/// A note is reported this long after it was last heard.
const RELEASE_SECONDS: f32 = 0.015;
/// A note first heard within the pick window must have been at least this
/// periodic at its pitch, or be heard for `UNCLEAR_BIRTH_SECONDS`: while the
/// bands ring from a pick they briefly hold partials of notes nobody played.
const PICK_CLARITY: f32 = 0.35;
const UNCLEAR_BIRTH_SECONDS: f32 = 0.04;
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
/// A sounding note whose level rises this far above its lowest level since
/// its last onset was picked again. Decay only falls, and vibrato or a slide
/// moves the level by much less.
const REPICK_RISE: f32 = 1.3;
/// Right after the dry signal's peak jumps (a pick somewhere), a smaller
/// rise is enough, which finds the repick sooner.
const REPICK_RISE_AFTER_PICK: f32 = 1.1;
/// The dry signal's peak rising within `PICK_HOPS` hops to this factor above
/// its highest level over the `PICK_LOOKBACK_HOPS` before is a pick; a picked
/// note's level counts as rising for `PICK_WINDOW_SECONDS`. Two strings
/// sounding together beat at their fundamentals' difference (12–17 Hz for low
/// thirds): the peak sags and recovers every beat, so the lookback spans more
/// than one beat (≈ 130 ms at 1.3 ms hops) and a recovery is not a pick.
const PICK_JUMP: f32 = 1.2;
const PICK_HOPS: usize = 8;
const PICK_LOOKBACK_HOPS: usize = 96;
const PICK_WINDOW_SECONDS: f32 = 0.04;
/// A sounding note whose pitch jumps this far (semitones) in one hop right
/// after a pick was picked again.
const REPICK_JUMP: f32 = 0.5;
/// After an onset the level is still rising toward the pick's peak; no
/// repick is looked for until this long after it.
const REPICK_HOLDOFF_SECONDS: f32 = 0.08;

/// A note followed from hop to hop, pending until it is reported, with the
/// tracker's bookkeeping on it.
#[derive(Clone, Copy, Default)]
struct NoteTrack {
    note: DetectedNote,
    hops_alive: u32,
    hops_missed: u32,
    reported: bool,
    /// Lowest level since the hold-off after the latest onset ended.
    valley: f32,
    hops_since_onset: u32,
    /// Highest periodicity clarity heard before the note was reported.
    clarity: f32,
    /// Hops since the latest pick when the note was first heard.
    born_after_pick: u32,
}

pub struct Tracker {
    decimator: Decimator,
    bank: Bank,
    history: History,
    sieve: Sieve,
    gate: f32,
    analysis_phase: usize,
    pick_meter: PeakMeter,
    tracks: [Option<NoteTrack>; MAX_DETECTED_NOTES],
    next_id: u32,
    birth_hops: u32,
    unclear_birth_hops: u32,
    release_hops: u32,
    tuner_settle_hops: u32,
    repick_holdoff_hops: u32,
    /// The pick meter's level at each of the last `PICK_LOOKBACK_HOPS` hops.
    pick_history: [f32; PICK_LOOKBACK_HOPS],
    pick_history_index: usize,
    hops_since_pick: u32,
    pick_window_hops: u32,
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
            gate: crate::NoiseGate::default().threshold(),
            analysis_phase: 0,
            pick_meter: PeakMeter::new(sample_rate),
            tracks: [None; MAX_DETECTED_NOTES],
            next_id: 0,
            birth_hops: hops(BIRTH_SECONDS),
            unclear_birth_hops: hops(UNCLEAR_BIRTH_SECONDS),
            release_hops: hops(RELEASE_SECONDS),
            tuner_settle_hops: hops(TUNER_SETTLE_SECONDS),
            repick_holdoff_hops: hops(REPICK_HOLDOFF_SECONDS),
            pick_history: [0.0; PICK_LOOKBACK_HOPS],
            pick_history_index: 0,
            hops_since_pick: u32::MAX,
            pick_window_hops: hops(PICK_WINDOW_SECONDS),
        }
    }

    pub fn set_gate(&mut self, threshold: f32) {
        self.gate = threshold;
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
        self.bank.hop();
        let mut sounding = FixedList::<f32, MAX_DETECTED_NOTES>::default();
        for t in self.tracks.iter().flatten().filter(|t| t.reported) {
            sounding.push(t.note.pitch);
        }
        // A note not yet reported counts too, unless it is near a sounding
        // note (that note's attack settling): a lone note's periodicity
        // estimate would follow it to a pitch between the two.
        let pending_apart = self
            .tracks
            .iter()
            .flatten()
            .filter(|t| {
                !t.reported
                    && !sounding
                        .iter()
                        .any(|&s| (s - t.note.pitch).abs() <= MATCH_RADIUS)
            })
            .count();
        let heard = self.sieve.analyse(
            &self.bank,
            &self.history,
            self.gate,
            &Context {
                sounding: &sounding,
                alone: sounding.len() + pending_apart <= 1,
                picked: self.hops_since_pick <= self.pick_window_hops,
            },
        );
        let found = continuations(&self.tracks, &heard);

        let pick_level = self.pick_meter.level();
        // The ring runs oldest first from `pick_history_index`; the levels at
        // least `PICK_HOPS` hops old are the ones a pick must rise above.
        let before = (0..=PICK_LOOKBACK_HOPS - PICK_HOPS)
            .map(|age| self.pick_history[(self.pick_history_index + age) % PICK_LOOKBACK_HOPS])
            .fold(0.0, f32::max);
        self.hops_since_pick = if pick_level > PICK_JUMP * before && pick_level >= self.gate {
            0
        } else {
            self.hops_since_pick.saturating_add(1)
        };
        self.pick_history[self.pick_history_index] = pick_level;
        self.pick_history_index = (self.pick_history_index + 1) % PICK_LOOKBACK_HOPS;
        let picked = self.hops_since_pick <= self.pick_window_hops;
        let repick_rise = if picked {
            REPICK_RISE_AFTER_PICK
        } else {
            REPICK_RISE
        };

        let mut matched = [false; MAX_DETECTED_NOTES];
        for (slot, found) in self.tracks.iter_mut().zip(found) {
            let Some(slot) = slot else { continue };
            match found {
                Some((i, distance)) => {
                    matched[i] = true;
                    // A pick disturbs the analysis of a sounding note: its
                    // pitch jumps even when its level hasn't risen yet.
                    let jumped =
                        distance > REPICK_JUMP && self.hops_since_pick <= self.pick_window_hops;
                    slot.note.pitch = heard[i].pitch;
                    slot.note.level = heard[i].level;
                    slot.hops_alive += 1;
                    if !slot.reported {
                        slot.clarity = slot.clarity.max(heard[i].clarity);
                    }
                    slot.hops_missed = 0;
                    if slot.reported {
                        slot.hops_since_onset += 1;
                        if slot.hops_since_onset <= self.repick_holdoff_hops {
                            slot.valley = slot.note.level;
                        } else if (picked && slot.note.level > repick_rise * slot.valley) || jumped
                        {
                            slot.note.repicks += 1;
                            slot.note.onset_strength = self.pick_meter.level();
                            slot.hops_since_onset = 0;
                            slot.valley = slot.note.level;
                        } else {
                            slot.valley = slot.valley.min(slot.note.level);
                        }
                    }
                }
                None => slot.hops_missed += 1,
            }
        }

        for slot in &mut self.tracks {
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
                    && (t.clarity >= PICK_CLARITY
                        || t.born_after_pick > self.pick_window_hops
                        || t.hops_alive >= self.unclear_birth_hops)
                {
                    t.reported = true;
                    t.note.onset_strength = self.pick_meter.level();
                }
            }
        }

        for i in (0..heard.len()).filter(|&i| !matched[i]) {
            let Some(free) = self.tracks.iter_mut().find(|s| s.is_none()) else {
                break;
            };
            self.next_id = self.next_id.wrapping_add(1);
            *free = Some(NoteTrack {
                note: DetectedNote {
                    id: self.next_id,
                    pitch: heard[i].pitch,
                    level: heard[i].level,
                    onset_strength: 0.0,
                    repicks: 0,
                },
                hops_alive: 1,
                hops_missed: 0,
                reported: false,
                valley: 0.0,
                hops_since_onset: 0,
                clarity: heard[i].clarity,
                born_after_pick: self.hops_since_pick,
            });
        }
    }

    /// The reported notes, loudest first.
    pub fn notes(&self) -> DetectedNotes {
        let mut notes = DetectedNotes::default();
        for t in self.tracks.iter().flatten().filter(|t| t.reported) {
            notes.push(t.note);
        }
        notes.sort_unstable_by(|a, b| b.level.total_cmp(&a.level));
        notes
    }

    /// A reading only when exactly one note is sounding and has settled.
    pub fn tuner(&self) -> Option<TunerReading> {
        let mut reported = self.tracks.iter().flatten().filter(|t| t.reported);
        let only = reported.next()?;
        if reported.next().is_some() || only.hops_alive < self.birth_hops + self.tuner_settle_hops {
            return None;
        }
        Some(TunerReading::nearest(only.note.pitch))
    }
}

/// Which note heard this hop (index into `heard`, and how far it lies) each
/// track continues as: the nearest one within its match radius.
/// Reported notes choose first: a pending note that took the one a reported
/// note continues as would make it expire and return with a new id mid-note.
fn continuations(
    tracks: &[Option<NoteTrack>; MAX_DETECTED_NOTES],
    heard: &[RawNote],
) -> [Option<(usize, f32)>; MAX_DETECTED_NOTES] {
    let mut taken = [false; MAX_DETECTED_NOTES];
    let mut found = [None; MAX_DETECTED_NOTES];
    let reported_first = [true, false].into_iter().flat_map(|reported| {
        (0..MAX_DETECTED_NOTES)
            .filter(move |&slot| tracks[slot].is_some_and(|t| t.reported == reported))
    });
    for slot in reported_first {
        let Some(t) = tracks[slot] else { continue };
        let radius = if t.reported {
            MATCH_RADIUS
        } else {
            PENDING_MATCH_RADIUS
        };
        let nearest = heard
            .iter()
            .enumerate()
            .filter(|&(i, _)| !taken[i])
            .map(|(i, h)| (i, (h.pitch - t.note.pitch).abs()))
            .filter(|&(_, distance)| distance <= radius)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = nearest {
            taken[i] = true;
        }
        found[slot] = nearest;
    }
    found
}
