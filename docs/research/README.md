# Synthbass research notes (2026-11-21)

The research done before v1 was written. Where a note and the code disagree, the code
wins: some recommendations changed during implementation (for example, the UI shares the
patch with the audio thread through atomics, not a message queue, and drive is now
level-matched). Read these instead of re-researching. Versions were checked on crates.io,
APIs were checked against crate sources, and the DSP claims marked ✓ / "verified" come
from scratch sims on this machine.

- [rust-crates.md](rust-crates.md): versions and verified APIs for jack 0.14 (PipeWire),
  eframe/egui 0.37 (**`App::ui`, not `update`**), rtrb + triple_buffer for UI↔audio, hound,
  serde/toml 1.x/dirs 7. Also the **real Scarlett JACK port names** and how to get 64–128
  frame buffers (`PIPEWIRE_LATENCY`).
- [dsp-ladder-filter.md](dsp-ladder-filter.md): ZDF/TPT 4-pole ladder with one tanh,
  resonance compensation, stability at k = 4, log-domain cutoff smoothing, denormals.
- [dsp-polyphonic-tracking.md](dsp-polyphonic-tracking.md): concrete first design
  (decimate to 6 kHz → order-2 complex resonator bank → IF clusters → harmonic sieve with
  cancellation → voice matching), onset strength, string following, tuner, YIN mono
  fallback, latency numbers.
- [dsp-misc.md](dsp-misc.md): envelope follower, tanh drive with ADAA, PolyBLEP
  saw/square/sub, log-domain glide, parameter smoothing, a guaranteed-ceiling limiter, and
  FTZ/DAZ (measured 63× denormal penalty).
