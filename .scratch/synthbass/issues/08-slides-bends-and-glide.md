# 08: Slides, bends and glide

**What to build:** voices follow exact pitch, so slides, bends, and vibrato come through on the synth. A voice keeps its identity while a slide crosses several semitone bands instead of ending and restarting. The tracked section gains a glide control that makes voices slide into new pitch targets like Moog portamento.

**Blocked by:** 07 (Four-voice polyphony)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. This ticket iterates on real recordings, so delegate bulky clip analysis (per-frame dumps, spectra) to a subagent and keep only its summary. Keep test output terse (failures and per-clip pass/fail only). If tuning is not converging by roughly 100K tokens, stop, record findings and remaining gaps in a `## Comments` section, and split the rest into a new ticket.

- [ ] Clip tests: each slide clip yields a single voice whose pitch moves continuously from start to end note (no voice restart mid-slide)
- [ ] Synthetic tests: a continuous pitch sweep across several semitones keeps one voice; a vibrato tone is tracked within tolerance; glide 0 follows pitch immediately, higher glide reaches the target over a proportionally longer time
- [ ] Double-stop and palm-mute clip tests from 07 still pass
- [ ] Verified by playing slides and bends
