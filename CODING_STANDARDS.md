# Coding standards

Judgement calls for review. Mechanical rules live in `cargo fmt`, `cargo
clippy -D warnings` and the tests (`scripts/check.sh`), not here.

## Gain staging

**Every level assumption cites real input.** A constant, a default or a test
signal that assumes how loud the dry signal is (a reference peak, an envelope
range, a test tone's amplitude, an absolute output threshold) must match the
recorded clips in `clips/`, and cites their measured peaks: 0.12–0.24 (a
passive bass at full interface gain; `cargo run --release --bin render`
prints any clip's numbers). A synthetic test tone at 0.4–0.5 hides level bugs
that real playing exposes.

**Every gain stage is level-matched.** A stage that adds gain (drive, makeup,
a resonance boost) either undoes it at a stated reference level or states why
it doesn't. At equal level knobs the dry signal and both layers should sit
within a few dB of each other; `crates/engine/tests/layer_balance.rs` checks
this on the default patch.

Why: v1 shipped a drive with +18 dB of unmatched pre-gain and voices ~9 dB
under the dry signal. Every test fed hot synthetic tones, so the synth sat
~17 dB under a layer that was the dry signal driven and filtered, and every
patch sounded like the dry signal.
