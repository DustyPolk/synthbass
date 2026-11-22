//! Note detection and tuner, driven through the engine's public interface
//! with synthetic bass-like tones.

mod common;

use common::*;
use synthbass_engine::NoiseGate;

#[test]
fn silence_gives_no_tuner_reading_and_no_notes() {
    let mut engine = engine();
    render(&mut engine, &vec![0.0; 24_000], &[128]);
    let readout = engine.readout();
    assert_eq!(readout.tuner, None);
    assert!(readout.notes.is_empty());
}

/// Cents between a fractional MIDI pitch and an equal-tempered note.
fn cents_off(pitch: f32, note: f32) -> f32 {
    (pitch - note) * 100.0
}

#[test]
fn tuner_reads_a_held_bass_note() {
    let mut engine = engine();
    render(&mut engine, &bass_tone(hz(33.0), 0.5, 1.0, 0.5), &[128]);
    let readout = engine.readout();
    let tuner = readout.tuner.expect("a tuner reading");
    assert_eq!((tuner.name(), tuner.octave()), ("A", 1));
    assert!(tuner.cents.abs() < 3.0, "cents {}", tuner.cents);
    assert_eq!(readout.notes.len(), 1, "notes {:?}", &readout.notes[..]);
    let note = readout.notes[0];
    assert!(
        cents_off(note.pitch, 33.0).abs() < 3.0,
        "pitch {}",
        note.pitch
    );
}

const LOW_E: f32 = 28.0;
/// G string, 24th fret.
const TOP_OF_G_STRING: f32 = 67.0;
/// "Within a few cents" for a settled note.
const CENTS_TOLERANCE: f32 = 3.0;

/// Plays `pitch` for 0.4 s on a fresh engine and returns the worst way the
/// readout got it wrong, if any.
fn check_held_note(pitch: f32, fundamental: f32) -> Result<f32, String> {
    let mut engine = engine();
    render(
        &mut engine,
        &bass_tone(hz(pitch), 0.4, fundamental, 0.4),
        &[64, 128, 37],
    );
    let readout = engine.readout();
    let [note] = &readout.notes[..] else {
        return Err(format!(
            "{pitch}: notes {:?}",
            readout.notes.iter().map(|n| n.pitch).collect::<Vec<_>>()
        ));
    };
    let error = cents_off(note.pitch, pitch);
    let tuner = readout.tuner.ok_or(format!("{pitch}: no tuner reading"))?;
    let tuner_error = cents_off(tuner.note as f32 + tuner.cents / 100.0, pitch);
    if error.abs() > CENTS_TOLERANCE || tuner_error.abs() > CENTS_TOLERANCE {
        return Err(format!(
            "{pitch}: off by {error:.1} c (tuner {tuner_error:.1} c)"
        ));
    }
    Ok(error.abs())
}

fn check_every_semitone(offset: f32, fundamental: f32) {
    let mut worst = 0.0f32;
    let mut failures = Vec::new();
    for semitone in LOW_E as u8..=TOP_OF_G_STRING as u8 {
        match check_held_note(semitone as f32 + offset, fundamental) {
            Ok(error) => worst = worst.max(error),
            Err(failure) => failures.push(failure),
        }
    }
    println!("offset {offset}, fundamental {fundamental}: worst {worst:.2} c");
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn every_semitone_from_low_e_to_the_top_of_the_g_string_is_detected() {
    check_every_semitone(0.0, 1.0);
}

#[test]
fn notes_between_semitones_are_read_exactly() {
    check_every_semitone(0.17, 1.0);
}

#[test]
fn notes_with_a_weak_fundamental_are_detected() {
    check_every_semitone(0.0, 0.3);
}

/// Plays `pitch` in 64-sample blocks and returns how long after the onset
/// the note was first reported. Every report during the attack must be that
/// note alone.
fn detection_latency_ms(pitch: f32, fundamental: f32) -> f32 {
    const BLOCK: usize = 64;
    let mut engine = engine();
    let tone = bass_tone(hz(pitch), 0.4, fundamental, 0.2);
    let mut left = [0.0; BLOCK];
    let mut right = [0.0; BLOCK];
    let mut first = None;
    for (i, block) in tone.chunks(BLOCK).enumerate() {
        let n = block.len();
        engine.process(block, &mut left[..n], &mut right[..n]);
        let readout = engine.readout();
        let pitches: Vec<f32> = readout.notes.iter().map(|n| n.pitch).collect();
        match pitches[..] {
            [] => assert!(first.is_none(), "note dropped during the attack"),
            [p] => {
                assert_eq!(p.round(), pitch, "wrong note {p} during the attack");
                first.get_or_insert((i + 1) * BLOCK);
            }
            _ => panic!("extra notes during the attack: {pitches:?}"),
        }
    }
    first.expect("note never detected") as f32 / SAMPLE_RATE * 1000.0
}

#[test]
fn low_e_is_detected_within_about_two_periods() {
    let two_periods_ms = 2.0 * 1000.0 / hz(LOW_E);
    let bound_ms = two_periods_ms + 10.0;
    for fundamental in [1.0, 0.3] {
        let latency = detection_latency_ms(LOW_E, fundamental);
        println!("low E latency, fundamental {fundamental}: {latency:.1} ms");
        assert!(
            latency <= bound_ms,
            "latency {latency:.1} ms > {bound_ms:.1} ms"
        );
    }
}

/// True if any block's readout reported a note or a tuner reading.
fn anything_detected(engine: &mut synthbass_engine::Engine, signal: &[f32]) -> bool {
    let mut left = [0.0; 128];
    let mut right = [0.0; 128];
    signal.chunks(128).any(|block| {
        let n = block.len();
        engine.process(block, &mut left[..n], &mut right[..n]);
        let readout = engine.readout();
        !readout.notes.is_empty() || readout.tuner.is_some()
    })
}

#[test]
fn nothing_below_the_noise_gate_is_detected() {
    for pitch in [LOW_E, 45.0, TOP_OF_G_STRING] {
        let mut engine = engine();
        engine.set_noise_gate(NoiseGate::new(0.05).unwrap());
        assert!(
            !anything_detected(&mut engine, &bass_tone(hz(pitch), 0.03, 1.0, 0.5)),
            "{pitch} detected below the gate"
        );
        assert!(
            anything_detected(&mut engine, &bass_tone(hz(pitch), 0.3, 1.0, 0.5)),
            "{pitch} not detected above the gate"
        );
    }
}

#[test]
fn a_note_decaying_below_the_noise_gate_ends() {
    let mut engine = engine();
    engine.set_noise_gate(NoiseGate::new(0.05).unwrap());
    // Decays with a 1.5 s time constant from 0.2: below 0.05 after ~2 s.
    render(&mut engine, &bass_tone(hz(33.0), 0.2, 1.0, 3.0), &[128]);
    let readout = engine.readout();
    assert!(readout.notes.is_empty(), "notes {:?}", &readout.notes[..]);
    assert_eq!(readout.tuner, None);
}

#[test]
fn hiss_below_the_default_noise_gate_is_not_detected() {
    let mut engine = engine();
    let mut seed = 1u32;
    let hiss: Vec<f32> = (0..48_000)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            ((seed >> 8) as f32 / (1 << 24) as f32 - 0.5) * 0.002
        })
        .collect();
    assert!(!anything_detected(&mut engine, &hiss));
}

#[test]
fn a_double_stop_gives_two_notes_and_no_tuner_reading() {
    let mut engine = engine();
    let low = bass_tone(hz(LOW_E), 0.3, 1.0, 0.4);
    let high = bass_tone(hz(44.0), 0.3, 1.0, 0.4);
    let both: Vec<f32> = low.iter().zip(&high).map(|(a, b)| a + b).collect();
    render(&mut engine, &both, &[128]);
    let readout = engine.readout();
    let mut pitches: Vec<f32> = readout.notes.iter().map(|n| n.pitch).collect();
    pitches.sort_by(f32::total_cmp);
    assert_eq!(pitches.len(), 2, "pitches {pitches:?}");
    assert!(
        cents_off(pitches[0], LOW_E).abs() < 10.0,
        "pitches {pitches:?}"
    );
    assert!(
        cents_off(pitches[1], 44.0).abs() < 10.0,
        "pitches {pitches:?}"
    );
    assert_eq!(readout.tuner, None);
}

/// The loudest note's (id, repicks) after each block of `dry`.
fn onsets(dry: &[f32]) -> Vec<Option<(u32, u32)>> {
    let mut engine = engine();
    dry.chunks(64)
        .map(|block| {
            render(&mut engine, block, &[64]);
            engine.readout().notes.first().map(|n| (n.id, n.repicks))
        })
        .collect()
}

fn repick_counts(dry: &[f32]) -> Vec<u32> {
    onsets(dry)
        .iter()
        .map(|o| o.map_or(0, |(_, r)| r))
        .collect()
}

/// Picking a sounding note again shows as a new onset: the note counts a
/// repick, or (when the pick disturbs the analysis enough) it is heard as a
/// new note.
#[test]
fn picking_a_sounding_note_again_is_a_new_onset() {
    for note in [28.0, 45.0, 60.0] {
        let mut dry = bass_tone(hz(note), 0.4, 1.0, 0.6);
        dry.extend(bass_tone(hz(note), 0.4, 1.0, 0.4));
        let onsets = onsets(&dry);
        let repick_block = (0.6 * SAMPLE_RATE) as usize / 64;
        let before = onsets[repick_block - 1].expect("the note sounds");
        assert_eq!(before.1, 0, "note {note}");
        let detected = onsets[repick_block..]
            .iter()
            .position(|o| o.is_some_and(|(id, repicks)| id != before.0 || repicks > 0));
        let latency_ms = detected.map(|blocks| blocks as f32 * 64.0 / SAMPLE_RATE * 1000.0);
        assert!(
            latency_ms.is_some_and(|ms| ms < 40.0),
            "note {note}: {latency_ms:?}"
        );
        let last = onsets.last().unwrap().expect("the note sounds");
        assert!(last.1 <= 1, "note {note}: one onset only, {last:?}");
    }
}

/// A plucked tone whose pitch swings ±`depth` semitones at `rate` Hz.
fn vibrato_tone(note: f32, depth: f32, rate: f32, seconds: f32) -> Vec<f32> {
    let len = (seconds * SAMPLE_RATE) as usize;
    let mut phase = 0.0f32;
    (0..len)
        .map(|n| {
            let t = n as f32 / SAMPLE_RATE;
            let pitch = note + depth * (std::f32::consts::TAU * rate * t).sin();
            phase = (phase + hz(pitch) / SAMPLE_RATE).fract();
            let tone: f32 = (1..=8)
                .map(|h| (std::f32::consts::TAU * phase * h as f32).sin() / h as f32)
                .sum();
            0.25 * tone * (-t / 1.5).exp()
        })
        .collect()
}

#[test]
fn decay_vibrato_and_slides_are_not_repicks() {
    let held = bass_tone(hz(33.0), 0.4, 1.0, 2.0);
    let vibrato = vibrato_tone(40.0, 0.3, 5.5, 2.0);
    let slide = {
        let len = 2.0 * SAMPLE_RATE;
        let mut phase = 0.0f32;
        (0..len as usize)
            .map(|n| {
                let pitch = 40.0 + 5.0 * n as f32 / len;
                phase = (phase + hz(pitch) / SAMPLE_RATE).fract();
                let tone: f32 = (1..=8)
                    .map(|h| (std::f32::consts::TAU * phase * h as f32).sin() / h as f32)
                    .sum();
                0.25 * tone * (-(n as f32) / SAMPLE_RATE / 1.5).exp()
            })
            .collect::<Vec<f32>>()
    };
    for (name, dry) in [("held", held), ("vibrato", vibrato), ("slide", slide)] {
        let counts = repick_counts(&dry);
        assert!(
            counts.iter().all(|&c| c == 0),
            "{name}: {:?}",
            counts.iter().max()
        );
    }
}

/// A1 with harmonics 1–6 at `amplitude` for `seconds_full`, after which
/// every harmonic but the second fades out within 20 ms. The second goes on
/// at `remnant` for `seconds_after`, unbroken.
fn lower_partials_die_first(
    amplitude: f32,
    remnant: f32,
    seconds_full: f32,
    seconds_after: f32,
) -> Vec<f32> {
    let f0 = hz(33.0);
    let fade_start = (seconds_full * SAMPLE_RATE) as usize;
    let fade_len = (0.02 * SAMPLE_RATE) as usize;
    let len = ((seconds_full + seconds_after) * SAMPLE_RATE) as usize;
    (0..len)
        .map(|n| {
            let t = n as f32 / SAMPLE_RATE;
            let fade = 1.0 - (n.saturating_sub(fade_start) as f32 / fade_len as f32).min(1.0);
            (1..=6)
                .map(|h| {
                    let gain = if h == 2 {
                        amplitude + (remnant - amplitude) * (1.0 - fade)
                    } else {
                        amplitude * fade
                    };
                    gain / h as f32 * (std::f32::consts::TAU * f0 * h as f32 * t).sin()
                })
                .sum::<f32>()
        })
        .collect()
}

#[test]
fn an_upper_harmonic_left_below_the_noise_gate_does_not_keep_its_note() {
    let mut engine = engine();
    engine.set_noise_gate(NoiseGate::new(0.05).unwrap());
    // The second harmonic stays at 0.07 / 2 = 0.035: under the gate, over
    // the half of it the partial search keeps.
    let dry = lower_partials_die_first(0.3, 0.07, 0.4, 0.4);
    render(&mut engine, &dry[..(0.35 * SAMPLE_RATE) as usize], &[128]);
    let playing = engine.readout().notes;
    assert!(
        playing.len() == 1 && (playing[0].pitch - 33.0).abs() < 0.1,
        "notes before the fade {:?}",
        &playing[..]
    );
    render(&mut engine, &dry[(0.35 * SAMPLE_RATE) as usize..], &[128]);
    let readout = engine.readout();
    assert!(readout.notes.is_empty(), "notes {:?}", &readout.notes[..]);
}

#[test]
fn an_upper_harmonic_left_above_the_noise_gate_keeps_its_note() {
    let mut engine = engine();
    engine.set_noise_gate(NoiseGate::new(0.05).unwrap());
    render(
        &mut engine,
        &lower_partials_die_first(0.3, 0.2, 0.4, 0.4),
        &[128],
    );
    let notes = engine.readout().notes;
    assert!(
        notes.len() == 1 && (notes[0].pitch - 33.0).abs() < 0.1,
        "notes {:?}",
        &notes[..]
    );
}
