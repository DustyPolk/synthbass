# 04: Recording tests check the tuner and buffer sizes

**What to build:** the recorded-clip tests verify two things the v1 spec asked for but never checked on real playing: the open-string and chromatic clips assert the tuner reading for each event within the clip's cents tolerance, and every recorded-clip run repeats at odd and varying buffer sizes (for example 61, 128 and 512 in rotation) as well as 128, with the same pass criteria. If a buffer size changes what the tracker hears, fix the engine at the root rather than loosening the test.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] Open-string and chromatic clip tests assert the tuner's note and cents per event (blank where no clear single note is expected)
- [ ] All recorded-clip suites run at 128 and at an odd/varying rotation, with identical pass criteria
- [ ] Any buffer-size dependence found is fixed in the engine, with the cause written in the ticket's `## Comments`
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions
