# 13: Held note keeps its voice through an attack on its own partials

**What to build:** a held note keeps its voice when a short attack lands within about half a semitone of it, close enough that the tracker's bands can't separate the two notes' partials. Follow-up 11 fixed the louder-attack case (unjudged periodicity) and found this second mechanism. The held note's fundamental partial drops out of its support for about 20 ms: the candidate fails the ghost-below check, and its octave above takes over or the held note jumps a semitone and counts a repick. The 15 ms release then expires it. See follow-up 11's `## Comments` for the traces and the two approaches already tried.

**Blocked by:** none

**Status:** needs-triage

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, follow-up 11's `## Comments`, and `notes/from-followup-11.md` and `notes/from-ticket-12.md` in the shared notes folder (ghost suppression has thin margins). Delegate bulky signal analysis to a subagent; keep recorded-clip output to per-clip pass/fail. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] `tracked_layer.rs` `a_held_note_keeps_its_voice_through_any_nearby_attack` passes and is no longer ignored (63 of 216 restart today)
- [ ] The root cause fix is written in this ticket's `## Comments`
- [ ] All existing tests pass, including the recorded-clip suites and the block-size identity test, with no new exemptions

**Related, found by the follow-up review fixes:** when E2 is muted and E1 picked within about 5 ms, E2 stays reported alongside E1 for about 30 ms (an octave-up ghost). It does not come from the sieve's `carried_through_attack` rule; likely the same octave/partial-sharing area as this ticket.
