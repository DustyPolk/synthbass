# 07: Four-voice polyphony

**What to build:** up to four notes ringing at once each get their own voice. Double stops at a third or a tenth play as two voices at the right pitches. A fifth simultaneous note replaces the quietest voice. This is the experiment named in ADR 0002, tuned and tested against the bassist's real recordings; it ends with a clear verdict on whether polyphonic tracking is clean enough.

**Blocked by:** 03 (Record the test clips), 06 (First audible voice)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. This ticket iterates on real recordings, so delegate bulky clip analysis (per-frame dumps, spectra) to a subagent and keep only its summary. Keep test output terse (failures and per-clip pass/fail only). If tuning is not converging by roughly 100K tokens, stop, record findings and remaining gaps in a `## Comments` section, and split the rest into a new ticket.

- [ ] Never more than four voices
- [ ] Voice stealing replaces the quietest voice
- [ ] Clip tests: open-string and chromatic clips produce one voice per note at the expected pitch (timing and cents tolerances from the descriptions); double-stop clips (thirds, tenths) produce two voices at the expected pitches; palm-mute clips end voices promptly
- [ ] Synthetic tests: five simultaneous tones yield four voices with the quietest dropped
- [ ] Octave and fifth fusion is allowed and noted, not tested as a failure
- [ ] A verdict is written in the ticket's : clean enough, or fall back to monophonic tracking per ADR 0002 (and if falling back, the ADR is updated)
- [ ] Verified by playing double stops
