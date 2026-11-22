# 01: Dry signal passthrough

**What to build:** the tracer bullet. Launching the app with `cargo run --release` opens a minimal dark window, registers a JACK client through PipeWire, auto-connects Scarlett input 1 to outputs 1/2, and the bassist hears their dry signal through the app. The window has a dry signal level knob, an output level knob, an input level meter, and a connection status line. This establishes the two-part structure (pure engine library + thin app, ADR 0003), the lock-free path for parameter changes and readout snapshots between the UI and audio threads, the hard limiter at the end of the chain, and the offline test harness that drives the engine over synthetic signals.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] The repo has an engine library with no JACK or GUI dependencies, and an app that depends on it
- [x] Engine is constructed with sample rate and max block size, processes mono blocks into output blocks, and allocates nothing in its processing path
- [x] Engine exposes a readout snapshot including input level
- [x] Signal chain in this slice: dry signal → dry signal level → output level → hard limiter → outputs 1/2
- [x] Parameter changes are smoothed (no clicks)
- [x] App auto-connects Scarlett input 1 and outputs 1/2 when present; window shows connection status
- [x] Audio thread never blocks on the UI thread
- [x] Offline tests: silence in gives silence out; a loud input with output level at max never exceeds the limiter ceiling; varying and odd block sizes produce identical output to a single large block; a mid-stream level change shows no discontinuity above a threshold
- [ ] Verified by playing: bass is audible through the app with the Scarlett
