# Moog-style ladder filter (per voice + direct layer), 48 kHz

**Recommendation:** Zavalishin's zero-delay-feedback (ZDF / TPT) 4-pole ladder with a single
`tanh` at the input stage, feedback solved linearly ("cheap nonlinear ZDF"). One `tan` per
cutoff update, one `tanh` per sample, ~25 flops/sample. Five instances (4 voices + direct
layer) cost nothing measurable.

Sources: V. Zavalishin, *The Art of VA Filter Design* (rev. 2.1.x), ch. 3 (TPT one-pole),
ch. 5 (ladder), ch. 6 (nonlinear). Huovilainen, "Non-linear digital implementation of the
Moog ladder filter" (DAFx-04). Välimäki & Huovilainen, "Oscillator and filter algorithms for
virtual analog synthesis" (CMJ 2006).

## Verified numerically (scratch sim, f32, fs = 48 kHz, code below)

| Test | Result |
|---|---|
| Impulse, fc = 500 Hz, `comp = 0`, k = 3.8 / 3.95 | decays (no oscillation) |
| k = 4.0 | **self-oscillation threshold exactly at k = 4**, at the set cutoff (500 Hz) — TPT keeps the analog threshold and tuning |
| k = 4.05 / 4.2 / 6.0 | sustained oscillation at 500 Hz, amplitude bounded by tanh (≈0.05 / 0.1 / 0.26 peak) |
| ±4.0 square at 41 Hz into k = 4.5, cutoff swept 30 Hz–30 kHz (clamped) for 10 s | finite, max \|y\| = 1.32 |
| Passband (40 Hz, fc = 2 kHz) gain at k = 0 / 2 / 3.5, `comp=0` | 0 / −9.5 / −13.1 dB (the classic Moog bass loss) |
| same, `comp=0.5` | 0 / −3.5 / −4.3 dB |
| same, `comp=1.0` | 0 / 0 / 0 dB |

## Equations

Prewarp: `g = tan(π · fc / fs)`, `G = g / (1 + g)`. Clamp `fc ≤ 0.45·fs` (tan blows up
toward fs/2).

TPT one-pole lowpass with state `s` (trapezoidal integrator):
```
v = (x − s)·G ;  y = v + s ;  s = y + v
```
Equivalently `y = G·x + s/(1+g)`. Four in series; the output of the cascade is affine in the
input `u`: `y4 = G⁴·u + S` with
```
S = (G³·s1 + G²·s2 + G·s3 + s4) / (1 + g)
```
Feedback `u = x − k·y4` → solve the zero-delay loop in closed form:
```
u = (x − k·S) / (1 + k·G⁴)
```
Then saturate `u ← tanh(u)` and run the four stages. `k ∈ [0, 4]` maps resonance 0..1;
`k = 4` is self-oscillation.

```rust
pub struct Ladder { s: [f32; 4], g: f32, big_g: f32, k: f32, comp: f32 }

impl Ladder {
    pub fn set_cutoff(&mut self, fc: f32, fs: f32) {
        let g = (core::f32::consts::PI * fc.clamp(10.0, 0.45 * fs) / fs).tan();
        self.g = g;
        self.big_g = g / (1.0 + g);
    }
    pub fn set_resonance(&mut self, r01: f32) { self.k = 4.0 * r01.clamp(0.0, 1.0); }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let gg = self.big_g;
        let b = 1.0 / (1.0 + self.g);       // or store 1-G (identical: 1/(1+g) == 1-G)
        let g2 = gg * gg;
        let sig = (g2 * gg * self.s[0] + g2 * self.s[1] + gg * self.s[2] + self.s[3]) * b;
        let x = x * (1.0 + self.comp * self.k);           // resonance compensation
        let mut u = ((x - self.k * sig) / (1.0 + self.k * g2 * g2)).tanh();
        for s in &mut self.s {
            let v = (u - *s) * gg;
            let y = v + *s;
            *s = y + v;
            u = y;
        }
        u
    }
}
```
(Note `1/(1+g) == 1 − G`; store it to avoid the divide.)

## Nonlinearity placement

- **One tanh on the input/feedback sum** (above) gives the Moog "the louder you hit it the
  more resonance gets squashed" behaviour and bounds self-oscillation. This is what we
  verified. It's not an exact solution of the nonlinear ZDF (the feedback is solved
  linearly, then saturated) but it is stable and sounds right.
- More faithful options, only if the sound demands it:
  - tanh in every stage (Huovilainen): `y_i += 2·Vt·g·(tanh(y_{i-1}/2Vt) − tanh(y_i/2Vt))`.
    Uses a unit delay in the feedback (not ZDF) so it needs a half-sample phase correction
    and **2× oversampling**, 5 tanh/sample. Classic, but more work.
  - Full nonlinear ZDF: Newton-Raphson on `u = tanh(x − k·y4(u))`, 1–3 iterations, starting
    from the linear solution. 2–4× cost. Not needed for v1.
- Input level matters with tanh: scale so a hard-played bass sits around 0.3–1.0 into the
  filter. Drive the direct layer *before* the filter with the separate waveshaper
  (dsp-misc.md); keep the filter's own tanh as gentle colour.
- Use a fast tanh: `x·(27 + x²)/(27 + 9x²)` clamped to |x| ≤ 3 is a Padé approximation good to
  ~2% and monotone; or `f32::tanh` (libm, ~20 ns — 5 per sample at 48 kHz is ≈5 µs/ms,
  negligible). Start with `f32::tanh`; optimise only if profiling says so.

## Resonance compensation

Ladder DC gain is `1/(1+k)`, so cranking resonance thins the bass. Multiply the input by
`(1 + comp·k)`:
- `comp = 1.0` → flat passband (table above). Sounds "modern", loudest at high res.
- `comp = 0.5` → ~−4 dB at high resonance; keeps some of the vintage thinning. **Suggested
  default: 0.5**, fixed (not a patch parameter).
Because the gain rises with k, the limiter after the mix (dsp-misc.md) is the safety net.

## Stability at max resonance

- Linear part: TPT is unconditionally stable for k < 4 at any fc < fs/2 (poles inside unit
  circle). At k ≥ 4 it oscillates; tanh bounds the amplitude. Verified up to k = 6.
- Cap the UI resonance at k = 4.0 (r01 = 1.0). Allowing slightly above (≈4.1) gives a
  reliable self-oscillating "squelch" — optional.
- Clamp `fc` to `[10 Hz, 0.45·fs]` *after* adding envelope modulation, never before.
- NaN guard is cheap insurance: if `!y.is_finite()` reset `s = [0; 4]` (should never fire).

## Parameter smoothing & modulation

- Cutoff is modulated per sample (filter envelope, direct-layer envelope follower), so
  compute `g = tan(...)` per sample or per 8–16-sample sub-block. Per sample is fine
  (5 tan/sample).
- Smooth in the **log-frequency** domain: keep `log2(fc)` as the smoothed quantity (one-pole,
  ~10–20 ms), add envelope modulation in octaves, then `fc = 2^(..)`. Linear-Hz smoothing
  makes low-cutoff sweeps sound sluggish and high ones jumpy.
- Envelope amount in octaves is the natural unit: `fc = base_fc · 2^(amount_oct · env)`,
  amount 0..6 octaves.
- Resonance: smooth `k` linearly (~10 ms). Sudden k jumps near 4 can produce a burst.
- TPT tolerates audio-rate cutoff modulation without the zipper/instability of direct-form
  biquads — another reason to use it.

## Denormals

The four states decay exponentially toward 0 after a note ends; in f32 they pass through
the denormal range (< 1.18e‑38) and can make each op 10–100× slower on x86. In the sim, a
decayed filter's tail hit exactly 0, so they do get there.
- Primary fix: set FTZ/DAZ once on the audio thread (see dsp-misc.md, "Denormals").
- Belt and braces: the input is never exactly silent anyway (Scarlett noise floor
  ≈ −104 dBFS measured), but tracked voices are fed by oscillators that stop, so don't rely
  on that. Optionally add `1e-20` DC/alternating offset to the input of each filter.
- If a voice is idle (loudness 0 and envelope done), skip `tick` and zero the state.

## Per-voice notes

- Reset state to zero when a voice is (re)assigned to a new note *only if* the voice was
  silent; for a steal of a sounding voice, keep the state (avoids clicks) and let the filter
  envelope restart.
- Filter envelope: attack ≈ 0–2 ms (instant "bwow"), exponential decay controlled by the
  decay knob (30 ms – 2 s), amount scaled by pick sensitivity × onset strength (see
  dsp-polyphonic-tracking.md for onset strength).
- Key tracking (optional, small): `base_fc · (f_voice / 110 Hz)^kt` with kt ≈ 0.3–0.5 keeps
  high notes from sounding dull relative to low ones. Not in the spec's parameter list; if
  added, hard-code it.

## Aliasing

tanh on a filtered bass signal aliases little at 48 kHz; the oscillators (PolyBLEP) and the
direct layer waveshaper are bigger aliasing sources (dsp-misc.md). Don't oversample the
filter in v1.
