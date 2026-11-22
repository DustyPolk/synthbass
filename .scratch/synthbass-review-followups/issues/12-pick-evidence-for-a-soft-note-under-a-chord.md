# 12: Pick evidence for a soft note under a chord

**What to build:** the tracker reports whether each note began with a pick, and it catches a soft note picked while other strings ring. Today the only pick evidence is the dry signal's peak jumping 1.2× over the last ~130 ms. Under four ringing notes the chord's beating sets that peak, so a fifth note as loud as the quietest of them never registers. Overtones, attack ghosts (clarity under `PICK_CLARITY`) and notes re-emerging from a held chord must still read as unpicked. See follow-up 05's `## Comments` for the traces and the two approaches already tried (birth timing near the dry-peak pick; the note's own band restarting).

**Blocked by:** none (05 is blocked by this)

**Status:** needs-triage

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` (Voice stealing), follow-up 05's `## Comments`, and `notes/from-ticket-12.md` and `notes/from-followup-05.md` in the shared notes folder (ghost suppression has thin margins). Delegate bulky clip analysis to a subagent and keep recorded-clip output to per-clip pass/fail. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] Each detected note says whether it began with a pick (public `DetectedNote` field)
- [ ] Test: with four notes ringing (e.g. E1 0.3, C#4 0.25, F4 0.2, G4 0.12 from `bass_tone`), a fifth note picked no louder than the quietest reads as picked
- [ ] Test: a note that appears without a pick (an overtone, or a note that re-emerges when a louder one ends) reads as unpicked
- [ ] All existing tests pass, including the recorded-clip suites and the block-size identity test, with no new exemptions
