# 05: Every picked note gets a voice

**What to build:** a note detected with a pick (an onset or repick) always gets a voice: if all four are busy, it replaces the quietest sounding voice, however softly it was picked. Notes that appear without a pick (overtones, re-emerging notes) keep the existing loudness margin, so attack ghosts don't come back.

**Blocked by:** 03 (Engine tests through the public interface only)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] Test: with four voices sounding, a soft fifth picked note gets a voice and the quietest older voice is the one dropped (fails on the current code)
- [ ] Test: a note appearing without a pick cannot take a voice from a louder picked note
- [ ] The existing quietest-of-five test can no longer pass by dropping the new note
- [ ] The in-crate `tracked.rs` unit tests that feed `TrackedLayer` hand-made notes and cover stealing or voice reuse (see follow-up 03's `## Comments`), including `a_new_note_takes_a_silent_voice_not_one_still_fading`, are replaced by public-interface tests and deleted
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions
