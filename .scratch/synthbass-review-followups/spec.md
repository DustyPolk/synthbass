Status: ready-for-agent

# Spec: Synthbass v1 review follow-ups

## Problem Statement

Synthbass v1 is built and playable, but a two-axis code review of the integration branch against the v1 spec (`.scratch/synthbass/spec.md`) found places where it falls short.

As the bassist, two of them are audible. A new note can fail to sound when four voices are already playing and it is picked softly. And glide only works when two notes overlap, so a bass line with any gap between notes never slides like a mono Moog.

As the developer, the engine library can't yet be reused unchanged by a future plugin, because it only takes single parameter changes and the patch record lives in the app. Several tests reach into engine internals that the v1 testing rules forbid. The code also drifts from the glossary ("input" for the dry signal, "sound" for a patch), and duplicated logic plus repeated switches mean one change to a parameter touches half a dozen places.

## Solution

A follow-up pass on the same codebase. Nothing new for the bassist to learn, but:

- Every newly picked note always gets a voice. When all four voices are busy, it replaces the quietest one.
- Glide behaves like a mono Moog. A new note glides from the last note played if it starts within a short gap after that note ended, as well as when notes overlap.
- Oscillator 2 is audible by default. The oscillator mix knob starts at an even blend, and every built-in patch that sets oscillator 2 can be heard using it.
- The engine accepts a whole patch at once, and owns the patch record. The app only stores and loads it.
- All engine tests go through the engine's public interface, and the recorded-clip tests also check the tuner and odd and varying buffer sizes.
- The code uses the glossary's vocabulary. Each piece of duplicated domain logic lives in one place, and each parameter's facts (range, default, label, patch key) are defined once.

## User Stories

### Playing

1. As a bassist, I want every note I pick to sound even when four voices are already playing, so that new notes always speak.
2. As a bassist, I want a softly picked fifth note to replace the quietest sounding voice rather than be dropped, so that my playing dynamics never silence a note I just played.
3. As a bassist, I want a note that wasn't picked (an overtone or a lingering ghost) not to steal a voice from a real note, so that fixing story 1 doesn't bring back attack ghosts.
4. As a bassist, I want a new note to glide from the previous note when I play legato or with a short gap, so that glide feels like a mono Moog's portamento.
5. As a bassist, I want no glide when the gap since the last note is long, so that a new phrase starts on pitch.
6. As a bassist, I want the notes of a double stop picked together never to glide into each other, so that chords stay clean with glide on.
7. As a bassist, I want glide to stay at zero time when the glide knob is at zero, so that glide-off patches are unchanged.
8. As a bassist, I want glide to apply only when a voice moves to a new note, so that slides, bends and vibrato within a note are never smeared.
9. As a bassist, I want a voice that takes over a new note (replacement or reuse) to follow the same glide rule as a starting voice, so that a stolen voice never swoops in from an unrelated pitch.
10. As a bassist, I want oscillator 2's waveshape, octave and detune to be audible by default, so that "thicken the sound" works without finding a hidden knob.
11. As a bassist, I want every built-in patch that sets oscillator 2 to actually use it, so that its octave and detune settings change the sound.
12. As a bassist, I want the built-in patches to keep their character and stay balanced in loudness after this change, so that my favourites still sound right.
13. As a bassist, I want the window's dry signal meter labelled with the glossary's term, so that the window and the docs use the same words.

### Engine reuse

14. As the developer, I want to hand the engine a whole patch in one call, so that a host (the app now, a plugin later) can recall a sound without knowing every parameter.
15. As the developer, I want the patch record (every sound parameter, versioned, with defaults for missing fields) defined in the engine library, so that a plugin wrapper reuses it unchanged.
16. As the developer, I want applying a whole patch to be smoothed like individual changes, so that switching patches while playing doesn't click.
17. As the developer, I want applying a whole patch to stay realtime-safe, so that patch switching can't cause dropouts.
18. As the developer, I want the app's patch files to keep loading after the record moves, so that the bassist's saved patches survive the change.
19. As the developer, I want the noise gate threshold to remain outside the patch, so that switching sounds doesn't change how the app copes with room noise.

### Tests

20. As the developer, I want every engine test to drive the engine only through its public interface, so that refactoring the tracker or voices never breaks a test.
21. As the developer, I want the ladder filter, voice and tracker matching internals to stop being public, so that nothing outside the engine depends on them.
22. As the developer, I want the behaviour the internal tests covered (ladder slope and resonance, independent voice filter sweeps, reported notes keeping their identity over pending ones) re-expressed as public-interface tests, so that coverage isn't lost.
23. As the developer, I want the open-string and chromatic clip tests to check the tuner reading, so that the spec's "correct tuner reading" is verified on real playing.
24. As the developer, I want the recorded-clip tests to run at odd and varying buffer sizes as well as 128, so that buffer size can't change what the tracker hears.
25. As the developer, I want a test proving a soft fifth note still gets a voice, so that the "quietest of five is dropped" test can't pass by dropping the new note.
26. As the developer, I want tests for glide across a short gap, no glide across a long gap, and no glide between notes picked together, so that the mono-Moog behaviour is pinned down.

### Vocabulary and structure

27. As the developer, I want the engine's readout and meter named for the dry signal rather than "input", so that code follows the glossary.
28. As the developer, I want the app's shared parameter store named for the patch rather than "sound", so that code follows the glossary.
29. As the developer, I want JACK port pickers to keep saying "input port" and "output port", with the glossary clarifying that ports aren't the dry signal, so that standard audio terms stay where they belong.
30. As the developer, I want the tracker's per-note bookkeeping type, the sieve's bare adjective constants, and oscillator 1's fields renamed to say what they hold, so that the tracker reads clearly next to the tracked layer.
31. As the developer, I want one helper for one-pole smoothing coefficients, so that the time-constant formula isn't copied nine times.
32. As the developer, I want one place for note-number/frequency conversion and one for note-name-plus-cents, used by both the engine's tuner and the window, so that they can't disagree.
33. As the developer, I want one atomic-file-write helper shared by patch storage and settings, so that both are saved the same safe way.
34. As the developer, I want the recording command to choose the Scarlett port with the same logic as the app, so that both default to the same input.
35. As the developer, I want each parameter's range, default, label, display format and patch key defined once, so that adding a parameter is one edit instead of six.
36. As the developer, I want port slots iterated from one list, so that the routing code has no hand-written triples.
37. As the developer, I want waveshape and octave to be real types at the patch and UI boundary, so that a value isn't read three different ways.
38. As the developer, I want the noise gate threshold to be a type that validates itself, so that clamping happens in one place.
39. As the developer, I want oscillator 2's octave, detune and resulting pitch ratio grouped together, so that they change as one unit.
40. As the developer, I want unused public API (a detected note's frequency method) removed, so that the engine's surface is only what's used.
41. As the developer, I want the window's selector widgets and the engine's fixed-capacity lists to share their common shape, so that near-copies don't drift.

## Implementation Decisions

- **Scope**: a refactor-and-fix pass on the existing v1 codebase. ADRs 0001–0003 still hold: the hybrid of direct and tracked layers, the polyphonic tracked layer via a semitone filterbank, and the pure Rust engine library.
- **Voice stealing**: a note detected with a pick (an onset or repick) always gets a voice. If none is free, it replaces the quietest sounding voice, whatever its own level. The existing loudness margin stays only for notes that appear without a pick, so overtones and re-emerging notes still can't steal voices from real notes.
- **Glide (mono-Moog rule)**: the engine remembers the pitch of the most recently ended voice and when it ended. A voice that starts on a new note (or is reassigned) glides from:
  - the newest still-sounding voice, under the existing rule (it started at least 50 ms earlier, so a double stop doesn't glide into itself); otherwise
  - the most recently ended voice, if it ended within a short **glide memory** window.

  Glide memory is a fixed engine constant, on the order of 250 ms. It is not a new knob; tune it when the bassist plays. Glide time 0 still means no glide, and glide still never applies within a note (slides, bends, vibrato). This changes ticket 08's documented behaviour, so update the ticket 08 notes and the glossary's **Glide** entry to match.
- **Oscillator mix**: the oscillator mix parameter stays. Its default becomes an even blend of oscillators 1 and 2. Review the built-in patches: each that sets oscillator 2's waveshape, octave or detune gets a mix where oscillator 2 is audible. Keep the built-ins' loudness balanced (within about 3 dB, as ticket 11 measured). Add **Oscillator mix** to the glossary if it's a domain term the bassist uses; it is a knob on the panel, so it is.
- **Patch record moves into the engine**:
  - The engine library owns the patch record: every sound parameter, a version, defaults for missing fields, and a text encode/decode that is human-readable and backward compatible with existing patch files.
  - The engine gains an operation that applies a whole patch, smoothed like individual changes and allocation-free on the audio thread.
  - The app keeps patch storage (files, built-in patches, protection, last-used) and settings, and uses the engine's record.
  - The engine still has no audio-backend or GUI dependencies. If the text format needs a serialization dependency, keep it small and confined to the patch record.
- **Parameter metadata in one place**: a single table, or a method set on the parameter type, gives each parameter's range, default, label, display format and patch key. The engine's per-layer dispatch on parameters may remain (each layer reacts differently), but the app's patch field mapping, the window's knob formatting and range/default lookups all read the shared metadata. Parameter lookup by index is constant-time.
- **Types at the boundaries**:
  - Waveshape (saw/square) and octave (−1/0/+1) become small enums. Parameter values can stay `f32` for lock-free sharing, but conversion happens in exactly one place.
  - The noise gate threshold becomes a validated type, clamped to its range and rejecting non-finite values. The engine and settings loading both use it.
  - Oscillator 2's octave, detune and pitch ratio become one grouped value.
- **Shared helpers**:
  - A one-pole coefficient helper for the time-constant formula.
  - One note-number/frequency conversion pair and one note-name-plus-cents formatter, public from the engine for the window's use.
  - One atomic-file-write helper in the app.
  - The recording command reuses the app's Scarlett port matching, including its sort.
  - The window's selector widgets share one generic widget.
  - The engine's fixed-capacity lists share one small generic container.
- **Glossary alignment**:
  - The engine's readout field and meter for the dry signal's level are named for the dry signal, and the window labels it accordingly.
  - The app's shared parameter store is named for the patch.
  - Add a glossary note that JACK input and output ports are audio routing, not the dry signal, so "input port" stays.
- **Renames**:
  - The tracker's per-candidate bookkeeping type gets a name that doesn't clash with the tracked layer.
  - The sieve's bare adjective constants get names that say what they measure.
  - Oscillator 1's fields are named as oscillator 1.
  - The audio callback's nested engine field access is flattened.
- **Visibility**: the ladder filter, voice, sound and tracker matching become crate-private once their tests move to the public interface. `DetectedNote::frequency` is deleted.
- **Ordering**: do the structural moves first so the fixes land on the simpler base:
  1. patch record into the engine, plus parameter metadata;
  2. test migration to the public seam;
  3. behaviour fixes (stealing, glide, oscillator mix and built-in patches);
  4. renames and helpers last.

## Testing Decisions

- **What makes a good test**: as in v1, tests drive the engine through its public interface only. They feed audio blocks, apply a whole patch or single parameters, and assert on output audio and the readout. A refactor of tracker or voice internals must not break any test. No new exemptions in the recorded-clip tests. Existing exemptions are only removed when fixed.
- **Seam 1, the engine's public interface (extended with apply-whole-patch)**:
  - Migrated tests:
    - the ladder filter's 24 dB/octave slope, resonance peak and stability, observed through the direct layer's output at known settings;
    - two voices sweeping their filters independently, via two notes started at different times in a synthetic signal;
    - a reported note keeping its voice id when a pending candidate sits nearby, via a synthetic attack plus a held note, asserting the voice id in the readout never changes.
  - New tests:
    - A soft fifth picked note gets a voice, and the quietest older voice is the one dropped.
    - Glide happens from a note that ended within the glide memory, and not across a longer gap.
    - Notes picked together never glide.
    - A reassigned voice obeys the glide rule.
    - The default oscillator mix makes oscillator 2's octave audible (energy at the octave).
    - Applying a whole patch mid-stream produces no click and allocates nothing.
    - A whole patch applied at once matches the same values applied one by one.
  - Recorded clips:
    - The open-string and chromatic tests assert the tuner reading per event, within each clip's cents tolerance.
    - The recorded-clip runs repeat at odd and varying buffer sizes (for example 61, 128, 512 in rotation) as well as 128, with the same pass criteria.
- **Seam 2, patch storage (app)**: the existing round-trip, built-in, corrupt-file, old-version and protection tests keep passing against the engine-owned record. Add a test that a patch file written by v1 loads identically. Built-in patch tests also check that every built-in setting oscillator 2 has an oscillator mix where oscillator 2 is audible.
- **Prior art**: `crates/engine/tests/` (public-interface tests, synthetic signals, the allocation-counting realtime-safety test with its per-thread counter), `crates/engine/tests/recorded_polyphony.rs` and `slides_and_glide.rs` (recorded clips with per-event scoring), `crates/engine/tests/common/clip.rs` (clip running at chosen block sizes), and the app's patch storage tests.
- **Not automatically tested**: JACK integration and the window, as in v1. Renames and helper extraction are covered by the existing suite staying green.

## Out of Scope

- The tracker quality gaps left by ticket 12: a long D2+F2 third dropping out under the noise gate, and the E1+D#2 seventh's low-voice octave flip. These need their own tracker work.
- "Verified by playing" checks from v1 (built-in patches, double stops, slides, unplugging the Scarlett, a live 44.1 kHz rate change). They remain for the bassist.
- A glide memory knob. The window stays a fixed engine constant unless playing shows it needs to vary.
- Removing the oscillator mix parameter.
- A plugin wrapper. This spec only makes the engine ready for one.
- New sounds, LFOs, MIDI, the Nektar SE49, a styled look, and packaging, as in v1.

## Further Notes

- **Source**: the two-axis code review of `synthbass-v1` against `master`, after all v1 tickets and the first review's ten fixes were merged. The spec findings were the engine not taking a whole patch; no tuner check on recorded clips; recorded clips not run at varying buffer sizes; the unlisted oscillator mix; a soft fifth note dropped; and glide only between overlapping notes. The standards findings were tests reaching into internals, glossary drift, and the duplication, repeated-switch and primitive-type smells listed above.
- **Decisions made without interviewing the bassist** (they confirmed the test seams): the mono-Moog glide rule with a short glide memory, and keeping the oscillator mix with an even default. Both are easy to revisit after playing.
- **Watch for regressions**: ticket 12's ghost-suppression threshold has thin margins. The voice-stealing change must not let unpicked ghosts take voices, so keep the recorded-clip suite green at every step.
