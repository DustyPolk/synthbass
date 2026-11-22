//! Records the dry signal from a JACK input to a WAV file until Ctrl-C.
//!
//! Usage: `record <output.wav> [--port <jack port>]`. The port defaults to
//! Scarlett input 1. The file is mono 32-bit float at the session sample rate.

use std::io::{Seek, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use jack::{AudioIn, Client, ClientOptions, Control, PortFlags, PortSpec, ProcessScope};
use rtrb::{Consumer, Producer, RingBuffer};

const CLIENT_NAME: &str = "synthbass-record";
const INTERFACE_PATTERN: &str = "Scarlett";
/// Seconds of audio the ring between the JACK thread and the writer holds.
const RING_SECONDS: usize = 10;

const USAGE: &str = "usage: record <output.wav> [--port <jack port>]
Records the dry signal until Ctrl-C. The port defaults to Scarlett input 1.";

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match record(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("record: {message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq)]
struct Args {
    output: PathBuf,
    port: Option<String>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut output = None;
    let mut port = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => port = Some(args.next().ok_or("--port needs a port name")?),
            "-h" | "--help" => return Err(String::new()),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            _ if output.is_some() => return Err(format!("unexpected argument {arg}")),
            _ => output = Some(PathBuf::from(arg)),
        }
    }
    Ok(Args {
        output: output.ok_or("missing output path")?,
        port,
    })
}

fn record(args: &Args) -> Result<(), String> {
    let (client, _) = Client::new(CLIENT_NAME, ClientOptions::NO_START_SERVER)
        .map_err(|e| format!("audio server not reachable: {e}"))?;
    let source = match &args.port {
        Some(port) => port.clone(),
        None => default_source(&client)?,
    };
    if client.port_by_name(&source).is_none() {
        return Err(format!(
            "no port named {source:?}. Capture ports:\n  {}",
            capture_ports(&client, None).join("\n  ")
        ));
    }

    let sample_rate = client.sample_rate();
    let (producer, mut consumer) = RingBuffer::new(sample_rate as usize * RING_SECONDS);
    let dropped = Arc::new(AtomicUsize::new(0));
    let input = client
        .register_port("in", AudioIn::default())
        .map_err(|e| e.to_string())?;
    let input_name = input.name().map_err(|e| e.to_string())?;
    let mut wav = hound::WavWriter::create(&args.output, wav_spec(sample_rate))
        .map_err(|e| format!("{}: {e}", args.output.display()))?;

    let client = client
        .activate_async(
            (),
            Capture {
                input,
                samples: producer,
                dropped: dropped.clone(),
            },
        )
        .map_err(|e| e.to_string())?;
    client
        .as_client()
        .connect_ports_by_name(&source, &input_name)
        .map_err(|e| format!("could not connect {source}: {e}"))?;

    let stop = Arc::new(AtomicBool::new(false));
    let on_ctrl_c = stop.clone();
    ctrlc::set_handler(move || on_ctrl_c.store(true, Ordering::Release))
        .map_err(|e| e.to_string())?;
    eprintln!(
        "Recording {source} at {sample_rate} Hz to {}. Press Ctrl-C to stop.",
        args.output.display()
    );

    let written = write_until_stopped(&mut consumer, &mut wav, &stop);
    drop(client);
    let summary = written.map_err(|e| e.to_string())?;
    wav.finalize().map_err(|e| e.to_string())?;

    let peak_db = 20.0 * summary.peak.max(1e-10).log10();
    eprintln!(
        "Wrote {:.1} s to {} (peak {peak_db:.1} dBFS).",
        summary.frames as f64 / sample_rate as f64,
        args.output.display()
    );
    let dropped = dropped.load(Ordering::Relaxed);
    if dropped > 0 {
        eprintln!("Warning: {dropped} samples were dropped; the clip has gaps.");
    }
    Ok(())
}

/// Scarlett input 1: the first physical Scarlett capture port.
fn default_source(client: &Client) -> Result<String, String> {
    capture_ports(client, Some(INTERFACE_PATTERN))
        .into_iter()
        .next()
        .ok_or_else(|| {
            format!(
                "Scarlett not found; pass --port. Capture ports:\n  {}",
                capture_ports(client, None).join("\n  ")
            )
        })
}

fn capture_ports(client: &Client, pattern: Option<&str>) -> Vec<String> {
    let spec = AudioIn::default();
    client.ports(
        pattern,
        Some(spec.jack_port_type()),
        PortFlags::IS_OUTPUT | PortFlags::IS_PHYSICAL,
    )
}

fn wav_spec(sample_rate: u32) -> hound::WavSpec {
    hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    }
}

/// The JACK side: copies each period into the ring. Never blocks; a full ring
/// drops samples and counts them.
struct Capture {
    input: jack::Port<AudioIn>,
    samples: Producer<f32>,
    dropped: Arc<AtomicUsize>,
}

impl jack::ProcessHandler for Capture {
    fn process(&mut self, _: &Client, scope: &ProcessScope) -> Control {
        let mut dropped = 0;
        for &sample in self.input.as_slice(scope) {
            if self.samples.push(sample).is_err() {
                dropped += 1;
            }
        }
        if dropped > 0 {
            self.dropped.fetch_add(dropped, Ordering::Relaxed);
        }
        Control::Continue
    }
}

#[derive(Debug)]
struct Summary {
    frames: usize,
    peak: f32,
}

/// Moves samples from the ring into the WAV until `stop` is set, then writes
/// whatever was already queued.
fn write_until_stopped<W: Write + Seek>(
    samples: &mut Consumer<f32>,
    wav: &mut hound::WavWriter<W>,
    stop: &AtomicBool,
) -> hound::Result<Summary> {
    let mut summary = Summary {
        frames: 0,
        peak: 0.0,
    };
    loop {
        // Read the flag before draining so every sample queued before the
        // stop is written.
        let stopping = stop.load(Ordering::Acquire);
        let queued = samples.slots();
        let chunk = samples.read_chunk(queued).expect("slots are available");
        for sample in chunk {
            wav.write_sample(sample)?;
            summary.peak = summary.peak.max(sample.abs());
        }
        summary.frames += queued;
        if stopping {
            return Ok(summary);
        }
        if queued == 0 {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    fn wav_writer(buffer: &mut Cursor<Vec<u8>>) -> hound::WavWriter<&mut Cursor<Vec<u8>>> {
        hound::WavWriter::new(buffer, wav_spec(48_000)).unwrap()
    }

    #[test]
    fn args_take_an_output_path_and_an_optional_port() {
        let parse = |args: &[&str]| parse_args(args.iter().map(|a| a.to_string()));
        assert_eq!(
            parse(&["clips/open-strings.wav"]),
            Ok(Args {
                output: "clips/open-strings.wav".into(),
                port: None
            })
        );
        assert_eq!(
            parse(&["--port", "system:capture_2", "x.wav"]),
            Ok(Args {
                output: "x.wav".into(),
                port: Some("system:capture_2".into())
            })
        );
        assert!(parse(&[]).is_err());
        assert!(parse(&["x.wav", "--port"]).is_err());
        assert!(parse(&["x.wav", "y.wav"]).is_err());
    }

    #[test]
    fn samples_still_queued_when_stopped_are_written_before_the_wav_is_finalized() {
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(1024);
        let recorded = [0.1, -0.5, 0.25, 0.0];
        for s in recorded {
            producer.push(s).unwrap();
        }
        let stop = AtomicBool::new(true);

        let mut buffer = Cursor::new(Vec::new());
        let mut wav = wav_writer(&mut buffer);
        let summary = write_until_stopped(&mut consumer, &mut wav, &stop).unwrap();
        wav.finalize().unwrap();

        assert_eq!(summary.frames, 4);
        assert_eq!(summary.peak, 0.5);
        buffer.set_position(0);
        let mut reader = hound::WavReader::new(buffer).unwrap();
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.spec().channels, 1);
        let samples: Vec<f32> = reader.samples().map(Result::unwrap).collect();
        assert_eq!(samples, recorded);
    }

    #[test]
    fn recording_keeps_writing_until_stopped() {
        let (mut producer, mut consumer) = rtrb::RingBuffer::new(1024);
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let feeder = {
            let stop = stop.clone();
            std::thread::spawn(move || {
                // More samples than the ring holds, so the writer must drain
                // while recording rather than only at the end.
                for _ in 0..4 {
                    for _ in 0..1000 {
                        producer.push(0.2).unwrap();
                    }
                    while producer.slots() < 1000 {
                        std::thread::yield_now();
                    }
                }
                stop.store(true, std::sync::atomic::Ordering::Release);
            })
        };

        let mut buffer = Cursor::new(Vec::new());
        let mut wav = wav_writer(&mut buffer);
        let summary = write_until_stopped(&mut consumer, &mut wav, &stop).unwrap();
        feeder.join().unwrap();

        assert_eq!(summary.frames, 4000);
    }
}
