# 04: Direct layer

**What to build:** the bassist hears the direct layer: the dry signal is squared up by a drive control, then passes through a Moog-style ladder filter whose cutoff opens with playing intensity (envelope sensitivity), then a level control. It adds no latency and works on chords. The window gains a direct section with drive, cutoff, resonance, envelope sensitivity, and level. The ladder filter is built as a reusable engine component; the Moog voice (09) reuses it.

**Blocked by:** 01 (Dry signal passthrough)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Direct layer sits in parallel with the (future) tracked layer and mixes with the dry signal before output level and limiter
- [x] Ladder filter has cutoff and resonance, stays stable at maximum resonance, and is usable per-voice later
- [x] Offline tests: an onset produces direct layer output within the same block (zero added latency); level 0 removes the layer's contribution; a two-note chord produces no NaN, denormal stalls, or runaway output; max drive plus max resonance stays under the limiter ceiling
- [ ] Window direct section controls take effect smoothly while playing
- [ ] Verified by playing
