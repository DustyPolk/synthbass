# Misc DSP building blocks (48 kHz, f32)

Everything here is per-sample, allocation-free, and sized at construction. Snippets marked
✓ were compiled and run in a scratch project.

## Coefficient helper

```rust
/// One-pole coefficient for time constant `t_s` (time to reach 63%).
/// 10–90% rise ≈ 2.2·t_s. 99% settle ≈ 4.6·t_s.
#[inline] fn coef(t_s: f32, fs: f32) -> f32 { if t_s <= 0.0 { 0.0 } else { (-1.0 / (t_s * fs)).exp() } }
// y = c*y + (1-c)*x  ==  y += (1-c)*(x - y)
```

## 1. Envelope follower (direct-layer envelope sensitivity; onset; string following)

```rust
struct Follower { y: f32, att: f32, rel: f32 }
impl Follower {
    fn new(att_s: f32, rel_s: f32, fs: f32) -> Self { Self { y: 0.0, att: coef(att_s, fs), rel: coef(rel_s, fs) } }
    #[inline] fn tick(&mut self, x: f32) -> f32 {
        let r = x.abs();                                   // peak; use x*x for power/RMS
        let c = if r > self.y { self.att } else { self.rel };
        self.y = r + c * (self.y - r);
        self.y
    }
}
```
- **Direct layer "filter opens as you dig in"** (story 11): follower on the dry signal,
  attack 1–3 ms, release 60–150 ms. Map in dB above the noise gate, then to octaves:
  ```
  env_db  = 20·log10(y + 1e-9) − gate_db            // 0 at the gate
  amt     = clamp(env_db / 40, 0, 1)                // 40 dB range
  fc      = base_fc · 2^(sens · 5 · amt)            // sens 0..1 → up to +5 octaves
  ```
  Log mapping makes the response even across soft/hard playing; a linear-amplitude mapping
  only opens on the loudest notes. Clamp fc to [10 Hz, 0.45·fs].
- Attack must be fast (~1 ms) so the filter opens within the same block as the pick (test:
  "direct layer responds within the same block as an onset").
- A pure peak follower ripples at the signal frequency at low E if release is short; release
  ≥ 60 ms keeps cutoff ripple small. For string following, the tracker's band magnitudes are
  already smooth (see dsp-polyphonic-tracking.md §7).

## 2. Waveshaper / drive ("square up", story 9)

```
y = tanh(drive · x) / tanh(drive)     drive = 10^(drive_db/20), 0..40 dB
```
Dividing by `tanh(drive)` keeps the peak level roughly constant as drive rises. At high
drive this is nearly a square wave at the played pitch, with the string's dynamics squashed.

**Aliasing is the real issue**: a hard-clipped 200 Hz tone has harmonics past 24 kHz falling
off only as 1/n; they fold back as inharmonic fizz below the filter cutoff, and the ladder
can't remove what's already in-band. Two fixes, in order of preference:
1. **First-order ADAA** (antiderivative anti-aliasing, Parker et al. DAFx-16; Bilbao et al.
   2017). Cheap, ~1 extra `ln`/`exp` per sample, adds half a sample of delay:
   ```rust
   // F(u) = ln(cosh(u)), computed without overflow
   #[inline] fn ln_cosh(u: f32) -> f32 { let a = u.abs(); a + (-2.0 * a).exp().ln_1p() - core::f32::consts::LN_2 }
   struct TanhAdaa { u1: f32, f1: f32 }
   impl TanhAdaa {
       #[inline] fn tick(&mut self, u: f32) -> f32 {      // u = drive * x
           let f = ln_cosh(u);
           let d = u - self.u1;
           let y = if d.abs() > 1e-4 { (f - self.f1) / d } else { (0.5 * (u + self.u1)).tanh() };
           self.u1 = u; self.f1 = f;
           y
       }
   }
   ```
   ✓ `ln_cosh` matches f64 `ln(cosh)` to 1e-7 for u ∈ [0,100]; 200 Hz sine at drive 100 gave
   max |y| = 1.0000007, no NaN/overshoot.
   f32 caveat: `f − f1` cancels badly at large |u| with small `d`; the 1e-4 threshold plus
   the fallback handles it. Use f64 inside if you hear crackle at extreme drive.
2. **2× oversampling** around the shaper (polyphase half-band FIR up/down, ~31 taps). More
   code, more CPU, better at very high drive. Do this only if ADAA isn't clean enough.

Other notes:
- Optional pre-filter: lowpass the dry signal at ~1–2 kHz before the shaper. Less fret noise
  and pick click gets squared, and less high-frequency content to alias.
- Chords through a shaper produce intermodulation. That's the expected fuzz-synth sound
  and doesn't break story 12 ("never glitches"), since nothing is detected.
- Symmetric tanh produces no DC. If an asymmetric shaper is added, put a DC blocker after
  it: `y = x − x1 + R·y1`, `R = 0.995`, which is about a 38 Hz high-pass at 48 kHz. Use
  R ≈ 0.9987 (≈10 Hz) to keep low E intact.
- **Gate the direct layer** with the global noise gate (smoothed, attack ~1 ms, release
  ~80 ms, ~3 dB hysteresis). At 40 dB drive, the Scarlett's −104 dBFS floor becomes about
  −64 dBFS of hiss, and real pickup hum is much higher. Without a gate, story 36 ("silence
  when not playing") fails.

## 3. Band-limited oscillators: PolyBLEP ✓

```rust
#[inline] fn poly_blep(t: f32, dt: f32) -> f32 {     // t = phase in [0,1), dt = f/fs
    if t < dt { let x = t / dt; 2.0 * x - x * x - 1.0 }
    else if t > 1.0 - dt { let x = (t - 1.0) / dt; x * x + 2.0 * x + 1.0 }
    else { 0.0 }
}
struct Osc { phase: f32 }
impl Osc {
    #[inline] fn advance(&mut self, dt: f32) { self.phase += dt; if self.phase >= 1.0 { self.phase -= 1.0; } }
    #[inline] fn saw(&mut self, dt: f32) -> f32 {
        let y = 2.0 * self.phase - 1.0 - poly_blep(self.phase, dt); self.advance(dt); y }
    #[inline] fn square(&mut self, dt: f32) -> f32 {
        let t = self.phase;
        let mut y = if t < 0.5 { 1.0 } else { -1.0 };
        y += poly_blep(t, dt);
        let t2 = if t + 0.5 >= 1.0 { t - 0.5 } else { t + 0.5 };
        y -= poly_blep(t2, dt);
        self.advance(dt); y }
}
```
Verified at 110 Hz: saw has mean 0.0000 and max |y| 0.995; square has mean 0.0000 and max
|y| 1.000. PolyBLEP alias rejection is modest, but bass fundamentals are low, so it's plenty here.
- `dt = f / fs` is recomputed every sample, which makes glide, vibrato and slides free.
  Keep `dt < 0.5`.
- **Osc 2**: `f2 = f · 2^(octave) · 2^(detune_cents / 1200)`, octave ∈ {−1, 0, +1}, detune
  0..±50 c. Give it its own phase, randomised at voice birth so detuned unisons don't start
  phase-cancelled.
- **Sub-oscillator**: a square at `f/2`. Keep it phase-locked to osc 1 by toggling its sign
  each time osc 1's phase wraps, then apply PolyBLEP at that edge using osc 1's `t` and `dt`
  (the sub's edge coincides with osc 1's wrap). Simpler alternative: a separate
  `Osc::square(dt/2)` reset together with osc 1 at birth. It drifts only by float rounding.
- Mix: `(osc1 + osc2_level·osc2 + sub_level·sub) / (1 + osc2_level + sub_level)` stays within
  ±1 into the ladder.
- Waveshape switch (saw ↔ square) on a sounding voice: crossfade over ~5 ms (see §5).

## 4. Glide (story 26)

One-pole in the **log-frequency** domain (semitones), per sample per voice:
```rust
pitch_st += (target_st - pitch_st) * (1.0 - coef(glide_s, fs));   // constant-time (Moog-like)
let f = 440.0 * ((pitch_st - 69.0) / 12.0).exp2();
```
Moog portamento is exponential like this. Gliding in Hz would sound wrong: too slow at the
top of the range and too fast at the bottom.

**Design choice to keep slides and vibrato exact (story 19).** Don't run the tracker's
continuous pitch updates through the user's glide time, or a 200 ms glide smears vibrato.
Use two layers:
- Continuous pitch (IF updates within a note): a short fixed smoother (~3 ms) removes hop
  steps and nothing else.
- User glide: applies only when the voice's target **jumps** (a re-pluck to a new note on
  the same voice, a steal, or a legato pitch change ≥ ~80 c with an onset). At voice birth,
  start `pitch_st` at the target (no glide from nowhere), unless you want "glide from the
  previous voice's pitch" for mono-style legato.

`exp2` per sample for 4 voices × 3 oscillators is cheap. Compute `f` once per voice per
sample and derive osc 2 and sub from it.

## 5. Click-free parameter smoothing (story 53)

```rust
struct Smoothed { cur: f32, target: f32, c: f32 }
impl Smoothed {
    fn new(v: f32, t_s: f32, fs: f32) -> Self { Self { cur: v, target: v, c: coef(t_s, fs) } }
    fn set(&mut self, v: f32) { self.target = v; }
    #[inline] fn next(&mut self) -> f32 { self.cur = self.target + self.c * (self.cur - self.target); self.cur }
    fn snap(&mut self) { self.cur = self.target; }   // on reset / first patch load
}
```
| parameter | smooth what | time |
|---|---|---|
| levels (direct, tracked, dry, output) | linear gain (convert dB → linear **before** smoothing) | 15–20 ms |
| cutoff | log2(Hz) (dsp-ladder-filter.md) | 20–30 ms |
| resonance | k | 10–20 ms |
| drive | dB or log gain | 20 ms |
| detune, sub level, env amount | value | 20 ms |
| waveshape, octave switch (discrete) | **crossfade** old→new output over 5–10 ms (run both for that window), or fade out 3 ms → switch → fade in 3 ms | 5–10 ms |

- Per-sample smoothing costs a few flops per parameter. Do it per sample, not per block, or a
  64-sample block gives a staircase at 750 Hz (audible zipper on gain).
- Patch load: send every parameter as a new target. Everything morphs over about 20 ms. No
  click, no special case.
- For the "no discontinuity above threshold" test, assert the max |Δy| between successive
  samples around the parameter change stays below the max |Δy| of the steady signal × a
  small factor.
- Readout values shown in the UI (meters) are smoothed in the UI, not the engine.

## 6. Safety limiter with a guaranteed ceiling (story 35) ✓

Instant attack, smooth release, then a hard clamp. Because `g ≤ ceil/|x|` on every sample,
`|x·g| ≤ ceil` holds by construction. The clamp guards against float edge cases.
```rust
struct Limiter { ceil: f32, g: f32, rel: f32 }          // rel = 1 - coef(0.1, fs)
impl Limiter {
    #[inline] fn tick(&mut self, x: f32) -> f32 {
        let x = if x.is_finite() { x } else { self.g = 0.0; 0.0 };  // NaN/Inf → silence
        let a = x.abs();
        let target = if a > self.ceil { self.ceil / a } else { 1.0 };
        self.g = if target < self.g { target } else { self.g + (target - self.g) * self.rel };
        (x * self.g).clamp(-self.ceil, self.ceil)
    }
}
```
Verified: input at +26 dB (20× full scale) bursting on and off at 700 Hz, with a NaN and a
1e30 spike injected. Worst |y| = 0.891 with ceiling 0.891 (−1 dBFS).
- Ceiling: −1 dBFS (0.891). Release ≥ 80–100 ms. A shorter release lets the gain modulate
  within a low-E cycle (24 ms) and audibly distorts the bass.
- Instant attack distorts the first cycle of an overload. That's acceptable for a safety
  device; it should rarely engage with sane patches. A 1–2 ms lookahead would be cleaner but
  adds latency, and the spec says non-lookahead.
- Optional soft knee: `x' = ceil·tanh(x/ceil)` for |x| > 0.5·ceil before the limiter. Keep
  the limiter after it for the guarantee.
- If NaN shows up, also reset the engine's filter states (a NaN in a ladder state stays NaN
  forever). Do it on the next block, from a flag.
- Process mono, then copy to both outputs after limiting.

## 7. Denormals ✓

Measured on this machine (x86_64): a decaying-state loop took **337 ms without FTZ/DAZ and
5.3 ms with it (63× slower)**. MXCSR at thread start was `0x1fb2`, so FTZ (bit 15, 0x8000)
and DAZ (bit 6, 0x40) are **off by default**. Ladder states, resonators, followers and
smoothers all decay into the denormal range after a note ends.

Set FTZ+DAZ at the top of every `Engine::process` call and restore at the end. A RAII guard
works; two instructions per block. Then offline tests and live audio behave the same, and
the engine doesn't depend on which thread calls it.
```rust
struct DenormalGuard { old: u32 }
impl DenormalGuard {
    #[inline] fn new() -> Self {
        let mut old: u32 = 0;
        #[cfg(target_arch = "x86_64")]
        unsafe {
            core::arch::asm!("stmxcsr [{}]", in(reg) &mut old, options(nostack));
            let new = old | 0x8040;                        // FTZ | DAZ
            core::arch::asm!("ldmxcsr [{}]", in(reg) &new, options(nostack));
        }
        Self { old }
    }
}
impl Drop for DenormalGuard {
    #[inline] fn drop(&mut self) {
        #[cfg(target_arch = "x86_64")]
        unsafe { core::arch::asm!("ldmxcsr [{}]", in(reg) &self.old, options(nostack)); }
    }
}
// ✓ MXCSR 0x1fb2 → 0x9ff2 inside the guard → 0x1fb2 after drop.
// aarch64: FPCR bit 24 (FZ) via `mrs {}, fpcr` / `msr fpcr, {}`.
```
- `std::arch::x86_64::_mm_setcsr` / `_mm_getcsr` are **deprecated** in current Rust. The
  docs say changing MXCSR is technically UB, because the compiler assumes the default FP
  environment. Inline asm (above) avoids the deprecation warning but not that caveat. In
  practice FTZ/DAZ is standard in Rust audio code (nih-plug does the same).
- Crate alternative: `no_denormals = "0.3"` → `unsafe { no_denormals::no_denormals(|| { .. }) }`.
  It uses the deprecated intrinsics on x86 and supports aarch64.
- Portable belt and braces (no unsafe): when a voice goes idle, zero its filter/oscillator
  states instead of letting them decay, and skip processing idle voices entirely.
- `assert_no_alloc` (1.1.2) can wrap `process` in debug builds to catch accidental
  allocation on the audio thread.
