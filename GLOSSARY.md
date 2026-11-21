# Synthbass

A standalone instrument that turns a live bass guitar signal into analog-style synthesizer sound, for playing and jamming at home.

## Signal

**Dry signal**:
The clean, unprocessed bass guitar signal as it arrives from the audio interface.
_Avoid_: Input, clean signal, DI

**Direct layer**:
Synth sound shaped straight from the dry signal itself, with no pitch detection and no added delay.
_Avoid_: Instant layer, envelope synth, fuzz layer

**Tracked layer**:
Synth sound from oscillators whose pitch comes from notes detected in the dry signal.
_Avoid_: Pitch layer, oscillator layer, synth layer

**Voice**:
One sounding note in the tracked layer, with its own oscillators. Up to four voices sound at once.
_Avoid_: Note, channel

## Sound design

**Patch**:
One complete, named set of every sound parameter, saved and recalled as a unit. Some patches ship with the app.
_Avoid_: Preset, program, sound

**String following**:
A voice's loudness tracks the loudness of its note on the bass, so dynamics, palm mutes and natural decay pass through as played.
_Avoid_: Envelope follower (as a domain term), amp envelope

**Filter envelope**:
A Moog-style sweep of a voice's filter that fires when its note is detected and opens wider the harder the note is picked.
_Avoid_: ADSR, contour
