# 05: Note detection and tuner

**What to build:** the tracker's foundation (ADR 0002): a semitone-spaced filterbank covering at least low E (≈41 Hz) to the top of a fretted four-string's G string, with continuous pitch refinement within a band and the global noise gate. The window shows a working tuner (note name and cents sharp or flat, blank when no clear single note) and a noise gate threshold control. No voices sound yet; the readout carries the detected notes so the tuner and tests can see them.

**Blocked by:** 01 (Dry signal passthrough)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Readout includes a tuner reading (note, cents, or none) and the list of detected notes with pitch and level
- [x] Noise gate threshold is set on the engine separately from sound parameters
- [x] Offline tests with synthetic tones (with harmonics, bass-like decay): every semitone from low E to the top of the G string is detected within a few cents; detection latency on low E is within the physical bound (about two periods) plus a small margin; nothing below the gate is detected; silence gives no tuner reading
- [x] Processing path stays allocation-free
- [ ] Verified by playing: the tuner reads open strings correctly
