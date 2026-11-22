//! Gain staging: at equal level settings the dry signal, the direct layer and
//! the tracked layer come out about equally loud, so every level knob blends
//! rather than one layer burying the others. Measured on a recorded bass at
//! the level it really arrives at from the interface.

mod common;

use common::clip::{clip_path, load_clip};
use common::{engine, render};
use synthbass_engine::{NoiseGate, Param, Patch};

/// How far (dB) a layer at full level may sit from the dry signal at full
/// level. Filters and waveshapes still change loudness, as on any synth.
const BALANCE_DB: f32 = 6.0;

fn rms(signal: &[f32]) -> f32 {
    (signal.iter().map(|s| s * s).sum::<f32>() / signal.len() as f32).sqrt()
}

fn db(ratio: f32) -> f32 {
    20.0 * ratio.log10()
}

fn render_patch(patch: &Patch, dry: &[f32]) -> Vec<f32> {
    let mut engine = engine();
    engine.apply_patch(patch);
    engine.set_noise_gate(NoiseGate::default());
    render(&mut engine, dry, &[128]).left
}

/// `patch` with only `layer` up, at full level.
fn alone(patch: &Patch, layer: Param) -> Patch {
    let mut patch = patch.clone();
    for level in [Param::DryLevel, Param::DirectLevel, Param::TrackedLevel] {
        patch.set(level, if level == layer { 1.0 } else { 0.0 });
    }
    patch.set(Param::OutputLevel, 1.0);
    patch
}

/// The built-in patches, read from the app's patch folder.
fn built_in_patches() -> Vec<Patch> {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../app/patches");
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    assert!(!paths.is_empty(), "no built-in patches in {dir}");
    paths
        .iter()
        .map(|path| {
            let text = std::fs::read_to_string(path).unwrap();
            Patch::from_toml(&text, "").unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        })
        .collect()
}

/// How closely `out` follows the dry signal's waveform, from 0 (unrelated) to
/// 1 (the dry signal scaled). Filters shift phase, so up to ~5 ms of lag is
/// allowed.
fn similarity_to_dry(out: &[f32], dry: &[f32]) -> f32 {
    (0..256)
        .map(|lag| {
            let (out, dry) = (&out[lag..], &dry[..dry.len() - lag]);
            let dot: f32 = out.iter().zip(dry).map(|(a, b)| a * b).sum();
            dot / (rms(out) * rms(dry) * out.len() as f32).max(1e-12)
        })
        .fold(f32::MIN, f32::max)
}

#[test]
fn each_layer_at_full_level_sits_near_the_dry_signal_at_full_level() {
    // The default patch, so the filters are where a new patch starts. A
    // patch can still make a layer quieter on purpose, e.g. a dark filter.
    let patch = Patch::new("defaults");
    let mut out_of_balance = vec![];
    for clip in ["open-strings", "chromatic-run", "palm-mutes"] {
        let dry = load_clip(&clip_path(clip)).dry;
        let reference = rms(&render_patch(&alone(&patch, Param::DryLevel), &dry));
        for layer in [Param::DirectLevel, Param::TrackedLevel] {
            let level = db(rms(&render_patch(&alone(&patch, layer), &dry)) / reference);
            if level.abs() > BALANCE_DB {
                out_of_balance.push(format!("{clip} {layer:?}: {level:+.1} dB"));
            }
        }
    }
    assert!(out_of_balance.is_empty(), "{out_of_balance:#?}");
}

#[test]
fn drive_changes_the_direct_layers_tone_more_than_its_loudness() {
    let dry = load_clip(&clip_path("open-strings")).dry;
    let at_drive = |drive: f32| {
        let mut patch = alone(&Patch::new("drive"), Param::DirectLevel);
        patch.set(Param::DirectDrive, drive);
        rms(&render_patch(&patch, &dry))
    };
    let (clean, full) = (at_drive(0.0), at_drive(1.0));
    let change = db(full / clean);
    assert!(
        change.abs() < BALANCE_DB,
        "full drive is {change:+.1} dB against none"
    );
}

#[test]
fn no_built_in_patch_sounds_like_the_dry_signal() {
    let mut sounds_dry = vec![];
    for clip in ["open-strings", "chromatic-run"] {
        let dry = load_clip(&clip_path(clip)).dry;
        for patch in built_in_patches() {
            let similarity = similarity_to_dry(&render_patch(&patch, &dry), &dry);
            if similarity > 0.8 {
                sounds_dry.push(format!("{clip}/{}: {similarity:.2}", patch.name));
            }
        }
    }
    assert!(sounds_dry.is_empty(), "{sounds_dry:#?}");
}

#[test]
fn turning_the_tracked_layer_up_audibly_changes_a_built_in_patch() {
    let dry = load_clip(&clip_path("open-strings")).dry;
    let patch = built_in_patches()
        .into_iter()
        .find(|p| p.name == "Detuned Saw")
        .expect("Detuned Saw is built in");
    let mut without = patch.clone();
    without.set(Param::TrackedLevel, 0.0);
    let (with, without) = (render_patch(&patch, &dry), render_patch(&without, &dry));
    let difference: Vec<f32> = with.iter().zip(&without).map(|(a, b)| a - b).collect();
    let share = db(rms(&difference) / rms(&with));
    assert!(
        share > -6.0,
        "the tracked layer is {share:.1} dB of the patch"
    );
}
