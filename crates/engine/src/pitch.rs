//! Pitch as a fractional MIDI note number (A4 = 69 = 440 Hz) and as Hz.

/// MIDI note number to frequency in Hz.
pub fn midi_to_hz(midi: f32) -> f32 {
    440.0 * 2f32.powf((midi - 69.0) / 12.0)
}

/// Frequency in Hz to fractional MIDI note number.
pub fn hz_to_midi(hz: f32) -> f32 {
    69.0 + 12.0 * (hz / 440.0).log2()
}
