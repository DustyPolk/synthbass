<img src="assets/icon.svg" width="64" height="64" alt="">

# Synthbass

Created in [T3 Code](https://t3.codes).

## Development

- `scripts/check.sh` runs every check: `cargo fmt --check`, clippy with
  warnings as errors, and the full test suite. There is no CI. After cloning,
  run `git config core.hooksPath .githooks` so commits run fmt and clippy and
  pushes run the whole check.
- `cargo run --release --bin render -- --clip chromatic-run --patch "Fat Sub" --out-dir /tmp/audition`
  plays patches over a recorded clip offline, writes WAVs and prints each
  patch's loudness against the dry signal, its peak, and its similarity to the
  dry signal. `--help` lists the options.
- `scripts/live-render.sh "<patch>" out.wav [clip]` does the same through the
  real running app, to check the live path.
- `cargo run --release --bin record -- clips/<name>.wav` records a new test
  clip; see `clips/README.md`.
