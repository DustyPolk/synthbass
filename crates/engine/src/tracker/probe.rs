//! Offline probes on the recorded clips, run by hand (ignored tests):
//!
//! - `SPECTRUM=clip:t0:t1[,…] cargo test --release -p synthbass-engine --lib probe::spectrum -- --ignored --nocapture`
//!   prints the partials of a stretch of a clip (Hann-windowed DFT peaks,
//!   30–800 Hz, 0.1 Hz steps), to check what was played.
//! - `DUMP=clip:t0:t1 … probe::notes …` prints the tracker's notes whenever
//!   their set changes.
//! - `CLIPS=a,b … probe::repicks …` lists the repicks the tracker reports.

use super::Tracker;
use crate::pitch::hz_to_midi;
use crate::readout::TunerReading;

fn load(name: &str) -> (Vec<f32>, f32) {
    let path = format!("{}/../../clips/{name}.wav", env!("CARGO_MANIFEST_DIR"));
    let mut reader = hound::WavReader::open(&path).unwrap();
    let spec = reader.spec();
    let dry: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.unwrap() as f32 * scale)
                .collect()
        }
    };
    (dry, spec.sample_rate as f32)
}

/// `clip:t0:t1` → (clip, t0, t1).
fn span(text: &str) -> (&str, f32, f32) {
    let parts: Vec<&str> = text.split(':').collect();
    (
        parts[0],
        parts[1].parse().unwrap(),
        parts[2].parse().unwrap(),
    )
}

/// A pitch as a note name and cents, e.g. `A1 -19¢`.
fn name(midi: f32) -> String {
    TunerReading::nearest(midi).to_string()
}

#[test]
#[ignore]
fn spectrum() {
    for item in std::env::var("SPECTRUM").unwrap().split(',') {
        let (clip, t0, t1) = span(item);
        let (dry, rate) = load(clip);
        let x = &dry[(t0 * rate) as usize..(t1 * rate) as usize];
        let n = x.len();
        let windowed: Vec<f64> = x
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                (0.5 - 0.5 * (std::f64::consts::TAU * i as f64 / n as f64).cos()) * v as f64
            })
            .collect();
        let freqs: Vec<f64> = (0..7701).map(|k| 30.0 + k as f64 * 0.1).collect();
        // Goertzel at each frequency; 4/n scales a Hann-windowed sinusoid
        // to its amplitude.
        let mags: Vec<f64> = freqs
            .iter()
            .map(|&f| {
                let c = 2.0 * (std::f64::consts::TAU * f / rate as f64).cos();
                let (mut s1, mut s2) = (0.0, 0.0);
                for &v in &windowed {
                    let s = v + c * s1 - s2;
                    s2 = s1;
                    s1 = s;
                }
                (s1 * s1 + s2 * s2 - c * s1 * s2).sqrt() * 4.0 / n as f64
            })
            .collect();
        let max = mags.iter().cloned().fold(0.0, f64::max);
        println!("{item}");
        for i in 1..mags.len() - 1 {
            if mags[i] > mags[i - 1] && mags[i] >= mags[i + 1] && mags[i] > max * 0.01 {
                println!(
                    "  {:7.1} Hz {:>9} {:6.1} dB",
                    freqs[i],
                    name(hz_to_midi(freqs[i] as f32)),
                    20.0 * (mags[i] / max).log10()
                );
            }
        }
    }
}

#[test]
#[ignore]
fn notes() {
    let spec = std::env::var("DUMP").unwrap();
    let (clip, t0, t1) = span(&spec);
    let (dry, rate) = load(clip);
    let mut tracker = Tracker::new(rate);
    let mut last = String::new();
    for (i, chunk) in dry.chunks(64).enumerate() {
        tracker.process(chunk);
        let t = (i * 64) as f32 / rate;
        if t < t0 || t > t1 {
            continue;
        }
        let notes = tracker.notes();
        let mut key: Vec<String> = notes
            .iter()
            .map(|n| format!("{}#{}", n.pitch.round(), n.id))
            .collect();
        key.sort();
        let key = key.join(" ");
        if key != last {
            let shown: Vec<String> = notes
                .iter()
                .map(|n| format!("{}#{} {:.4} r{}", name(n.pitch), n.id, n.level, n.repicks))
                .collect();
            println!("{t:.3} => {}", shown.join(", "));
            last = key;
        }
    }
}

#[test]
#[ignore]
fn repicks() {
    for clip in std::env::var("CLIPS").unwrap().split(',') {
        let (dry, rate) = load(clip);
        let mut tracker = Tracker::new(rate);
        let mut seen = std::collections::HashMap::<u32, u32>::new();
        let mut times = Vec::new();
        for (i, chunk) in dry.chunks(64).enumerate() {
            tracker.process(chunk);
            for n in tracker.notes().iter() {
                if n.repicks > seen.insert(n.id, n.repicks).unwrap_or(0) {
                    times.push(format!("{:.2}:{}", (i * 64) as f32 / rate, name(n.pitch)));
                }
            }
        }
        println!("{clip}: {} repicks: {}", times.len(), times.join(" "));
    }
}
