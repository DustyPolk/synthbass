# 02: Recording command

**What to build:** a small command that records the dry signal from a JACK input (defaulting to Scarlett input 1) to a WAV file at the session sample rate, so the bassist can capture test clips. Alongside it, test support for loading a WAV clip together with a short expected-notes description (which notes, roughly when, with tolerances), so later tickets can assert on real playing.

**Blocked by:** 01 (Dry signal passthrough)

**Status:** done

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Recording command takes an output path and optional port; records until stopped (Ctrl-C) and writes a valid WAV
- [x] A documented expected-notes description format for clips (note names or frequencies, approximate onset/offset times, simultaneous notes)
- [x] Test helper loads a clip plus its description and runs it through the engine in block-sized chunks
- [x] A short how-to (in the clips directory) tells the bassist exactly which clips to record and how to name and describe them: open strings, chromatic run, double stops (thirds, tenths), palm mutes, slides
- [x] A test proves the helper works on a synthetic WAV generated in the test
