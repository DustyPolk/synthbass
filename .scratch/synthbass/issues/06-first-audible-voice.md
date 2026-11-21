# 06: First audible voice

**What to build:** the tracked layer makes sound. A detected note gets one voice with oscillator 1 (saw or square) at the detected pitch, its loudness set by string following, mixed in through a tracked layer level control. The voice ends when the string is muted or decays below the noise gate. The window gains a tracked section (oscillator 1 waveshape, tracked layer level) and an active-voice indicator. One voice at a time in this slice.

**Blocked by:** 05 (Note detection and tuner)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [ ] Readout lists the active voice with pitch and loudness
- [ ] String following: voice loudness tracks the note's level, including decay and sudden mutes
- [ ] Offline tests: a synthetic note produces one voice at the right pitch whose loudness follows the input envelope; muting the input ends the voice; tracked layer level 0 removes its contribution; the voice fades in without a click
- [ ] Direct layer and dry signal still mix correctly alongside the tracked layer
- [ ] Verified by playing
