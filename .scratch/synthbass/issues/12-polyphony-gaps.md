# 12: Polyphony gaps left by ticket 07

**What to build:** close the gaps ticket 07 left in polyphonic tracking, measured on the recorded clips. Each gap is an exempt event in `crates/engine/tests/recorded_polyphony.rs` (the `known` lists); fixing one means deleting its exemption.

**Blocked by:** 07 (Four-voice polyphony), 08 (Slides, bends and glide) — both reshape the tracker, so they run in sequence

**Status:** ready-for-agent

**Decisions already made (orchestrator, for the bassist):**
- Thirds 30.60 s (G1+B1): treat the take as unusable (weakly fretted, open strings ringing louder). Remove that event from `double-stop-thirds.notes` with a comment saying why, rather than tuning the tracker to it.
- open-strings 8.34 s: the clip deliberately lets strings ring into the next. Extend the expectation so E1 is allowed (not required) to keep sounding for its natural decay under A1; if the format can't express "allowed", list E1 alongside A1 until it decays below the gate.
- chromatic-run D2/E2: fretted intonation on this bass really is ~15–20 c flat high on the neck. Raise that clip's cents tolerance to 25.

**Context budget:** finish within one fresh context, well under 150K tokens. Delegate bulky clip analysis to a subagent. `cargo test --release --test recorded_polyphony print_scores -- --ignored --nocapture` prints per-event coverage/extra; `print_voice_histograms` (env `CLIPS=`) shows which semitones voices sit on; `tracker/probe.rs` has spectrum, note-dump and repick probes.

- [ ] Repicks no longer misfire on sustained double stops: beating between two strings reads as a level rise, so the tracker reports ~129 repicks on double-stop-thirds and the filter envelope retriggers on held chords (audible). Highest priority.
- [ ] Attack ghosts that ticket 07's chord tuning added to single notes are gone: palm-mutes 11.06, 12.26, 12.84 s (extra 0.23–0.31) and chromatic-run 6.67 s (coverage 0.82, extra 0.27); also chromatic 4.21 and 9.38 s and palm-mutes 11.97 s (extra 0.10–0.11)
- [ ] Short chord changes don't carry leftover notes: double-stop-thirds 32.20 and 33.00 s (extra 0.26, 0.46)
- [ ] D2+F2 (thirds 15.65 s): F2 settles in well under 0.9 s (coverage 0.51)
- [ ] Thirds 30.60 s (G1+B1): check the take first. The spectrum shows strong 41.6 and 55.2 Hz (open E and A ringing) with weak, off-pitch G1/B1; either fix the `.notes` file with the bassist or the tracker
- [ ] open-strings 8.34 s: E1 rings on under A1 (the clip lets strings ring into the next). Decide whether the expectation should list E1 while it rings, then drop the exemption
- [ ] chromatic-run 16.10 and 18.90 s: fretted D2/E2 read −20/−18 c against a 20 c tolerance (the notes header measured ~15 c flat; open strings read 0..+3 c and no IF bias was found). Decide with the bassist whether the tolerance or the expectation changes
- [ ] Major sevenths (extra data) E1+D#2 at 2.48 s: coverage [0.72 0.86], extra 0.68
