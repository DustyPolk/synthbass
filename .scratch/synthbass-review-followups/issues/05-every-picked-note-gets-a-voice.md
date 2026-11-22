# 05: Every picked note gets a voice

**What to build:** a note detected with a pick (an onset or repick) always gets a voice: if all four are busy, it replaces the quietest sounding voice, however softly it was picked. Notes that appear without a pick (overtones, re-emerging notes) keep the existing loudness margin, so attack ghosts don't come back.

**Blocked by:** 03 (Engine tests through the public interface only), 12 (Pick evidence for a soft note under a chord)

**Status:** wontfix

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] Test: with four voices sounding, a soft fifth picked note gets a voice and the quietest older voice is the one dropped (fails on the current code)
- [ ] Test: a note appearing without a pick cannot take a voice from a louder picked note
- [ ] The existing quietest-of-five test can no longer pass by dropping the new note
- [ ] The in-crate `tracked.rs` unit tests that feed `TrackedLayer` hand-made notes and cover stealing or voice reuse (see follow-up 03's `## Comments`), including `a_new_note_takes_a_silent_voice_not_one_still_fading`, are replaced by public-interface tests and deleted
- [ ] All existing tests pass, including the recorded-clip suites, with no new exemptions

## Comments

- Stopped at the context budget without code changes; every criterion is still open. The blocker is that the tracker can't tell that a soft fifth note was picked. Split into 12 (pick evidence for a soft note under a chord). 05 is now blocked by it.
- What blocks the first test. The only pick signal is the dry signal's peak jumping 1.2× above its highest level over the last ~130 ms (ticket 12's `PICK_JUMP`/`PICK_LOOKBACK_HOPS`). Under four ringing notes that peak is set by the chord's beating. A fifth note softer than the quietest voice, or even as loud, never raises it 1.2×. Traced: E1 0.3, C#4 0.25, F4 0.2 and G4 0.12 (`bass_tone`), plus D#4 0.12 at 1.0 s. The pick level went from 0.313 to 0.346 against a lookback peak of 0.376, so no pick fired before the note was born. Equal amplitudes (all four at 0.15, the new one 0.12 to 0.15) didn't fire either. A pick did register a few hops after the note's birth in some runs, but after the note was first heard.
- Also tried, then reverted: (1) `DetectedNote.picked` = first heard within the pick window and clarity ≥ `PICK_CLARITY`, also set when a pick fires within the window after the note was first heard. The soft fifth was still never flagged. (2) The note's own band restarting (`Bank::restarted`, a 1.5× rise in 8 hops) near birth. It didn't line up: the soft D#4 is first heard 30 to 70 ms after it starts, after its band's rise.
- Chord notes picked together are often unflagged too. In the chord above, C#4 and G4 were born after the 40 ms window or with clarity 0 (G4 at 52 hops after the pick, clarity 0.00). A `picked` flag built on birth timing would leave real chord notes on the margin rule. That's harmless (they get free voices), but don't build a test on it.
- Layer design worked out for when the tracker has the signal (`tracked.rs`): voices are claimed only at a note's onset. The layer remembers last update's notes (id plus pick count, from the ungated list, so a mute gap doesn't make every note new). A note's pick count is `repicks` plus 1 if it began with a pick. A voiceless note whose pick count rose replaces the quietest voice not assigned in the same update. Notes come loudest first, so five notes picked together still drop the quietest. A note that appears without a pick may take the quietest voice only by `STEAL_MARGIN`, and only when it appears. Otherwise a displaced louder note would take its voice back from the soft picked note on the next update. A voiceless held note never steals later, so the margin's other job (no trading between held notes) goes away. No timers, so it fits follow-up 04's 32-sample grid.
- The quietest-of-five test (`polyphony.rs`) passes today because D#4 0.06 is never in `Readout.notes` (ticket 07's note). To make it meaningful, assert the quiet note is in `Readout.notes` but has no voice. That needs a fifth pitch the tracker hears, which the probe above didn't establish.
- The `tracked.rs` unit tests are unchanged: all five are still there for 05/06 (none migrated or deleted).

**Closed as wontfix (bassist's decision).** A 4-string bass rings at most four strings at once, so a "fifth note" in practice is a re-fretted string whose previous note is still dying, not a fifth independent note; the tracker can't detect a soft fifth under four ringing notes anyway. The voice-allocation design above stays on record for a 5+-string bass. The five in-crate `tracked.rs` unit tests listed for migration move to follow-up 06.
