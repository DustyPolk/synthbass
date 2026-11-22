# 09: Glossary names and renames

**What to build:** the code speaks the glossary. The engine's readout field and meter for the dry signal's level are named for the dry signal, and the window labels it that way. The app's shared parameter store is named for the patch, not "sound". `GLOSSARY.md` gains a note that JACK input and output ports are audio routing, not the dry signal, so the port pickers keep "input port" and "output port". The tracker's per-candidate bookkeeping type gets a name that doesn't clash with the tracked layer; the sieve's bare adjective constants get names that say what they measure; oscillator 1's fields are named as oscillator 1; the audio callback's nested engine field access is flattened.

**Blocked by:** 07 (Oscillator 2 audible by default), 08 (Proper types for waveshape, octave and noise gate)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] No identifier or window label uses "input" for the dry signal (JACK port names excepted) or "sound" for a patch
- [x] `GLOSSARY.md` has the port note
- [x] The tracker's bookkeeping type, the sieve constants and oscillator 1's fields are renamed; no nested engine-in-engine access remains
- [x] Pure renames: no behaviour change
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions
