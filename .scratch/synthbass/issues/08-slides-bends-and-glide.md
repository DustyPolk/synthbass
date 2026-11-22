# 08: Slides, bends and glide

**What to build:** voices follow exact pitch, so slides, bends, and vibrato come through on the synth. A voice keeps its identity while a slide crosses several semitone bands instead of ending and restarting. The tracked section gains a glide control that makes voices slide into new pitch targets like Moog portamento.

**Blocked by:** 07 (Four-voice polyphony)

**Status:** done

**Context budget:** finish within one fresh context, well under 150K tokens. This ticket iterates on real recordings, so delegate bulky clip analysis (per-frame dumps, spectra) to a subagent and keep only its summary. Keep test output terse (failures and per-clip pass/fail only). If tuning is not converging by roughly 100K tokens, stop, record findings and remaining gaps in a `## Comments` section, and split the rest into a new ticket.

- [x] Clip tests: each slide clip yields a single voice whose pitch moves continuously from start to end note (no voice restart mid-slide)
- [x] Synthetic tests: a continuous pitch sweep across several semitones keeps one voice; a vibrato tone is tracked within tolerance; glide 0 follows pitch immediately, higher glide reaches the target over a proportionally longer time
- [x] Double-stop and palm-mute clip tests from 07 still pass
- [ ] Verified by playing slides and bends

## Comments

Built on `ticket/08-slides-bends-and-glide`.

- Tracker: a note sounding alone follows its periodicity estimate once that leads its 60 ms slow IF by more than 0.2 semitones (fully at 0.4). The slow IF lagged a 17 st/s slide by over a semitone, and the note's fresh partials started notes of their own. While a note moves it also owns the partials it passed, and ghosts on its harmonics can't pass as steady. A fret crossing blurs the periodicity, so a moving note keeps following down to clarity 0.75 and also searches around where it was last hop.
- `VoiceReading.id` stays the same while a voice sounds; the slide tests count voices with it.
- Slides clip: all three slides are carried by one voice the whole window (extra 0.00 / 0.00 / 0.04: a brief attack ghost at the 14.20 pick that the old tracker also had). Start −6 / −0 / −2 c, arrival −19 / −3 / −19 c. Largest steps are 0.35 to 0.65 st, from fret to fret. The old tracker fails all three.
- Vibrato ±30 c at 5.5 Hz: worst 22 c on A1, 17 c on E2, allowing 20 ms of delay (the old tracker was 35 c off).
- Glide (`TrackedGlide`, 0 to 1 s, default 0): the time to cover nine tenths of a jump, exponential in semitones. It applies on steals, and when a new voice starts while one that started at least 50 ms earlier still sounds (it glides in from that voice). The tracker reports the old note for ~30 ms after a new pick, so "hand the fading voice to the new note" never fired; gliding from the previous voice does the same job. Notes picked together don't glide.
- Not verified by playing (unchecked).
