//! Plays patches over a recorded clip offline and measures each against the
//! dry signal. For hearing and checking patches without the bass or the app.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use synthbass::patches::{self, PatchLibrary};
use synthbass::render::{measure, render};
use synthbass_engine::Patch;

const USAGE: &str =
    "usage: render --clip <name|file.wav> [--patch <name|file.toml|all>]... [--out-dir <dir>]
Plays patches over a recorded clip offline and prints, for each, its loudness
against the dry signal, its peak, and its similarity to the dry signal (above
about 0.8 it sounds like the dry signal). A clip name is a file in clips/. The
patches default to all, built-in and saved. With --out-dir, writes
<clip>--<patch>.wav for each patch and <clip>.wav for the dry signal.";

fn main() -> ExitCode {
    let args = match parse_args(std::env::args().skip(1)) {
        Ok(Command::Render(args)) => args,
        Ok(Command::Help) => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        Err(message) => {
            eprintln!("{message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("render: {message}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, PartialEq)]
enum Command {
    Render(Args),
    Help,
}

#[derive(Debug, PartialEq)]
struct Args {
    clip: String,
    /// As given: a patch name, a `.toml` path, or `all`. Empty means all.
    patches: Vec<String>,
    out_dir: Option<PathBuf>,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Command, String> {
    let mut clip = None;
    let mut patches = vec![];
    let mut out_dir = None;
    while let Some(arg) = args.next() {
        let mut value = || args.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--clip" => clip = Some(value()?),
            "--patch" => patches.push(value()?),
            "--out-dir" => out_dir = Some(PathBuf::from(value()?)),
            "-h" | "--help" => return Ok(Command::Help),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            _ => return Err(format!("unexpected argument {arg}")),
        }
    }
    Ok(Command::Render(Args {
        clip: clip.ok_or("missing --clip")?,
        patches,
        out_dir,
    }))
}

/// A clip name means `clips/<name>.wav`; anything with a `/` or ending in
/// `.wav` is a path.
fn clip_path(spec: &str) -> PathBuf {
    if spec.contains('/') || spec.ends_with(".wav") {
        PathBuf::from(spec)
    } else {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../clips")
            .join(format!("{spec}.wav"))
    }
}

/// The first channel of a WAV file, and its sample rate.
fn read_wav(path: &Path) -> Result<(Vec<f32>, u32), String> {
    let mut reader =
        hound::WavReader::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let spec = reader.spec();
    let samples: Result<Vec<f32>, _> = match spec.sample_format {
        hound::SampleFormat::Float => reader.samples::<f32>().collect(),
        hound::SampleFormat::Int => {
            let scale = 1.0 / (1i64 << (spec.bits_per_sample - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|s| s as f32 * scale))
                .collect()
        }
    };
    let samples = samples.map_err(|e| format!("{}: {e}", path.display()))?;
    let first = samples
        .chunks(spec.channels.into())
        .map(|frame| frame[0])
        .collect();
    Ok((first, spec.sample_rate))
}

fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let fail = |e: hound::Error| format!("{}: {e}", path.display());
    let mut writer = hound::WavWriter::create(path, spec).map_err(fail)?;
    for &sample in samples {
        writer.write_sample(sample).map_err(fail)?;
    }
    writer.finalize().map_err(fail)
}

/// The file each patch's render is written to: `<clip>--<patch>.wav`, with
/// a number added where two patch names would make the same file. The dry
/// signal goes to `<clip>.wav`, which no patch file can be.
fn output_names(clip: &str, patches: &[Patch]) -> Vec<String> {
    let mut names: Vec<String> = vec![];
    for patch in patches {
        let stem = format!("{clip}--{}", slug(&patch.name));
        let name = (1..)
            .map(|n| match n {
                1 => format!("{stem}.wav"),
                n => format!("{stem}-{n}.wav"),
            })
            .find(|name| !names.contains(name))
            .expect("some number is free");
        names.push(name);
    }
    names
}

/// A file-name-safe form of a patch name, e.g. "Fat Sub" → "fat_sub".
fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn run(args: &Args) -> Result<(), String> {
    let path = clip_path(&args.clip);
    let (dry, sample_rate) = read_wav(&path)?;
    let patches = find_patches(&args.patches, &PatchLibrary::open(patches::default_dir()))?;
    let clip = path
        .file_stem()
        .map_or("clip".into(), |s| s.to_string_lossy());
    if let Some(dir) = &args.out_dir {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        write_wav(&dir.join(format!("{clip}.wav")), &dry, sample_rate)?;
    }
    let files = output_names(&clip, &patches);
    println!(
        "{:<16} {:>9} {:>6} {:>11}",
        "patch", "loudness", "peak", "similarity"
    );
    for (patch, file) in patches.iter().zip(&files) {
        let out = render(patch, &dry, sample_rate as f32);
        let measures = measure(&dry, &out, sample_rate as f32);
        println!(
            "{:<16} {:>+6.1} dB {:>6.2} {:>11.2}",
            patch.name, measures.loudness_db, measures.peak, measures.similarity_to_dry
        );
        if let Some(dir) = &args.out_dir {
            write_wav(&dir.join(file), &out, sample_rate)?;
        }
    }
    Ok(())
}

/// The patches `specs` name, in order: a patch name in any case, a `.toml`
/// file, or `all` (every built-in and saved patch). No specs means all.
fn find_patches(specs: &[String], library: &PatchLibrary) -> Result<Vec<Patch>, String> {
    let every = || library.built_in().iter().chain(library.user()).cloned();
    if specs.is_empty() {
        return Ok(every().collect());
    }
    let mut found = vec![];
    for spec in specs {
        if spec == "all" {
            found.extend(every());
        } else if spec.ends_with(".toml") {
            let text = std::fs::read_to_string(spec).map_err(|e| format!("{spec}: {e}"))?;
            found.push(Patch::from_toml(&text, spec).map_err(|e| format!("{spec}: {e}"))?);
        } else {
            let patch = every()
                .find(|p| p.name.eq_ignore_ascii_case(spec))
                .ok_or_else(|| {
                    let names: Vec<String> = every().map(|p| p.name).collect();
                    format!("no patch named {spec:?}. Patches: {}", names.join(", "))
                })?;
            found.push(patch);
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Command, String> {
        parse_args(args.iter().map(|a| a.to_string()))
    }

    #[test]
    fn args_take_a_clip_any_number_of_patches_and_an_optional_out_dir() {
        assert_eq!(
            parse(&["--clip", "slides"]),
            Ok(Command::Render(Args {
                clip: "slides".into(),
                patches: vec![],
                out_dir: None,
            }))
        );
        assert_eq!(
            parse(&[
                "--patch",
                "Fat Sub",
                "--clip",
                "slides",
                "--patch",
                "mine.toml",
                "--out-dir",
                "/tmp/x"
            ]),
            Ok(Command::Render(Args {
                clip: "slides".into(),
                patches: vec!["Fat Sub".into(), "mine.toml".into()],
                out_dir: Some("/tmp/x".into()),
            }))
        );
        assert_eq!(parse(&["--help"]), Ok(Command::Help));
    }

    #[test]
    fn a_clip_is_required_and_every_option_needs_its_value() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["--clip"]).is_err());
        assert!(parse(&["--clip", "slides", "--patch"]).is_err());
        assert!(parse(&["--clip", "slides", "--loud"]).is_err());
        assert!(parse(&["--clip", "slides", "extra"]).is_err());
    }

    fn names(patches: &[Patch]) -> Vec<&str> {
        patches.iter().map(|p| p.name.as_str()).collect()
    }

    #[test]
    fn a_patch_is_found_by_name_in_any_case() {
        let library = PatchLibrary::open(None);
        let found = find_patches(&["fat SUB".into()], &library).unwrap();
        assert_eq!(names(&found), ["Fat Sub"]);
    }

    #[test]
    fn all_or_no_patches_means_every_patch() {
        let library = PatchLibrary::open(None);
        let every = names(library.built_in());
        assert!(every.len() > 1);
        assert_eq!(names(&find_patches(&[], &library).unwrap()), every);
        assert_eq!(
            names(&find_patches(&["all".into()], &library).unwrap()),
            every
        );
    }

    #[test]
    fn a_toml_path_loads_that_file() {
        let path =
            std::env::temp_dir().join(format!("synthbass-render-{}.toml", std::process::id()));
        std::fs::write(&path, "version = 2\nname = \"From File\"\n").unwrap();
        let found = find_patches(&[path.display().to_string()], &PatchLibrary::open(None));
        std::fs::remove_file(&path).unwrap();
        assert_eq!(names(&found.unwrap()), ["From File"]);
    }

    #[test]
    fn an_unknown_name_is_an_error_that_lists_the_patches() {
        let error = find_patches(&["Kazoo".into()], &PatchLibrary::open(None)).unwrap_err();
        assert!(
            error.contains("Kazoo") && error.contains("Pluck"),
            "{error}"
        );
    }

    #[test]
    fn a_clip_name_is_a_file_in_the_clips_folder_and_a_path_is_itself() {
        assert!(
            clip_path("slides").is_file(),
            "{}",
            clip_path("slides").display()
        );
        assert_eq!(clip_path("take.wav"), PathBuf::from("take.wav"));
        assert_eq!(clip_path("/tmp/x/take"), PathBuf::from("/tmp/x/take"));
    }

    #[test]
    fn every_render_gets_its_own_file_apart_from_the_dry_signal() {
        let patches: Vec<Patch> = ["Fat Sub", "fat-sub", "Dry", "Pluck"]
            .into_iter()
            .map(Patch::new)
            .collect();
        assert_eq!(
            output_names("slides", &patches),
            [
                "slides--fat_sub.wav",
                "slides--fat_sub-2.wav",
                "slides--dry.wav",
                "slides--pluck.wav"
            ]
        );
    }
}
