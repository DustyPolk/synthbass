# 11: Held note keeps its voice through nearby attacks

**What to build:** a held note keeps its voice (same voice id, no restart, no filter envelope retrigger) when other short attacks happen nearby. Follow-up 03 found that the tracker still restarts the held note's voice in 33 of 216 synthetic attack variants (short sharp attacks beside a held E1), independent of the slot-matching order bug fixed after the v1 review. Its guess is masking during the attack burst; not investigated. Find the root cause and fix it.

**Blocked by:** 06 (Mono-Moog glide) — voice-handling code churns until then

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md`, follow-up 03's `## Comments` and `notes/from-followup-03.md` in the shared notes folder (the attack-variant search is described there). Delegate bulky signal analysis to a subagent; keep test output terse. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] A public-interface test sweeps the attack variants follow-up 03 used (or a representative subset) and asserts the held note's voice id never changes; it fails on the current code
- [ ] The root cause is written in this ticket's `## Comments`, and fixed in the tracker or voice allocation
- [ ] All existing tests pass, including the recorded-clip suites and the block-size identity test, with no new exemptions
