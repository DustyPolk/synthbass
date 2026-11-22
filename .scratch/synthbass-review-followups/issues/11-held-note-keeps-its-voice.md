# 11: Held note keeps its voice through nearby attacks

**What to build:** a held note keeps its voice (same voice id, no restart, no filter envelope retrigger) when other short attacks happen nearby. Follow-up 03 found that the tracker still restarts the held note's voice in 33 of 216 synthetic attack variants (short sharp attacks beside a held E1), independent of the slot-matching order bug fixed after the v1 review. Its guess is masking during the attack burst; not investigated. Find the root cause and fix it.

**Blocked by:** 06 (Mono-Moog glide) — voice-handling code churns until then

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md`, follow-up 03's `## Comments` and `notes/from-followup-03.md` in the shared notes folder (the attack-variant search is described there). Delegate bulky signal analysis to a subagent; keep test output terse. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [ ] A public-interface test sweeps the attack variants follow-up 03 used (or a representative subset) and asserts the held note's voice id never changes; it fails on the current code
- [ ] The root cause is written in this ticket's `## Comments`, and fixed in the tracker or voice allocation
- [x] All existing tests pass, including the recorded-clip suites and the block-size identity test, with no new exemptions

## Comments

- The sweep: `tracked_layer.rs` `a_held_note_keeps_its_voice_through_any_nearby_attack`. Held E1 and A1 (`bass_tone` 0.3), attacks 0.30 to 0.55 st sharp (`bass_tone` 0.6, linear fade-out), 30 to 80 ms long, at 0.8, 0.9 and 1.0 s: 216 variants. Follow-up 03's exact grid was not recorded; this one restarts more of them (134 of 216 on the old code, against 03's 33). It now restarts 63 and is `#[ignore]`d, pointing at follow-up 13. The first two criteria are therefore only partly met: one root cause is found and fixed, the second is found and split out.
- Root cause 1 (fixed, in the tracker's sieve): `History::periodicity` reports `judged: false` when the older half of its span carries less than a quarter of the newer half's energy. An attack twice the held note's level does exactly that for every candidate, for 20 to 40 ms. The sieve dropped every unjudged candidate, the held note's own steady one too (4 to 6 settled partials, clarity 0.4 to 0.65), so the tracker heard nothing for longer than the 15 ms release and the note expired. It came back with a new id and took a new voice. A second spot had the same premise: `outranked_from_below` let any unjudged lower probe outrank a candidate, and during the attack every probe is unjudged (the A1 at 0.9 s variants).
  - Fix: a candidate within `CONTINUES` of a sounding note, steady, and still at least `UNJUDGED_CLARITY` (0.3) periodic carries on unjudged, and unjudged lower probes don't outrank it.
  - The clarity floor is what keeps `palm-mutes` green: without it, a palm-muted E1 drifting off pitch (clarity 0.1 to 0.2, steady only on some hops) carried on through the next G1 attack as an extra voice.
  - Test: `a_held_note_keeps_its_voice_through_a_louder_attack_beside_it`, 66 variants. All 66 restart on the old code. With only the unjudged-candidate half of the fix, 22 of the E1 half still restart. With only the outranked half, 36 of 66 restart.
- Root cause 2 (not fixed; follow-up 13): when the attack lies within about half a semitone of the held note, the bank can't separate their partials. The held note's fundamental partial drops out of its support: the candidate fails the ghost-below check (no h1, and no h2 or h3). Its octave above (the held note's h2) then wins as a candidate of its own, or the held note jumps a semitone and counts a repick. Either way the held note goes unheard for about 20 ms, just past the 15 ms release. Tried and reverted:
  - accepting the octave below when it is a sounding note (gained 2 variants, broke `double-stop-thirds` 28.75 s coverage);
  - extending the release by the pick window while picked (gained 5, broke `palm-mutes` with extra voices at five onsets).
- Follow-up 06's two equal-level held notes trading a voice: not measured. This fix touches only unjudged candidates, so it is unlikely to change that.
