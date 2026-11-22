# 08: Proper types for waveshape, octave and noise gate

**What to build:** domain values stop hiding in bare numbers. Waveshape (saw/square) and octave (−1/0/+1) become small enums; parameter values may stay `f32` for lock-free sharing, but conversion to and from the enums happens in exactly one place. The noise gate threshold becomes a type that validates itself (clamped to its range, rejecting non-finite values), used by both the engine and settings loading. Oscillator 2's octave, detune and resulting pitch ratio are grouped into one value that changes as a unit.

**Blocked by:** 02 (Engine owns the patch), 06 (Mono-Moog glide)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Waveshape and octave are enums; every read of their parameter value goes through one conversion
- [x] The noise gate is a validated type; clamping and non-finite rejection happen only inside it; engine and settings both use it
- [x] Oscillator 2's octave, detune and pitch ratio are one grouped value
- [x] Patch files and settings files from v1 still load identically
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions
