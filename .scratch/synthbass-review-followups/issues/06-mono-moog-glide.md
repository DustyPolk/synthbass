# 06: Mono-Moog glide

**What to build:** glide works like a mono Moog's portamento. The engine remembers the pitch and end time of the most recently ended voice. A voice starting on a new note, or reassigned to one, glides from the newest still-sounding voice under the existing rule (it started at least 50 ms earlier), or else from the most recently ended voice if it ended within a short glide memory (an engine constant around 250 ms, tuned later by playing; not a knob). Glide time 0 still means no glide; glide still never applies within a note (slides, bends, vibrato); notes picked together never glide into each other.

**Blocked by:** None — 05 (Every picked note gets a voice) was closed as wontfix

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [x] Test: a note starting shortly after the previous one ended glides from it (fails on the current code)
- [x] Test: a note starting after a gap longer than the glide memory starts on pitch
- [x] Test: two notes picked together never glide, with glide on
- [x] Test: a reassigned voice glides from the right source, never from its own unrelated old pitch
- [x] Slide, vibrato and glide-zero tests still pass
- [x] The glossary's **Glide** entry and ticket 08's notes in `.scratch/synthbass/issues/08-slides-bends-and-glide.md` (a `## Comments` addendum) describe the new rule
- [x] All five in-crate `tracked.rs` unit tests that feed hand-made notes (listed in follow-up 03's `## Comments`, including `a_new_note_takes_a_silent_voice_not_one_still_fading`) are replaced by public-interface tests and deleted, so no engine test reaches into internals (ignored diagnostics in `tracker/probe.rs` excepted)
- [x] Any new timer (glide memory) advances in `TrackedLayer::listen`, not `update`, so output stays identical at any block size (follow-up 04's test proves it)
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions

## Comments

- Glide memory: `GLIDE_MEMORY_SECONDS` = 250 ms, in `tracked.rs`. A note "ends" when its voice loses it (the note ends, or the string is muted), not when the fade finishes. The memory's timer advances in `TrackedLayer::listen`, next to the glide-source timer, and the block-size identity test passes.
- Source order: first the newest voice to take a note, if it still sounds and started at least 50 ms earlier. Otherwise the last ended note, within the memory. When the newest voice is younger than 50 ms (a note picked together with another), the note falls back to the memory. So a chord picked shortly after a previous note glides out from that note, the same way a chord already did when the previous note still sounded. A chord picked from silence, or after a gap longer than the memory, doesn't glide.
- Tests: `slides_and_glide.rs` has `a_note_shortly_after_the_last_one_ended_glides_from_it` (100 ms gap, glide 0.1 s; it reached D2 after 0.001 s on the old code) and `a_note_after_a_long_gap_starts_on_pitch` (600 ms gap; it fails with a 2.5 s memory). Notes picked together: the existing `notes_picked_together_do_not_glide_into_each_other`.
- The five `tracked.rs` unit tests are deleted, and the `#[cfg(test)]` module in `tracked.rs` is gone. Their public replacements are in `polyphony.rs`. Each was checked against a deliberate break:
  - quietest of five: the existing `five_simultaneous_tones_give_four_voices_without_the_quietest`. It is still weak (05's note): the tracker never reports the quiet D#4.
  - fifth replaces the quietest: `a_loud_fifth_note_takes_the_quietest_voice`. It fails when stealing is off.
  - steal margin: `a_fifth_note_about_as_loud_as_the_quietest_does_not_trade_voices_with_it`. A fifth D#4 at 0.06 against an E1 that started at 0.12. It fails with `STEAL_MARGIN` 1.0, but only for fifth amplitudes from 0.055 to 0.065. Below that range the tracker doesn't trade, and above it the note is loud enough to steal even with the margin.
  - silent voice, not a fading one: `a_new_note_leaves_the_last_ones_voice_to_fade_out`. It loops over gaps of 0 to 50 ms, because only some gaps (20 and 40 ms) catch a break that prefers fading voices.
  - stolen voice glide: `a_stolen_voice_glides_from_the_newest_voice_not_its_own_old_note`. It fails when stolen voices don't glide.
- Seen while probing: a chord voice that starts while an older voice sounds glides in from it. E1 then a C#4/F4/G4 chord: the F4 voice swoops up from E1. That is the existing 50 ms rule, unchanged. Watch for it when playing.
- Also seen: two held notes of near-equal tracker level (F4 and G4, 0.2 each, under three louder notes) trade the last voice back and forth even with the 1.25 margin. The tracker's levels for them swing by more than 25%. Not addressed.
