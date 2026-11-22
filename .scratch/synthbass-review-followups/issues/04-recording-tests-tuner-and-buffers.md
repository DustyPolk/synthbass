# 04: Recording tests check the tuner and buffer sizes

**What to build:** the recorded-clip tests verify two things the v1 spec asked for but never checked on real playing: the open-string and chromatic clips assert the tuner reading for each event within the clip's cents tolerance, and every recorded-clip run repeats at odd and varying buffer sizes (for example 61, 128 and 512 in rotation) as well as 128, with the same pass criteria. If a buffer size changes what the tracker hears, fix the engine at the root rather than loosening the test.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs. This ticket runs the recorded-clip suite; keep its output to per-clip pass/fail and delegate bulky clip analysis (traces, spectra) to a subagent. If not converging by about 100K tokens, stop, record findings in a `## Comments` section and split the rest into a new ticket.

- [x] Open-string and chromatic clip tests assert the tuner's note and cents per event (blank where no clear single note is expected)
- [x] All recorded-clip suites run at 128 and at an odd/varying rotation, with identical pass criteria
- [x] Any buffer-size dependence found is fixed in the engine, with the cause written in the ticket's `## Comments`
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions

## Comments

**Tuner.** `score_tuner` (tests/common/clip.rs) judges each event's window: for an event with one steady
note the tuner must name that note (MIDI note, and cents within the clip's tolerance) for the same share
of the window a voice must cover it (0.85), and show any other reading for at most the voice "extra"
share (0.1). Where no clear single note is expected (several notes, or `(E1)` ringing on), blank is
right and a reading of one of the event's notes is accepted. Results at 128: open strings 1.00 on note
for every event (A1 + ringing E1: blank or A1 throughout); chromatic run 0.88–1.00 on note, at most 0.06
other readings (release tails drifting past the tolerance at note ends).

**Buffer sizes.** Every recorded-clip check (voices, tuner, held-double-stop repicks, slides) runs at
`BLOCK_PATTERNS` = `[128]` and `[61, 128, 512]` in rotation with the same bar; all pass at both.

**Buffer-size dependence found and fixed.** A new test renders every clip with the tracked layer
audible at both patterns and requires identical samples. It failed at the first voice of open strings
(6.398 s). Cause: `Engine::process` cut each block into 32-sample chunks starting at the block's first
sample, and the voices took the tracker's notes at each chunk start. With 128-sample blocks that is
every 32 samples of the stream; with 61 it is chunks of 32 and 29, so voices started, stole, glided and
followed the string level on different samples depending on the host's buffer, and the string level
and glide timer counted partial chunks. Fix: the engine counts the 32-sample voice-update grid across
blocks (`until_voice_update`), and the voices update from what the tracker and string-level follower
have heard up to that sample (`TrackedLayer::listen` feeds the dry signal; `update` takes only the
notes). The output is now sample-identical at any block size. Side effect: a detection now reaches the
voices at the next 32-sample boundary after it rather than at the start of the chunk it happened in
(up to 32 samples, 0.67 ms, later); every existing bar still passes.
