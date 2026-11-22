# 12: Polyphony gaps left by ticket 07

**What to build:** close the gaps ticket 07 left in polyphonic tracking, measured on the recorded clips. Each gap is an exempt event in `crates/engine/tests/recorded_polyphony.rs` (the `known` lists); fixing one means deleting its exemption.

**Blocked by:** 07 (Four-voice polyphony), 08 (Slides, bends and glide) — both reshape the tracker, so they run in sequence

**Status:** ready-for-agent

**Decisions already made (orchestrator, for the bassist):**
- Thirds 30.60 s (G1+B1): treat the take as unusable (weakly fretted, open strings ringing louder). Remove that event from `double-stop-thirds.notes` with a comment saying why, rather than tuning the tracker to it.
- open-strings 8.34 s: the clip deliberately lets strings ring into the next. Extend the expectation so E1 is allowed (not required) to keep sounding for its natural decay under A1; if the format can't express "allowed", list E1 alongside A1 until it decays below the gate.
- chromatic-run D2/E2: fretted intonation on this bass really is ~15–20 c flat high on the neck. Raise that clip's cents tolerance to 25.

**Context budget:** finish within one fresh context, well under 150K tokens. Delegate bulky clip analysis to a subagent. `cargo test --release --test recorded_polyphony print_scores -- --ignored --nocapture` prints per-event coverage/extra; `print_voice_histograms` (env `CLIPS=`) shows which semitones voices sit on; `tracker/probe.rs` has spectrum, note-dump and repick probes.

- [x] Repicks no longer misfire on sustained double stops: beating between two strings reads as a level rise, so the tracker reports ~129 repicks on double-stop-thirds and the filter envelope retriggers on held chords (audible). Highest priority.
- [x] Attack ghosts that ticket 07's chord tuning added to single notes are gone: palm-mutes 11.06, 12.26, 12.84 s (extra 0.23–0.31) and chromatic-run 6.67 s (coverage 0.82, extra 0.27); also chromatic 4.21 and 9.38 s and palm-mutes 11.97 s (extra 0.10–0.11)
- [x] Short chord changes don't carry leftover notes: double-stop-thirds 32.20 and 33.00 s (extra 0.26, 0.46)
- [ ] D2+F2 (thirds 15.65 s): F2 settles in well under 0.9 s (coverage 0.51)
- [x] Thirds 30.60 s (G1+B1): check the take first. The spectrum shows strong 41.6 and 55.2 Hz (open E and A ringing) with weak, off-pitch G1/B1; either fix the `.notes` file with the bassist or the tracker
- [x] open-strings 8.34 s: E1 rings on under A1 (the clip lets strings ring into the next). Decide whether the expectation should list E1 while it rings, then drop the exemption
- [x] chromatic-run 16.10 and 18.90 s: fretted D2/E2 read −20/−18 c against a 20 c tolerance (the notes header measured ~15 c flat; open strings read 0..+3 c and no IF bias was found). Decide with the bassist whether the tolerance or the expectation changes
- [ ] Major sevenths (extra data) E1+D#2 at 2.48 s: coverage [0.72 0.86], extra 0.68

## Comments

Built on `ticket/12-polyphony-gaps`.

- **Repicks:** double-stop-thirds 178 → 5 repicks (the re-plucked D2+F#2 at 28.75 and 29.35 s, and 32.22 s where G1 moves to F#1 with a pick); tenths 77 → 0. Two strings beat at their fundamentals' difference (12–17 Hz for low thirds), so the dry peak sagged and recovered every beat and the pick detector fired on each recovery. A pick now has to rise 1.2× above the highest dry peak of the last ~130 ms, and a level rise only counts as a repick during a pick. Test: `held_double_stops_are_not_picked_again` (thirds, tenths, sevenths: no repicks away from an onset).
- **Sevenths take:** each upper note is plucked 0.3–0.5 s after its low note (the dry peak jumps and the upper partials appear only then), so the `.notes` file now splits each chord into the low note alone, then both. The late pluck had read as a repick of the low note.
- **Attack ghosts:** a note first heard within the 40 ms pick window is reported only if it was periodic at its own pitch (clarity ≥ 0.35) or after 40 ms; right after a pick a sparse candidate needs 0.75 of the strongest salience. All attack-ghost exemptions are gone; the stray D#2 at slides 14.29 s is gone too. The 0.35 threshold has thin margins: target ghosts measured −0.02..0.30, the synthetic E1 in `notes_picked_together_do_not_glide_into_each_other` 0.39.
- **Short chord changes (32.20, 33.00 s):** a candidate whose refined pitch strays over a semitone from its partials is dropped (outside the pick window); a sounding note with a settled partial beside a louder note survives a hop of low periodicity, so D#2's 3rd harmonic no longer becomes A#3. Extra 0.26 → 0.07 and 0.46 → 0.14.
- **Decided items:** thirds 30.60 s removed from the `.notes` as unusable; open-strings 8.34 s lists E1 as ringing (new `.notes` syntax `(E1)`: may sound, need not); chromatic-run tolerance 25 c. Their exemptions are gone.
- **Left open: D2+F2 (thirds 15.65 s), exemption kept.** Settling isn't the problem: both are voiced cleanly from 15.8 to about 21.2 s. Then F2's partials (−9 to −10.5 dB under D2, about 0.002–0.0027 total) fall to the default gate (0.002) and the sieve drops F2; it can't be born again under the +3 dB birth headroom. Coverage tops out near (21.2−15.8)/(26.35−15.8) ≈ 0.52. Closing it would mean holding notes under the gate. Once alone, D2's voice also wanders ±40 c (beating with the unheard F2), which is most of the 0.33 extra.
- **Left open: sevenths E1+D#2 (now the 2.79 s event, not under test):** cover [0.70 0.89], extra 0.73. The low voice flips between E1 and E2−21 c about every 0.33 s, each flip a new voice id: E1's fundamental is weak (7 dB under h2) and its 2nd harmonic (82.4 Hz) beats with D#2 (77.8 Hz) at ~4.6 Hz, so at each beat maximum the 82 Hz band, pulled toward D#2, reads as its own E2. Needs the octave-below rule to hold E1 through h2 beating; not attempted within budget.
- `realtime_safety` failed once in a full `cargo test --release` run and passed on every rerun.

