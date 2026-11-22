# 03: Engine tests through the public interface only

**What to build:** every engine test drives the engine only through its public interface (audio in, patch or parameters, output audio and readout out), as the v1 testing rules require. The tests that build voices directly, call the tracker's private matching step, or drive the ladder filter alone are rewritten against the public interface with equivalent coverage, and those internals stop being public. The unused public frequency method on detected notes is deleted.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [ ] Ladder filter behaviour (24 dB/octave slope, resonance peak, stability at max resonance) is tested through the direct layer's output at known settings
- [ ] Two voices sweeping their filters independently is tested via two notes started at different times in a synthetic signal
- [ ] A reported note keeping its voice id when a pending candidate sits nearby is tested via a synthetic attack plus a held note, asserting the voice id in the readout never changes
- [ ] The ladder filter, voice, sound and tracker matching are no longer public; no test reaches into engine internals
- [ ] The detected note's unused frequency method is gone
- [ ] Each rewritten test fails when the behaviour it covers is broken on purpose (check, then revert)
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions
