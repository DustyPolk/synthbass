Status: ready-for-agent

# Spec: Synthbass v1

## Problem Statement

I play bass guitar at home through a Focusrite Scarlett 4i4 on Linux, and I want to play synthesizer sounds with it — especially the old analog Moog-style bass sound — without buying hardware synth pedals or running a DAW. Off-the-shelf options either track pitch so slowly that low notes feel mushy, only handle one note at a time, or don't run as a simple standalone Linux app. I want to plug in, launch one program, and jam.

## Solution

Synthbass is a standalone Linux desktop app. It takes the dry signal from Scarlett input 1 and plays analog-style synth sound out of Scarlett outputs 1/2 in real time, through PipeWire's JACK API.

The sound comes from two layers blended with the dry signal:

- The **direct layer** shapes the dry signal itself (drive into a Moog-style ladder filter that opens as you dig in). It adds no latency and works on chords, so every note has an instant attack.
- The **tracked layer** detects the notes you play — up to four at once — and gives each one a **voice** with real oscillators, a sub-oscillator, glide, and its own ladder filter with a **filter envelope**. Voice loudness uses **string following**, so dynamics, palm mutes, and natural decay come through as played. Voices follow exact pitch, so slides, bends, and vibrato carry into the synth.

A simple dark window shows three sections (direct, tracked, mix), a tuner, an input meter, and a **patch** picker. Patches are saved as named files; about six Moog-style patches ship with the app. An output knob and a hard limiter protect ears from resonance spikes.

## User Stories

### Getting sound

1. As a bassist, I want the app to connect Scarlett input 1 to outputs 1/2 automatically at launch, so that I can plug in and play without routing anything.
2. As a bassist, I want to choose a different input port from a dropdown, so that I can use input 2 or another interface if I plug in elsewhere.
3. As a bassist, I want to choose different output ports from a dropdown, so that I can send sound to other outputs or devices.
4. As a bassist, I want the app to run through PipeWire's JACK API, so that my other desktop audio keeps working while I play.
5. As a bassist, I want round-trip latency through the direct layer to feel instantaneous (a 64–128 sample buffer at 48 kHz), so that the synth feels like part of the instrument.
6. As a bassist, I want a clear message in the window if the audio server isn't reachable or the Scarlett isn't present, so that I know why there's no sound.
7. As a bassist, I want the app to keep running and let me reconnect if the chosen ports disappear (e.g. the interface is unplugged), so that I don't have to restart.
8. As a bassist, I want an input level meter, so that I can set the Scarlett's gain knob to a healthy level without clipping.

### Direct layer

9. As a bassist, I want the direct layer to square up my bass signal with a drive control, so that I get a synthy, buzzy tone with zero delay.
10. As a bassist, I want the direct layer's sound to pass through a Moog-style ladder filter with cutoff and resonance controls, so that it has that classic analog character.
11. As a bassist, I want the direct layer's filter to open as I play harder, with an envelope sensitivity control, so that it responds like an envelope-filter pedal.
12. As a bassist, I want the direct layer to work with chords and double stops, so that it never glitches on multiple notes.
13. As a bassist, I want a direct layer level control, so that I can blend it against the tracked layer or turn it off.

### Tracked layer — detection

14. As a bassist, I want the tracked layer to detect each note I play on my fretted four-string bass in standard tuning (low E at 41 Hz up through the top of the G string), so that voices sound at the right pitch across the whole neck.
15. As a bassist, I want up to four notes ringing at once to each get their own voice, so that double stops and chords play as synth chords.
16. As a bassist, I want intervals of a third, a tenth, or wider to track as separate voices, so that common bass double stops work.
17. As a bassist, I accept that octaves and fifths may fuse into a single voice, so that the polyphonic experiment stays feasible.
18. As a bassist, I want a fifth simultaneous note to replace the quietest sounding voice, so that new notes always speak and I don't notice the one that drops.
19. As a bassist, I want voices to follow my exact pitch, so that slides, bends, and vibrato come through on the synth.
20. As a bassist, I want a voice to keep its identity while I slide across several frets, so that a slide sounds like one gliding note rather than a series of new notes.
21. As a bassist, I want a global noise gate threshold, so that hum, pickup noise, and string squeak don't start voices.
22. As a bassist, I want voices to end when I mute a string or the note decays below the gate, so that the synth stops when I stop.

### Tracked layer — sound

23. As a bassist, I want each voice to have a first oscillator with a saw or square waveshape, so that I can pick the basic tone.
24. As a bassist, I want a second oscillator with its own waveshape, an octave switch (−1, 0, +1), and a detune control, so that I can thicken the sound the way classic Moog patches do.
25. As a bassist, I want a sub-oscillator one octave below with a level control, so that I get deep synth-bass weight.
26. As a bassist, I want a glide control, so that voices slide into new pitches like Moog portamento.
27. As a bassist, I want each voice to have its own Moog-style ladder filter with cutoff and resonance controls, so that notes starting at different times each get a correct filter sweep.
28. As a bassist, I want each voice's filter envelope to fire when its note is detected, with amount and decay controls, so that I get the classic "bwow" pluck.
29. As a bassist, I want the filter envelope to open wider the harder I pick, with a pick sensitivity control, so that my picking dynamics shape the tone.
30. As a bassist, I want each voice's loudness to follow its string (string following), so that dynamics, palm mutes, and natural decay pass through exactly as played.
31. As a bassist, I want a tracked layer level control, so that I can blend it against the direct layer and dry signal.
32. As a bassist, I want the direct layer's instant attack to cover the tracked layer's detection delay on low notes, so that the combined sound feels immediate even when tracking lags.

### Mix and safety

33. As a bassist, I want a dry signal level control, so that I can keep the clean low end and pick attack under the synth.
34. As a bassist, I want an output level control, so that I can set overall volume from the app.
35. As a bassist, I want a hard limiter at the end of the chain, so that a resonance spike or feedback never blasts my ears or monitors.
36. As a bassist, I want silence out when I'm not playing (beyond the dry signal's own noise), so that the app doesn't hiss or drone between notes.

### Tuner

37. As a bassist, I want a tuner readout showing the detected note name and how many cents sharp or flat I am, so that I can tune without a separate tuner.
38. As a bassist, I want the tuner to show nothing (rather than flicker) when no clear single note is present, so that it's readable.

### Patches

39. As a bassist, I want every sound parameter saved together as a named patch, so that I can recall a whole sound at once.
40. As a bassist, I want to pick a patch from a list in the window, so that I can switch sounds quickly between songs.
41. As a bassist, I want to save my current settings as a new named patch, so that I keep sounds I like.
42. As a bassist, I want to overwrite an existing patch I created, so that I can refine a sound over time.
43. As a bassist, I want to delete patches I created, so that the list stays tidy.
44. As a bassist, I want about six Moog-style patches built in, so that I have great sounds immediately.
45. As a bassist, I want built-in patches to be protected from accidental overwrite or deletion, so that I can always get back to a known-good sound.
46. As a bassist, I want my patches stored as files under my config directory, so that I can back them up or copy them between machines.
47. As a bassist, I want the window to show when the current settings differ from the loaded patch, so that I know I have unsaved tweaks.
48. As a bassist, I want the app to reload the last patch I used at startup, so that I pick up where I left off.
49. As a bassist, I want a corrupt or unreadable patch file to be skipped with a message rather than crash the app, so that one bad file doesn't stop me playing.
50. As a bassist, I want patch files from an older version of the app to still load, with sensible defaults for any new parameters, so that upgrades don't lose my sounds.
51. As a bassist, I want the noise gate threshold kept outside patches, so that switching sounds doesn't change how the app copes with my room's noise.

### Window

52. As a bassist, I want a single dark window with three labelled sections — direct, tracked, mix — so that I can find any control fast.
53. As a bassist, I want parameter changes from the window to take effect smoothly and immediately while I play, so that tweaking knobs doesn't click, pop, or glitch.
54. As a bassist, I want the window to show which tracked voices are active, so that I can see how the tracker is reading my playing.
55. As a bassist, I want the audio to keep running smoothly even if the window is busy or minimised, so that the UI never causes dropouts.

### Development and tuning

56. As the developer, I want a small recording command that captures dry signal from the Scarlett to a file, so that the bassist can record test clips.
57. As the developer, I want a set of recorded clips — open strings, a chromatic run, double stops, palm mutes, and slides — kept in the repo, so that the tracker can be tuned and tested against real playing.
58. As the developer, I want to run the engine offline over a recorded clip and inspect its output and readout, so that I can tune detection without playing live every time.
59. As the developer, I want the app launchable with a release build via the standard cargo command, so that running it is one step.

## Implementation Decisions

- **Language and stack** (ADR 0003): Rust. Audio through the `jack` crate against PipeWire's JACK API. Window with egui.
- **Two-part structure**: a pure **engine library** with no audio-backend or GUI dependencies, and a thin **app** that owns the JACK client, the egui window, and patch storage on disk. A later plugin wrapper would reuse the engine library unchanged.
- **Engine interface** (the primary test seam):
  - Constructed with a sample rate and maximum block size.
  - Processes a block of mono dry-signal samples into a block of output samples (stereo or dual-mono to outputs 1/2).
  - Accepts a full patch and individual parameter changes; changes are smoothed internally to avoid clicks.
  - Accepts the global noise gate threshold separately from the patch.
  - Exposes a **readout** snapshot: input level, tuner reading (note, cents, or none), and the active voices with pitch and loudness. The window displays it; tests assert on it.
  - Must be realtime-safe in its processing path: no allocation, locking, or I/O. All memory sized at construction.
- **App ↔ engine communication**: parameter changes flow from the UI thread to the audio thread, and readout snapshots flow back, through lock-free channels or atomics. The audio thread never blocks on the UI.
- **Signal chain**: dry signal → (direct layer ∥ tracked layer) → mix with dry signal level → output level → hard limiter → outputs.
- **Direct layer**: drive/waveshaper → Moog-style ladder filter whose cutoff is modulated by the dry signal's level scaled by envelope sensitivity → level.
- **Tracked layer** (ADR 0002): semitone-spaced filterbank analysis covering at least the fretted four-string range (low E ≈ 41 Hz upward). Active bands become voices, capped at four. Each voice refines its pitch continuously rather than snapping to the band centre, and keeps its identity as its pitch moves across adjacent bands (slides). Voice stealing replaces the quietest voice. Detection respects the global noise gate.
- **Voice**: oscillator 1 (saw/square) + oscillator 2 (saw/square, octave −1/0/+1, detune) + sub-oscillator (square, −1 octave, level) → per-voice Moog-style ladder filter with a filter envelope triggered on detection (amount, decay, pick sensitivity scaling the envelope by onset strength) → loudness by string following. Glide applies when a voice's pitch target changes.
- **Hybrid** (ADR 0001): the direct layer is always latency-free; the tracked layer fades in as notes are detected. No attempt to delay the direct layer to align with tracking.
- **Patch**: a named, versioned record of every sound parameter (direct layer, tracked layer, mix). The noise gate threshold is not part of a patch. Unknown or missing fields fall back to defaults so older patch files keep loading.
- **Patch storage**: one human-readable file per patch in a patches directory under the user's config directory. Built-in patches are embedded in the app and read-only; user patches are files. The last-used patch name is remembered in a small app settings file alongside the global noise gate threshold and last-used ports.
- **Startup**: register the JACK client, auto-connect to Scarlett input 1 and outputs 1/2 when present, load the last-used patch (falling back to a built-in default).
- **Audio format**: 48 kHz, buffer size set by PipeWire (target 64–128 samples). The engine must work for any block size up to its configured maximum.
- **Recording command**: a separate small binary or subcommand that records the dry signal from a JACK input to a WAV file, used to build the test clip set.
- **Window**: plain dark egui panel with direct, tracked, and mix sections; tuner; input meter; active-voice indicator; patch picker with save, save-as, and delete; unsaved-changes marker; port dropdowns; noise gate threshold control.

## Testing Decisions

- **What makes a good test**: tests drive the engine through its public interface only — feed blocks of audio, set a patch or parameters, then assert on output audio and the readout. They never reach into filterbank internals, voice structs, or filter state. A refactor of the tracker's internals should not break any test.
- **Seam 1 — the engine library** (primary). Tests load recorded bass clips, process them in block-sized chunks (including odd and varying block sizes), and assert:
  - Open-string and chromatic clips yield one voice at the expected pitch per note, and a correct tuner reading.
  - Double-stop clips (thirds, tenths) yield two voices at the expected pitches.
  - More than four simultaneous notes never yields more than four voices.
  - Slide clips keep a single voice whose pitch moves continuously.
  - Palm mutes and note decay drop voice loudness and end voices below the gate.
  - Silence in gives silence out (with the dry signal level at zero), and nothing below the noise gate starts a voice.
  - The limiter keeps output under its ceiling for a pathological patch (max resonance, max drive) and a loud input.
  - Setting a layer's level to zero removes that layer's contribution.
  - Parameter changes mid-stream don't produce discontinuities beyond a threshold (no clicks).
  - The direct layer responds within the same block as an onset (zero added latency).
  - Some assertions need synthetic signals as well as recorded clips, e.g. pure tones at known frequencies to check tracking accuracy in cents, and step changes to check limiter and smoothing behaviour.
- **Seam 2 — patch storage**. Tests assert: save then load round-trips every parameter; every built-in patch loads; a corrupt file is reported and skipped without panicking; a file missing newer fields loads with defaults; built-in patches can't be overwritten or deleted.
- **Not automatically tested**: the JACK integration and the egui window. Both stay thin and are verified by playing.
- **Prior art**: none. This is a new repo; these tests establish the patterns.
- **Test clips**: the bassist records them with the recording command. Each clip has an expected-notes description (which notes, roughly when) that the tests check against with tolerances for timing and cents.

## Out of Scope

- MIDI output, and MIDI input or control from the Nektar SE49 keyboard or any other controller.
- LFOs, wobble, extra oscillator waveshapes, FM, pads, or any non-Moog-style sound beyond what the listed parameters can reach.
- Effects other than the limiter (no reverb, chorus, delay, compression).
- Plugin formats (VST3, CLAP, LV2).
- Five-string, six-string, fretless, or alternate tunings as a design target (they may work, but aren't tuned for).
- More than four simultaneous voices; reliable separation of octaves and fifths.
- A styled vintage look; theming beyond a plain dark panel.
- Packaging, an install target, or a desktop launcher entry.
- Direct ALSA access or any audio backend other than JACK through PipeWire.

## Further Notes

- The biggest risk is polyphonic tracking quality from a single ordinary pickup. ADR 0002 names falling back to monophonic tracking as the escape hatch if it can't be made clean enough; the engine interface (voices in the readout) doesn't change either way.
- Tracking latency on low notes is physically bounded (low E's period is about 24 ms). The hybrid design relies on the direct layer to make the combined sound feel instant; don't spend effort trying to beat physics in the tracker.
- The user's realtime privileges are already in place (`realtime` group, rtprio up to 98), so the JACK client can run its process callback at realtime priority.
- See `GLOSSARY.md` for terms (dry signal, direct layer, tracked layer, voice, patch, string following, filter envelope) and `docs/adr/` for the three recorded decisions.
