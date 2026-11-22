# 09: Moog voice

**What to build:** each voice gets the full Moog-style sound: oscillator 2 (saw or square, octave −1/0/+1, detune), a sub-oscillator one octave down with its level, and its own ladder filter (reused from the direct layer) with cutoff and resonance. A filter envelope fires when the voice's note is detected, with amount and decay controls, and opens wider the harder the note is picked (pick sensitivity). The window's tracked section shows all of these controls.

**Blocked by:** 04 (Direct layer), 06 (First audible voice)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Every voice has its own ladder filter and filter envelope; voices starting at different times sweep independently
- [x] Offline tests: a harder synthetic onset opens the filter envelope further than a soft one (measured as more high-frequency energy shortly after onset); filter envelope amount 0 leaves the filter static; sub-oscillator output is an octave below the voice pitch; octave switch shifts oscillator 2 by exactly an octave; max resonance stays under the limiter ceiling
- [x] String following still sets voice loudness
- [ ] Verified by playing: a classic Moog pluck is achievable from the controls
