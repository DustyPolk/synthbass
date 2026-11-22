# Test clips

Recordings of real bass playing (dry signal only) that the engine tests run
against. Each clip is two files with the same name:

- `<name>.wav`: the recording. Mono, made with the recording command.
- `<name>.notes`: what was played, in the format below.

## Recording a clip

1. Plug the bass into Scarlett input 1. Close the Synthbass app if it is open.
2. From the repo root, run:

   ```sh
   cargo run --release -p synthbass --bin record -- clips/open-strings.wav
   ```

   Use another input with `--port`, e.g.
   `--port "Scarlett 4i4 4th Gen Analog Surround 2.1:capture_FR"` for input 2.
   A wrong port name prints the list of available ports.
3. Wait one second, play, let the last note ring out, then wait one more second.
4. Press Ctrl-C. The command prints the length and the peak level.
   Aim for a peak between -12 and -6 dBFS. Above -1 dBFS, turn the Scarlett
   gain down and record again. Below -20 dBFS, turn it up.
5. Write the `.notes` file next to the WAV (see below).

Running it again with the same name overwrites the clip.

## Which clips to record

Standard tuning (E1 A1 D2 G2). Play cleanly, at a steady medium volume
unless the clip says otherwise. Leave a short silence between notes so each
onset is clear.

| File name | What to play |
|---|---|
| `open-strings` | Each open string, low to high: E, A, D, G. Let each ring about 2 s, then mute it before the next. |
| `chromatic-run` | E string, frets 0 to 12, one note per beat at about 80 bpm, each note muted before the next. |
| `double-stop-thirds` | Three or four major or minor thirds held about 2 s each, e.g. A1+C#2 (E string fret 5 + A string fret 4) and C2+E2 (A string fret 3 + D string fret 2). |
| `double-stop-tenths` | Three or four tenths held about 2 s each, e.g. E1+G#2 (open E + G string fret 1) and G1+B2 (E string fret 3 + G string fret 4). |
| `palm-mutes` | E string, frets 0, 3, 5 and 7, palm-muted eighth notes, about four notes each. |
| `slides` | Three slides on the A string: fret 2 to fret 7, fret 7 to fret 2, fret 5 to fret 12. Pick once and slide; hold the arrival note about 1 s. |

More clips are welcome (dynamics, a five-string low B, a riff). Use short,
lowercase, hyphenated names.

## The `.notes` format

One line per event: a stretch of time and the notes that sound during it.

```text
# open-strings: open strings, low to high
timing_tolerance 0.1
cents_tolerance 20

1.02 3.10 E1
3.60 5.55 A1
6.05 8.00 D2
8.50 10.40 G2
```

- `<onset> <offset> <note> [<note>...]`: times in seconds from the start of
  the file, offset after onset. List every note sounding at once on one line,
  e.g. `2.0 4.0 A1 C#2` for a double stop.
- Notes are names in scientific pitch (A4 = 440 Hz; open E on a four-string
  is `E1`), with `#` or `b` for sharps and flats: `E1`, `F#1`, `Bb1`, `B0`.
  A frequency also works: `98Hz`.
- A slide is `from->to`, e.g. `B1->E2`. The onset is the pick, the offset is
  when the arrival note stops.
- Palm mutes are ordinary notes; give each muted note its own line.
- A note in parentheses, e.g. `(E1)`, is still ringing from before: it may
  sound during the event but need not. Use it when a string is left to ring
  into the next note.
- `timing_tolerance <seconds>` and `cents_tolerance <cents>` set how far
  detection may stray for this clip. Defaults: 0.05 s and 20 cents. Raise
  the timing tolerance if the times are rough.
- `#` at the start of a field starts a comment. Blank lines are ignored.

To find the times, open the WAV in an audio editor such as Audacity and read
where each note starts and stops in the waveform. Rough times to about 0.1 s
are fine with `timing_tolerance 0.1`.

## Using clips in tests

`crates/engine/tests/common/clip.rs` has the helpers:

```rust
let clip = load_clip(&clip_path("open-strings")); // WAV + .notes, panics on errors
let run = run_clip(&clip, &[128, 61, 512]);        // engine at the clip's rate
run.output.left;                                   // rendered output
run.readouts;                                      // (seconds, Readout) after each block
clip.expected.events;                              // expected notes, with tolerances
```
