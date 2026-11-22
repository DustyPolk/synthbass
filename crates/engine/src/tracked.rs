//! The tracked layer: each detected note gets a voice whose oscillator plays
//! at the note's exact pitch and whose loudness follows the string.

use std::ops::Deref;

use crate::ladder::LadderFilter;
use crate::param::Param;
use crate::smoothed::Smoothed;
use crate::tracker::{DetectedNote, NOISE_GATE_DEFAULT};

/// The most voices that sound at once.
pub const MAX_VOICES: usize = 4;

/// One sounding voice, as the readout shows it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VoiceReading {
    /// Exact pitch as a fractional MIDI note number.
    pub pitch: f32,
    /// Current loudness from string following (linear, ≈ the note's peak
    /// amplitude), before the tracked layer level.
    pub loudness: f32,
}

/// The voices sounding right now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Voices {
    voices: [VoiceReading; MAX_VOICES],
    len: usize,
}

impl Deref for Voices {
    type Target = [VoiceReading];

    fn deref(&self) -> &[VoiceReading] {
        &self.voices[..self.len]
    }
}

pub struct TrackedLayer {
    voices: [Voice; MAX_VOICES],
    string: StringLevel,
    gate: f32,
    waveshape: Smoothed,
    osc2_waveshape: Smoothed,
    /// Oscillator 2's octave plus detune, in octaves; set from both params.
    osc2_octave: f32,
    osc2_detune: f32,
    osc2_log2_ratio: Smoothed,
    osc_mix: Smoothed,
    sub_level: Smoothed,
    log2_cutoff: Smoothed,
    resonance: Smoothed,
    envelope_amount: Smoothed,
    envelope_decay: Smoothed,
    pick_sensitivity: Smoothed,
    level: Smoothed,
    sample_rate: f32,
}

/// The tracked layer's sound settings for one sample, shared by every voice.
struct Sound {
    /// Oscillator 1 from saw (0) to square (1).
    osc1_square: f32,
    osc2_square: f32,
    /// Oscillator 2's frequency as a multiple of oscillator 1's.
    osc2_ratio: f32,
    /// 0 is oscillator 1 alone, 1 is oscillator 2 alone.
    osc_mix: f32,
    sub_level: f32,
    log2_cutoff: f32,
    resonance: f32,
    /// Octaves a full-strength filter envelope opens the filter.
    envelope_octaves: f32,
    /// Per-sample multiplier of the envelope's fall.
    envelope_decay: f32,
    pick_sensitivity: f32,
}

impl TrackedLayer {
    pub fn new(sample_rate: f32) -> Self {
        TrackedLayer {
            voices: std::array::from_fn(|i| Voice::new(sample_rate, i as u32)),
            string: StringLevel::new(sample_rate),
            gate: NOISE_GATE_DEFAULT,
            waveshape: Smoothed::new(Param::TrackedOsc1Waveshape.default_value(), sample_rate),
            osc2_waveshape: Smoothed::new(Param::TrackedOsc2Waveshape.default_value(), sample_rate),
            osc2_octave: Param::TrackedOsc2Octave.default_value(),
            osc2_detune: Param::TrackedOsc2Detune.default_value(),
            osc2_log2_ratio: Smoothed::new(
                osc2_log2_ratio(
                    Param::TrackedOsc2Octave.default_value(),
                    Param::TrackedOsc2Detune.default_value(),
                ),
                sample_rate,
            ),
            osc_mix: Smoothed::new(Param::TrackedOscMix.default_value(), sample_rate),
            sub_level: Smoothed::new(Param::TrackedSubLevel.default_value(), sample_rate),
            // Smoothed in octaves so sweeps sound even across the range.
            log2_cutoff: Smoothed::new(Param::TrackedCutoff.default_value().log2(), sample_rate),
            resonance: Smoothed::new(Param::TrackedResonance.default_value(), sample_rate),
            envelope_amount: Smoothed::new(
                Param::TrackedFilterEnvelopeAmount.default_value(),
                sample_rate,
            ),
            envelope_decay: Smoothed::new(
                Param::TrackedFilterEnvelopeDecay.default_value(),
                sample_rate,
            ),
            pick_sensitivity: Smoothed::new(
                Param::TrackedPickSensitivity.default_value(),
                sample_rate,
            ),
            level: Smoothed::new(Param::TrackedLevel.default_value(), sample_rate),
            sample_rate,
        }
    }

    /// Takes an already-clamped value for one of the tracked layer's params.
    pub fn set_param(&mut self, param: Param, value: f32) {
        match param {
            Param::TrackedOsc1Waveshape => self.waveshape.set_target(value.round()),
            Param::TrackedOsc2Waveshape => self.osc2_waveshape.set_target(value.round()),
            Param::TrackedOsc2Octave | Param::TrackedOsc2Detune => {
                if param == Param::TrackedOsc2Octave {
                    self.osc2_octave = value.round();
                } else {
                    self.osc2_detune = value;
                }
                self.osc2_log2_ratio
                    .set_target(osc2_log2_ratio(self.osc2_octave, self.osc2_detune));
            }
            Param::TrackedOscMix => self.osc_mix.set_target(value),
            Param::TrackedSubLevel => self.sub_level.set_target(value),
            Param::TrackedLevel => self.level.set_target(value),
            Param::TrackedCutoff => self.log2_cutoff.set_target(value.log2()),
            Param::TrackedResonance => self.resonance.set_target(value),
            Param::TrackedFilterEnvelopeAmount => self.envelope_amount.set_target(value),
            Param::TrackedFilterEnvelopeDecay => self.envelope_decay.set_target(value),
            Param::TrackedPickSensitivity => self.pick_sensitivity.set_target(value),
            _ => debug_assert!(false, "{param:?} is not a tracked layer param"),
        }
    }

    pub fn set_gate(&mut self, threshold: f32) {
        self.gate = threshold;
    }

    /// Hands the voices the notes heard now (loudest first) along with the
    /// latest chunk of dry signal.
    pub fn update(&mut self, notes: &[DetectedNote], dry: &[f32]) {
        let string_level = self.string.process(dry);
        // A muted string ends its voices at once, without waiting for the
        // tracker's analysis to ring down.
        let notes = if string_level < self.gate { &[] } else { notes };
        let mut playing: [Option<&DetectedNote>; MAX_VOICES] = [None; MAX_VOICES];
        for (voice, playing) in self.voices.iter().zip(&mut playing) {
            *playing = voice
                .note_id
                .and_then(|id| notes.iter().find(|note| note.id == id));
        }
        // Notes come loudest first, so when all voices are busy a new note
        // takes the voice of the quietest note, if it is louder.
        for note in notes {
            if playing.iter().flatten().any(|p| p.id == note.id) {
                continue;
            }
            let Some(slot) = self.free_voice(&playing).or_else(|| {
                quietest(&playing).filter(|&i| {
                    playing[i].is_some_and(|quiet| note.level > STEAL_MARGIN * quiet.level)
                })
            }) else {
                continue;
            };
            playing[slot] = Some(note);
        }
        for (voice, note) in self.voices.iter_mut().zip(playing) {
            voice.update(note, string_level);
        }
    }

    /// A voice with no note: a silent one if any, else the quietest one
    /// still fading out.
    fn free_voice(&self, playing: &[Option<&DetectedNote>; MAX_VOICES]) -> Option<usize> {
        (0..MAX_VOICES)
            .filter(|&i| playing[i].is_none())
            .min_by(|&a, &b| {
                let loudness = |i: usize| {
                    let voice = &self.voices[i];
                    if voice.sounding { voice.loudness } else { -1.0 }
                };
                loudness(a).total_cmp(&loudness(b))
            })
    }

    pub fn process(&mut self) -> f32 {
        let sound = Sound {
            osc1_square: self.waveshape.next(),
            osc2_square: self.osc2_waveshape.next(),
            osc2_ratio: self.osc2_log2_ratio.next().exp2(),
            osc_mix: self.osc_mix.next(),
            sub_level: self.sub_level.next(),
            log2_cutoff: self.log2_cutoff.next(),
            resonance: self.resonance.next(),
            envelope_octaves: ENVELOPE_OCTAVES * self.envelope_amount.next(),
            // Decay is the time the sweep takes to fall to a tenth.
            envelope_decay: (-std::f32::consts::LN_10
                / (self.envelope_decay.next() * self.sample_rate))
                .exp(),
            pick_sensitivity: self.pick_sensitivity.next(),
        };
        let mix: f32 = self.voices.iter_mut().map(|v| v.process(&sound)).sum();
        mix * self.level.next()
    }

    pub fn voices(&self) -> Voices {
        let mut voices = Voices::default();
        for reading in self.voices.iter().filter_map(Voice::reading) {
            voices.voices[voices.len] = reading;
            voices.len += 1;
        }
        voices
    }
}

/// A new note must be this much louder than the quietest voice's note to
/// take its voice, so two notes of about equal level don't trade a voice
/// back and forth.
const STEAL_MARGIN: f32 = 1.25;

/// The voice playing the quietest note.
fn quietest(playing: &[Option<&DetectedNote>; MAX_VOICES]) -> Option<usize> {
    (0..MAX_VOICES)
        .filter_map(|i| playing[i].map(|note| (i, note.level)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Octaves the filter envelope opens the filter at full amount, on the
/// hardest pick: from the 300 Hz default cutoff past the top of the range.
const ENVELOPE_OCTAVES: f32 = 6.0;

/// Pick strengths (dry signal peak, dBFS) that map to the softest and the
/// hardest pick for pick sensitivity.
const SOFTEST_PICK_DB: f32 = -36.0;
const HARDEST_PICK_DB: f32 = -6.0;

/// How hard a note was picked, 0 (softest) to 1 (hardest), from its onset
/// strength. On a dB scale so soft and hard playing respond evenly.
fn pick_velocity(onset_strength: f32) -> f32 {
    let db = 20.0 * onset_strength.max(1e-9).log10();
    ((db - SOFTEST_PICK_DB) / (HARDEST_PICK_DB - SOFTEST_PICK_DB)).clamp(0.0, 1.0)
}

fn osc2_log2_ratio(octave: f32, detune_cents: f32) -> f32 {
    octave + detune_cents / 1200.0
}

/// Removes the hop-to-hop steps from continuous pitch updates and nothing
/// else, so slides and vibrato come through exactly.
const PITCH_SMOOTHING_SECONDS: f32 = 0.003;
/// String following: how fast loudness moves toward the note's level. Short
/// enough to pass palm mutes and picking dynamics, long enough that the voice
/// fades in and out without a click.
const LOUDNESS_SMOOTHING_SECONDS: f32 = 0.005;
/// A released voice is silent, and free, once its loudness falls below this.
const SILENT: f32 = 1e-5;

struct Voice {
    /// The detected note this voice plays, while it still sounds.
    note_id: Option<u32>,
    /// The `repicks` count of the note when the filter envelope last fired.
    repicks: u32,
    sounding: bool,
    pitch: f32,
    target_pitch: f32,
    loudness: f32,
    target_loudness: f32,
    pitch_coefficient: f32,
    loudness_coefficient: f32,
    osc: Oscillator,
    osc2: Oscillator,
    sub: Oscillator,
    filter: LadderFilter,
    envelope: FilterEnvelope,
    /// Picks oscillator 2's starting phase.
    seed: u32,
    sample_rate: f32,
}

impl Voice {
    /// `index` makes each voice's oscillator 2 phases differ.
    fn new(sample_rate: f32, index: u32) -> Self {
        let coefficient = |seconds: f32| (-1.0 / (seconds * sample_rate)).exp();
        Voice {
            note_id: None,
            repicks: 0,
            sounding: false,
            pitch: 0.0,
            target_pitch: 0.0,
            loudness: 0.0,
            target_loudness: 0.0,
            pitch_coefficient: coefficient(PITCH_SMOOTHING_SECONDS),
            loudness_coefficient: coefficient(LOUDNESS_SMOOTHING_SECONDS),
            osc: Oscillator::default(),
            osc2: Oscillator::default(),
            sub: Oscillator::default(),
            filter: LadderFilter::new(sample_rate),
            envelope: FilterEnvelope::new(sample_rate),
            seed: 0x9e37_79b9_u32.wrapping_add(index.wrapping_mul(0x85eb_ca6b)),
            sample_rate,
        }
    }

    /// Plays `note`, or fades out when there is none. Loudness never exceeds
    /// `string_level`, the dry signal's recent peak.
    fn update(&mut self, note: Option<&DetectedNote>, string_level: f32) {
        match note {
            Some(note) => {
                if !self.sounding {
                    // A voice starts at its note's pitch and from silence.
                    self.pitch = note.pitch;
                    self.filter.reset();
                    self.envelope = FilterEnvelope::new(self.sample_rate);
                    // Starting the sub with oscillator 1 keeps it locked
                    // an octave below.
                    self.osc = Oscillator::default();
                    self.sub = Oscillator::default();
                    // Oscillator 2 starts at a fresh phase each note, so a
                    // detuned unison never starts out cancelling.
                    self.seed = self
                        .seed
                        .wrapping_mul(1_664_525)
                        .wrapping_add(1_013_904_223);
                    self.osc2 = Oscillator {
                        phase: (self.seed >> 8) as f32 / (1 << 24) as f32,
                    };
                    self.sounding = true;
                }
                // Fires for a new note and for the same note picked again.
                if self.note_id != Some(note.id) || self.repicks != note.repicks {
                    self.envelope.trigger(pick_velocity(note.onset_strength));
                }
                self.note_id = Some(note.id);
                self.repicks = note.repicks;
                self.target_pitch = note.pitch;
                self.target_loudness = note.level.min(string_level);
            }
            None => {
                self.note_id = None;
                self.target_loudness = 0.0;
            }
        }
    }

    fn process(&mut self, sound: &Sound) -> f32 {
        if !self.sounding {
            return 0.0;
        }
        self.pitch = self.target_pitch + (self.pitch - self.target_pitch) * self.pitch_coefficient;
        self.loudness = self.target_loudness
            + (self.loudness - self.target_loudness) * self.loudness_coefficient;
        if self.note_id.is_none() && self.loudness < SILENT {
            self.sounding = false;
            self.loudness = 0.0;
            return 0.0;
        }
        let frequency = 440.0 * ((self.pitch - 69.0) / 12.0).exp2();
        let dt = frequency / self.sample_rate;
        let osc1 = self.osc.process(dt, sound.osc1_square);
        let osc2 = self.osc2.process(dt * sound.osc2_ratio, sound.osc2_square);
        let sub = self.sub.process(0.5 * dt, 1.0);
        let oscillators = osc1 + (osc2 - osc1) * sound.osc_mix;
        let mix = (oscillators + sound.sub_level * sub) / (1.0 + sound.sub_level);
        let envelope = self.envelope.process(sound.envelope_decay);
        let pick = 1.0 - sound.pick_sensitivity * (1.0 - self.envelope.velocity);
        let octaves = sound.envelope_octaves * pick * envelope;
        self.filter.set_cutoff((sound.log2_cutoff + octaves).exp2());
        self.filter.set_resonance(sound.resonance);
        self.filter.process(mix) * self.loudness
    }

    fn reading(&self) -> Option<VoiceReading> {
        self.sounding.then_some(VoiceReading {
            pitch: self.pitch,
            loudness: self.loudness,
        })
    }
}

/// The Moog "bwow": jumps open when the voice's note is detected, then
/// falls away exponentially.
struct FilterEnvelope {
    /// 0 (closed) to 1 (fully open).
    value: f32,
    rising: bool,
    /// Rise per sample; the jump takes `ENVELOPE_ATTACK_SECONDS` so it never
    /// clicks.
    attack_step: f32,
    /// How hard the note that fired it was picked, 0..=1.
    velocity: f32,
}

const ENVELOPE_ATTACK_SECONDS: f32 = 0.001;

impl FilterEnvelope {
    fn new(sample_rate: f32) -> Self {
        FilterEnvelope {
            value: 0.0,
            rising: false,
            attack_step: 1.0 / (ENVELOPE_ATTACK_SECONDS * sample_rate),
            velocity: 0.0,
        }
    }

    fn trigger(&mut self, velocity: f32) {
        self.rising = true;
        self.velocity = velocity;
    }

    /// `decay` multiplies the value each sample once it has peaked.
    fn process(&mut self, decay: f32) -> f32 {
        if self.rising {
            self.value += self.attack_step;
            if self.value >= 1.0 {
                self.value = 1.0;
                self.rising = false;
            }
        } else {
            self.value *= decay;
        }
        self.value
    }
}

/// Longer than one period of the lowest note, so the peak does not ripple
/// at the note's frequency.
const STRING_WINDOW_SECONDS: f32 = 0.03;

/// The dry signal's peak over the last `STRING_WINDOW_SECONDS`, kept as the
/// peaks of fixed-length slots. The tracker's note levels lag a sudden mute
/// by over 100 ms on low notes; this falls within the window, so voices fall
/// silent as soon as the string does.
struct StringLevel {
    slot_peaks: Vec<f32>,
    slot: usize,
    filled: usize,
}

const SLOT_SAMPLES: usize = 32;

impl StringLevel {
    fn new(sample_rate: f32) -> Self {
        let slots = (STRING_WINDOW_SECONDS * sample_rate / SLOT_SAMPLES as f32).ceil() as usize;
        StringLevel {
            slot_peaks: vec![0.0; slots],
            slot: 0,
            filled: 0,
        }
    }

    fn process(&mut self, dry: &[f32]) -> f32 {
        for &sample in dry {
            if self.filled == SLOT_SAMPLES {
                self.slot = (self.slot + 1) % self.slot_peaks.len();
                self.slot_peaks[self.slot] = 0.0;
                self.filled = 0;
            }
            let peak = &mut self.slot_peaks[self.slot];
            *peak = peak.max(sample.abs());
            self.filled += 1;
        }
        self.slot_peaks.iter().fold(0.0, |max, &p| max.max(p))
    }
}

/// Band-limited (PolyBLEP) saw and square sharing one phase.
#[derive(Default)]
struct Oscillator {
    phase: f32,
}

impl Oscillator {
    /// `dt` is frequency / sample rate (below 0.5); `square` mixes saw (0)
    /// to square (1).
    fn process(&mut self, dt: f32, square: f32) -> f32 {
        let t = self.phase;
        let saw = 2.0 * t - 1.0 - poly_blep(t, dt);
        let half = if t < 0.5 { t + 0.5 } else { t - 0.5 };
        let pulse = if t < 0.5 { 1.0 } else { -1.0 } + poly_blep(t, dt) - poly_blep(half, dt);
        self.phase += dt;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        saw + (pulse - saw) * square
    }
}

/// Smooths the step at a phase wrap over one sample either side.
fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn note(id: u32, pitch: f32) -> DetectedNote {
        loud_note(id, pitch, 0.3)
    }

    fn loud_note(id: u32, pitch: f32, level: f32) -> DetectedNote {
        DetectedNote {
            id,
            pitch,
            level,
            onset_strength: 0.5,
            repicks: 0,
        }
    }

    /// Hands `notes` (any order) to the layer the way the tracker does,
    /// loudest first, and returns the sounding voices' pitches, low to high.
    fn play(layer: &mut TrackedLayer, notes: &[DetectedNote]) -> Vec<f32> {
        let mut notes = notes.to_vec();
        notes.sort_by(|a, b| b.level.total_cmp(&a.level));
        layer.update(&notes, &[0.5; 32]);
        for _ in 0..32 {
            layer.process();
        }
        // Rounded: a stolen voice glides to its new pitch.
        let mut pitches: Vec<f32> = layer
            .voices()
            .iter()
            .map(|v| (v.pitch * 100.0).round() / 100.0)
            .collect();
        pitches.sort_by(f32::total_cmp);
        pitches
    }

    /// Plays `notes` for `seconds`, so voices that lost their note fade out.
    fn hold(layer: &mut TrackedLayer, notes: &[DetectedNote], seconds: f32) -> Vec<f32> {
        let mut pitches = Vec::new();
        for _ in 0..(seconds * SAMPLE_RATE / 32.0) as usize {
            pitches = play(layer, notes);
        }
        pitches
    }

    #[test]
    fn five_notes_get_four_voices_and_the_quietest_has_none() {
        let mut layer = TrackedLayer::new(SAMPLE_RATE);
        let notes = [
            loud_note(1, 40.0, 0.3),
            loud_note(2, 44.0, 0.05),
            loud_note(3, 47.0, 0.25),
            loud_note(4, 52.0, 0.2),
            loud_note(5, 55.0, 0.15),
        ];
        assert_eq!(hold(&mut layer, &notes, 0.05), [40.0, 47.0, 52.0, 55.0]);
    }

    #[test]
    fn a_fifth_note_replaces_the_quietest_voice() {
        let mut layer = TrackedLayer::new(SAMPLE_RATE);
        let mut notes = vec![
            loud_note(1, 40.0, 0.2),
            loud_note(2, 44.0, 0.05),
            loud_note(3, 47.0, 0.25),
            loud_note(4, 52.0, 0.15),
        ];
        assert_eq!(hold(&mut layer, &notes, 0.05), [40.0, 44.0, 47.0, 52.0]);
        notes.push(loud_note(5, 57.0, 0.3));
        assert_eq!(hold(&mut layer, &notes, 0.05), [40.0, 47.0, 52.0, 57.0]);
    }

    #[test]
    fn notes_of_about_equal_level_keep_their_voices() {
        let mut layer = TrackedLayer::new(SAMPLE_RATE);
        let four = [
            loud_note(1, 40.0, 0.2),
            loud_note(2, 44.0, 0.2),
            loud_note(3, 47.0, 0.2),
            loud_note(4, 52.0, 0.2),
        ];
        hold(&mut layer, &four, 0.05);
        // A fifth note just louder than the rest doesn't steal.
        let mut five = four.to_vec();
        five.push(loud_note(5, 57.0, 0.22));
        assert_eq!(hold(&mut layer, &five, 0.05), [40.0, 44.0, 47.0, 52.0]);
    }

    #[test]
    fn a_new_note_takes_a_silent_voice_not_one_still_fading() {
        let mut layer = TrackedLayer::new(SAMPLE_RATE);
        play(&mut layer, &[note(1, 40.0)]);
        // Note 1 ends; its voice starts fading as note 2 arrives.
        assert_eq!(play(&mut layer, &[note(2, 45.0)]), [40.0, 45.0]);
    }

    fn plucky() -> Sound {
        Sound {
            osc1_square: 0.0,
            osc2_square: 0.0,
            osc2_ratio: 1.0,
            osc_mix: 0.0,
            sub_level: 0.0,
            log2_cutoff: 150f32.log2(),
            resonance: 0.0,
            envelope_octaves: ENVELOPE_OCTAVES,
            envelope_decay: (-std::f32::consts::LN_10 / (0.1 * SAMPLE_RATE)).exp(),
            pick_sensitivity: 0.0,
        }
    }

    fn brightness(signal: &[f32]) -> f32 {
        let energy: f32 = signal.iter().map(|s| s * s).sum();
        let difference: f32 = signal.windows(2).map(|w| (w[1] - w[0]).powi(2)).sum();
        difference / energy
    }

    /// Two voices whose notes start 0.2 s apart each get their own sweep:
    /// when the second starts, it is bright while the first has closed.
    #[test]
    fn voices_starting_at_different_times_sweep_independently() {
        let sound = plucky();
        let (mut first, mut second) = (Voice::new(SAMPLE_RATE, 0), Voice::new(SAMPLE_RATE, 1));
        let late = (0.2 * SAMPLE_RATE) as usize;
        let window = (0.04 * SAMPLE_RATE) as usize;
        let mut outputs = (Vec::new(), Vec::new());
        first.update(Some(&note(1, 33.0)), 1.0);
        for n in 0..late + window {
            if n == late {
                second.update(Some(&note(2, 33.0)), 1.0);
            }
            outputs.0.push(first.process(&sound));
            outputs.1.push(second.process(&sound));
        }
        let first_start = brightness(&outputs.0[100..window]);
        let first_late = brightness(&outputs.0[late..]);
        let second_start = brightness(&outputs.1[late + 100..]);
        assert!(
            first_start > 3.0 * first_late,
            "{first_start} vs {first_late}"
        );
        assert!(
            second_start > 3.0 * first_late,
            "{second_start} vs {first_late}"
        );
    }
}
