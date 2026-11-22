# 07: Oscillator 2 audible by default

**What to build:** the oscillator mix parameter (added in v1 but never specified) stays and is documented. Its default becomes an even blend of oscillators 1 and 2, so oscillator 2's waveshape, octave and detune are heard without hunting for a hidden knob. Every built-in patch that sets oscillator 2's waveshape, octave or detune gets a mix where oscillator 2 is audible, while keeping its character and loudness balance (within about 3 dB across built-ins, as ticket 11 measured).

**Blocked by:** 02 (Engine owns the patch)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [ ] Oscillator mix defaults to an even blend
- [ ] Test: with default settings, switching oscillator 2's octave changes the output (energy appears at the octave)
- [ ] Test: every built-in patch that sets oscillator 2 has a mix where oscillator 2 is audible
- [ ] Built-in patches stay within about 3 dB of each other on the chromatic-run clip; re-render them to the shared notes audition folder so the bassist can compare
- [ ] **Oscillator mix** is in `GLOSSARY.md`
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions
