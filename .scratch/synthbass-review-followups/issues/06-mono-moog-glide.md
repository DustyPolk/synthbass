# 06: Mono-Moog glide

**What to build:** glide works like a mono Moog's portamento. The engine remembers the pitch and end time of the most recently ended voice. A voice starting on a new note, or reassigned to one, glides from the newest still-sounding voice under the existing rule (it started at least 50 ms earlier), or else from the most recently ended voice if it ended within a short glide memory (an engine constant around 250 ms, tuned later by playing; not a knob). Glide time 0 still means no glide; glide still never applies within a note (slides, bends, vibrato); notes picked together never glide into each other.

**Blocked by:** 05 (Every picked note gets a voice)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] Test: a note starting shortly after the previous one ended glides from it (fails on the current code)
- [ ] Test: a note starting after a gap longer than the glide memory starts on pitch
- [ ] Test: two notes picked together never glide, with glide on
- [ ] Test: a reassigned voice glides from the right source, never from its own unrelated old pitch
- [ ] Slide, vibrato and glide-zero tests still pass
- [ ] The glossary's **Glide** entry and ticket 08's notes in `.scratch/synthbass/issues/08-slides-bends-and-glide.md` (a `## Comments` addendum) describe the new rule
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions
