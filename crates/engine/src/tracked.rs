//! The tracked layer: each detected note gets a voice whose oscillator plays
//! at the note's exact pitch and whose loudness follows the string.

use std::ops::Deref;

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
    voice: Voice,
    string: StringLevel,
    gate: f32,
    waveshape: Smoothed,
    level: Smoothed,
}

impl TrackedLayer {
    pub fn new(sample_rate: f32) -> Self {
        TrackedLayer {
            voice: Voice::new(sample_rate),
            string: StringLevel::new(sample_rate),
            gate: NOISE_GATE_DEFAULT,
            waveshape: Smoothed::new(Param::TrackedOsc1Waveshape.default_value(), sample_rate),
            level: Smoothed::new(Param::TrackedLevel.default_value(), sample_rate),
        }
    }

    /// Takes an already-clamped value for one of the tracked layer's params.
    pub fn set_param(&mut self, param: Param, value: f32) {
        match param {
            Param::TrackedOsc1Waveshape => self.waveshape.set_target(value.round()),
            Param::TrackedLevel => self.level.set_target(value),
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
        self.voice.update(notes, string_level);
    }

    pub fn process(&mut self) -> f32 {
        let square = self.waveshape.next();
        self.voice.process(square) * self.level.next()
    }

    pub fn voices(&self) -> Voices {
        let mut voices = Voices::default();
        if let Some(reading) = self.voice.reading() {
            voices.voices[0] = reading;
            voices.len = 1;
        }
        voices
    }
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
    sounding: bool,
    pitch: f32,
    target_pitch: f32,
    loudness: f32,
    target_loudness: f32,
    pitch_coefficient: f32,
    loudness_coefficient: f32,
    osc: Oscillator,
    sample_rate: f32,
}

impl Voice {
    fn new(sample_rate: f32) -> Self {
        let coefficient = |seconds: f32| (-1.0 / (seconds * sample_rate)).exp();
        Voice {
            note_id: None,
            sounding: false,
            pitch: 0.0,
            target_pitch: 0.0,
            loudness: 0.0,
            target_loudness: 0.0,
            pitch_coefficient: coefficient(PITCH_SMOOTHING_SECONDS),
            loudness_coefficient: coefficient(LOUDNESS_SMOOTHING_SECONDS),
            osc: Oscillator::default(),
            sample_rate,
        }
    }

    /// Follows its note while the note sounds; otherwise takes the loudest
    /// note heard, or fades out when there is none. Loudness never exceeds
    /// `string_level`, the dry signal's recent peak.
    fn update(&mut self, notes: &[DetectedNote], string_level: f32) {
        let note = self
            .note_id
            .and_then(|id| notes.iter().find(|note| note.id == id))
            .or(notes.first());
        match note {
            Some(note) => {
                if !self.sounding {
                    // A voice starts at its note's pitch and from silence.
                    self.pitch = note.pitch;
                    self.osc = Oscillator::default();
                    self.sounding = true;
                }
                self.note_id = Some(note.id);
                self.target_pitch = note.pitch;
                self.target_loudness = note.level.min(string_level);
            }
            None => {
                self.note_id = None;
                self.target_loudness = 0.0;
            }
        }
    }

    /// `square` crossfades the oscillator from saw (0) to square (1).
    fn process(&mut self, square: f32) -> f32 {
        if !self.sounding {
            return 0.0;
        }
        self.pitch = self.target_pitch + (self.pitch - self.target_pitch) * self.pitch_coefficient;
        self.loudness =
            self.target_loudness + (self.loudness - self.target_loudness) * self.loudness_coefficient;
        if self.note_id.is_none() && self.loudness < SILENT {
            self.sounding = false;
            self.loudness = 0.0;
            return 0.0;
        }
        let frequency = 440.0 * ((self.pitch - 69.0) / 12.0).exp2();
        self.osc.process(frequency / self.sample_rate, square) * self.loudness
    }

    fn reading(&self) -> Option<VoiceReading> {
        self.sounding.then_some(VoiceReading {
            pitch: self.pitch,
            loudness: self.loudness,
        })
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
