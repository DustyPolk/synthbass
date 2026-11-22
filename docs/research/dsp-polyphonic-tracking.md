# Polyphonic bass tracking: semitone resonator bank (ADR 0002)

Range: 4-string standard tuning, E1 = 41.20 Hz (MIDI 28) up to about G4 = 392 Hz (G string,
fret 24, MIDI 67). Up to 4 voices. Octaves and fifths may fuse (spec story 17).

## Physics first (sets expectations for tests)

- Low E period = 24.3 ms. Any method needs ~1.5–2 periods → **30–50 ms on low E**,
  ~20 ms on the G string. The direct layer covers this (ADR 0001); don't try to beat it.
- Separating two partials Δf apart needs an observation time of roughly 1/Δf to 2/Δf.
  Adjacent semitones at E1 differ by 2.4 Hz (≈0.4 s!). A **major third at E1–G#1 is 10.7 Hz
  apart → ~100 ms before it separates**. A tenth (E1–G#2, 62 Hz apart) separates fast.
  So low-register thirds will be slow and that's inherent. Write test tolerances for it.
- An FFT/CQT with semitone resolution at 41 Hz needs a ~400 ms window. That's why we use
  per-sample resonators with **wider-than-a-semitone bands at the bottom** and recover exact
  pitch from instantaneous frequency (IF), not from which band won.
- Real bass: the low-E **fundamental is often weaker than the 2nd harmonic** (pickup
  position, string physics). Never require the fundamental band to be the loudest.

## Recommended first design

```
dry 48k ─► HPF 30 Hz ─► LPF ~1.1 kHz (4th-order) ─► keep every 8th sample ─► 6 kHz analysis
   │                                                             │
   │                                         complex resonator bank, 1 band per semitone
   │                                         MIDI 26 (D1) … 79 (G5): 54 bands
   │                                         each → (magnitude, IF) per analysis sample
   │                                                             │
   │                                         every hop (8 analysis samples = 64 in = 1.33 ms):
   │                                           1. partial picking (IF-consistent peaks)
   │                                           2. harmonic sieve + iterative cancel → ≤4 notes
   │                                           3. match notes to voices (slides keep identity)
   │                                           4. per-voice pitch (period-averaged IF), loudness
   └─► HPF ~2 kHz energy ─► onset detector (full rate) ─► onset strength for filter env
```
Cost: 54 bands × 2 complex one-poles × 6 kHz ≈ 4 MFLOP/s. Decision logic per hop is tiny.
Bank must cover harmonics of the top note (2×392 = 784 Hz) for the sieve.
Decimation aliasing: 4th-order Butterworth at 1.1 kHz is ~50 dB down at 4.9 kHz (what folds
to 1.1 kHz). Group delay of the LPF at 41 Hz is well under 1 ms.

### 1. Band design: order-2 complex resonator

Each band = two cascaded complex one-poles (better skirt rejection than one; one-pole bands
let the 2nd harmonic leak in and the IF ripples):
```
p = r·e^{jω0},  r = exp(−π·B/fs_a),  ω0 = 2π·fc/fs_a,  gain = 1 − r
z1[n] = p·z1[n−1] + gain·x[n]          // x real
z2[n] = p·z2[n−1] + gain·z1[n]         // band output (complex)
mag  = |z2|            (≈ amplitude/2 of a sinusoid at fc; calibrate once)
IF   = arg( Σ_{k=0}^{P−1} z2[n−k] · conj(z2[n−k−1]) ) · fs_a / 2π
```
- **Bandwidth** `B(f) = max(f / 17, Bmin)` Hz: semitone-Q (Q≈17) up high, floored at the
  bottom so low bands respond fast. `Bmin` is the key latency/separation knob:

| order, Bmin | E1 band 50% rise | steady-state IF error, E1 & neighbours (P = 1 period) |
|---|---|---|
| 2, 8 Hz | 66 ms | ≤ 2 c |
| 2, 12 Hz | 45 ms | ≤ 2 c |
| 2, 20 Hz | 28 ms | ≤ 2 c (±15 c four bands away) |
| 1, 10 Hz | 22 ms | 7–20 c (harmonic leakage) |
(scratch sim: 6 kHz, f64, bass-like tone 41.2 Hz +10 c with harmonics 1/h). Steady state, **every
band near the partial reports the same IF** — that's what lets wide, overlapping bands still
give exact pitch. Start with **order 2, Bmin ≈ 12 Hz**; tune on clips. E1+G#1 thirds were
separated (two IF clusters at 27.9 and 32.1 MIDI) with Bmin = 8 Hz at 300 ms; wider Bmin
separates worse — test both.
- **IF averaging window P must be one period of the voice's f0** (not a fixed short window).
  Harmonic leakage makes IF ripple at f0; averaging over exactly one f0 period nulls it.
  Measured: P = ½ period → ±100 c ripple; P = 1 period → ≤ 2 c. Keep a short ring of the
  lag-1 products `z[n]·conj(z[n−1])` per band (≤ 160 entries at 6 kHz) with a running sum.
- Precision: f32 is fine at 6 kHz (1 − r ≈ 0.006–0.03). At 48 kHz rates, r gets too close to 1
  for comfort — another reason to decimate.
- Gain: with `gain = 1 − r` per stage, each stage has gain exactly 1 at fc for a complex
  input, so a real sinusoid of amplitude A at fc reads `mag ≈ A/2` in every band. No per-band
  calibration needed. Off-centre partials read lower; a partial's amplitude is the max over
  its IF cluster.

### 2. Partial picking

A band is **IF-consistent** if its IF lies within ±0.5 semitone of its own centre. A
**partial** = a local maximum of `mag` among consistent bands (or a run of adjacent bands
whose IFs agree within ~20 c — take the max-magnitude band as representative). Each partial
→ `(freq = IF, amp = mag)`. Drop partials below `gate_lin × 0.5`.

### 3. Harmonic sieve with iterative cancellation (Klapuri-style)

Candidates f0: every partial frequency in [38, 420] Hz, **plus** half of every partial
frequency in [76, 840] Hz (missing/weak fundamental). For each candidate:
```
salience(f0) = Σ_{h=1..6}  w_h · amp(partial within ±40 c of h·f0)     w_h = 1/√h  (start)
```
Loop up to 4 times:
1. Take the max-salience candidate. Accept only if
   - salience > gate threshold (global noise gate, converted to linear) and > 0.15 × the
     first accepted salience (relative floor kills weak ghost notes), and
   - **subharmonic check:** a partial exists at h = 1, **or** partials exist at both h = 2
     and h = 3. This rejects f0/2 and f0/3 ghosts, which get support from the true note's
     even (or every-third) harmonics but have nothing at their own odd harmonics.
2. **Cancel**: mark the accepted note's partials (h = 1..6 within ±40 c) as used — remove
   their amplitude from every other candidate's salience. This stops the 2nd/3rd harmonic of
   a note spawning an octave/fifth-above voice.
3. Recompute salience of the rest; repeat.

Consequences (all match the spec): a true octave dyad fuses (its upper note's partials are
consumed by the lower note). A fifth E1–B1 usually yields both (B1's fundamental 61.7 Hz is
not a harmonic of E1), but B1's 2nd harmonic = E1's 3rd, so the B1 salience is weaker and it
may fuse. Thirds and tenths share no low harmonics → separate voices once resolved.
Optional octave rescue later: if an accepted note's 2nd-harmonic partial is ≫ its expected
level (e.g. > +9 dB over the fundamental *and* above what the note had before), keep the
2nd partial as its own note. Skip in v1.

### 4. Voice assignment, identity across bands (slides), birth/death

State per voice: `pitch_cents`, `loudness`, `age`, `missed_hops`, `last_onset_hop`.
- **Match** accepted notes to active voices by cents distance with a radius of ~150 c per
  hop. A 12-fret slide in 200 ms is ~6 c/ms ≈ 8 c per 1.33 ms hop, so 150 c is generous; the
  radius mainly stops a new note capturing an old voice. With ≤ 4×4 pairs, greedy by
  smallest distance is fine.
- **Matched** → update pitch target (slide/bend/vibrato keeps the voice). If an onset fired
  since the last hop *and* the pitch moved ≥ 80 c, treat it as a **re-pluck**: retrigger the
  filter envelope but keep the voice (glide applies, story 26).
- **Unmatched note** → birth after it persists for 2–3 hops (3–4 ms; kills one-hop flukes).
  If 4 voices are active, steal the one with the lowest `loudness` (story 18).
- **Unmatched voice** → `missed_hops += 1`; release after ~8–15 hops (10–20 ms) or when its
  loudness < gate. Hysteresis: birth threshold = gate + 3 dB, keep-alive threshold = gate.
- Slides cross bands continuously: because IF clustering doesn't care which band "owns" the
  partial, the voice just follows the partial's IF as it walks through neighbouring bands.
  Look up the voice's partial each hop as "the partial nearest the voice's last pitch",
  not "the band the voice started in".

### 5. Pitch refinement per voice

- `f_voice = IF` of the voice's fundamental partial, averaged over one period of the current
  estimate. If the fundamental partial is weak/missing, use `IF(2nd)/2`; when both are
  present, magnitude-weighted average of `IF(h)/h` for h = 1, 2.
- **At birth the IF is not settled** (sim: −300 to −100 c at 25–35 ms on low E, ±30 c at
  60 ms, ≤ 5 c later). So: start the voice at the semitone of the winning sieve candidate
  (rounded), then switch to the IF value once it is within 50 c of that semitone and its
  variance over ~2 periods is < 10 c. Smooth the hand-over with the glide one-pole (~5 ms
  extra) — the tracked layer is still fading in, so this is inaudible.
- Higher-accuracy early option (also the mono fallback core): restricted-lag YIN around the
  candidate's period ±1.5 semitones on the 6 kHz signal (~25 lags × 150 samples). Synthetic
  E1 was exact by 50 ms.

### 6. Onset strength (pick sensitivity, story 29)

At full rate, independent of pitch:
```
hf  = HPF_2kHz(dry)                       // pick click + string noise live up here
e_f = follower(hf², attack 0.5 ms, release 15 ms)
e_s = follower(hf², attack 30 ms,  release 30 ms)
odf = max(0, 10·log10(e_f + ε) − 10·log10(e_s + ε))    // dB jump
onset when odf > 6 dB and ≥ 30 ms since the last one; strength = peak odf over next 5 ms
```
Fires 1–3 ms after the pick — 20–40 ms before the voice is born. Keep the last onset's
strength and time; a voice born within ~60 ms of an onset uses it. Map dB → 0..1 (e.g.
6 dB → 0, 30 dB → 1). Filter-env amount = `amount · lerp(1, strength, pick_sens)`.
Also add the dry signal's broadband level at onset as a second cue if the HF one proves
too dependent on tone-knob setting. Slides (no onset) don't retrigger.

### 7. String following (story 30)

Voice loudness = smoothed magnitude of the voice's partials (fundamental + 2nd harmonic,
summed), follower attack ~3 ms, release ~40 ms at the 6 kHz rate. Interpolate to 48 kHz with a
one-pole (~2 ms) before multiplying the oscillator output, or it zippers. Palm mutes decay the
bands quickly → loudness falls → voice ends below the gate (story 22).

### 8. Tuner (stories 37–38)

Report `(note, cents)` only if exactly one voice is active, it has been alive ≥ 60 ms, and
its pitch varied < 5 c over the last 50 ms; otherwise `None`. Hold the last value ~150 ms
before blanking to stop flicker.

## Monophonic fallback (ADR 0002 escape hatch)

YIN (de Cheveigné & Kawahara 2002) on the same 6 kHz decimated signal; readout keeps the same
shape with 0 or 1 voice.
- Window W = 150 samples (25 ms), lags τ ∈ [12, 160] (500 Hz … 37.5 Hz), threshold 0.10–0.15,
  first dip below threshold then walk to the local minimum, parabolic interpolation on
  d′(τ). Hop 32 analysis samples (5.3 ms). Cost ≈ W·τmax = 24k MAC per hop ≈ 4.5 MMAC/s.
- Scratch-sim results (bass-like synthetic tone with onset at t = 0):

| note | 30 ms | 40 ms | 50 ms | 60 ms+ |
|---|---|---|---|---|
| E1 +10 c, strong fund. | none | +46 c (W=100) | 0.0 c | 0.0 c |
| E1 +10 c, fund. at 0.3 | none | +24 c | 0.0 c | 0.0 c |
| A1 55 Hz | none | +0.1 c | +0.9 c | 0.1 c |
| G3 196 Hz | −0.4 c | −1.5 c | −0.9 c | −0.5 c |
  W + τmax samples of signal (~42 ms at the bottom) must exist before low E can be reported.
- MPM / McLeod NSDF (McLeod & Wyvill 2005) is the main alternative: normalised square
  difference, pick the first key maximum above k·max (k ≈ 0.9). Similar cost; often more
  robust on weak fundamentals. Either is fine; YIN is simpler to get right first.
- Even in poly mode, run the mono YIN for the tuner — it's more accurate than the bank early.

## Testing hints (engine seam only)

- Synthetic test tones should be bass-like: harmonics with amplitude ~1/h, **a weak
  fundamental variant** (fund × 0.3), slight decay, random phase. Pure sines over-flatter
  the tracker.
- Check: cents accuracy after 100 ms (±5 c), no voice for a tone below the gate, one voice
  (not two) for a single low E (catches 2nd-harmonic ghosts), two voices for E1+G#2 tenths
  by ~60 ms, a slide (log-linear chirp 41→82 Hz in 300 ms) keeps one voice id.
- Tracker is the riskiest piece: build an offline CLI that dumps per-hop
  partials/candidates/voices as CSV for a clip, and iterate on recorded clips.

## Pitfalls

- Don't pick notes by "loudest band". Use IF clusters + harmonic sieve.
- Don't average IF over a fixed short window; average over one f0 period.
- Don't run the bank at 48 kHz (precision + 8× CPU).
- Ghost voices from the 2nd harmonic are the #1 expected bug; cancellation fixes it.
- Ghost f0/2 voices are #2; the subharmonic check fixes it.
- A weak low-E fundamental is normal; the missing-fundamental candidate path handles it.
- Thirds below ~A1 will take ~100 ms to split into two voices. That's physics.
