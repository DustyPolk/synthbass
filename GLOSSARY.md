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

**Oscillator mix**:
The balance between a voice's two oscillators, from oscillator 1 alone to oscillator 2 alone. It starts at an even blend, so oscillator 2's waveshape, octave and detune are heard. The sub-oscillator has its own level and sits outside the mix.
_Avoid_: Oscillator balance, crossfade, oscillator 2 level

**Filter envelope**:
A Moog-style sweep of a voice's filter that fires when its note is detected and opens wider the harder the note is picked.
_Avoid_: ADSR, contour

**Glide**:
A voice sliding into a new note's pitch over a set time instead of jumping, like Moog portamento. It applies only when a voice changes note; slides, bends and vibrato within a note are followed exactly.
_Avoid_: Portamento (as a control name), slew
