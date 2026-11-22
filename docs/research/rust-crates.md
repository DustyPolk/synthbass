# Rust crates for synthbass (verified 2026-11-21)

Toolchain on this machine: `rustc 1.99.1`, `cargo 1.99.1`, edition 2024.
Everything below was compiled against the listed versions in a scratch project
(not guessed). Versions from the crates.io API on 2026-11-21.

| Crate | Version | Use | Notes |
|---|---|---|---|
| `jack` | **0.14.0** (2026-11-07) | audio I/O | jack-sys 0.7.0. MSRV 1.88. Active (RustAudio). |
| `eframe` / `egui` | **0.37.0** (2026-10-15) | window | **`App::update` is gone — implement `App::ui`** (see below). |
| `rtrb` | **0.4.1** | UI→audio param queue | SPSC, wait-free, no alloc after `new`. |
| `triple_buffer` | **9.0.1** | audio→UI readout | latest-value snapshot, wait-free. |
| `atomic_float` | 1.1.0 (2024-08) | single scalars (gate) | Stable/finished, not abandoned; tiny crate. |
| `hound` | 3.5.1 (2023-09) | WAV read/write | Repo still active (pushes 2026); no release needed. |
| `serde` | 1.0.229 | derive | `features = ["derive"]` |
| `toml` | **1.2.2** | patch/settings files | toml went 0.9 → 1.x; API `toml::from_str` / `to_string_pretty` unchanged. |
| `dirs` | **7.0.0** | `~/.config` | `dirs::config_dir()` → `Some("/home/dustin/.config")` here. |
| `assert_no_alloc` | 1.1.2 (2021) | optional debug | Old but works; global allocator that aborts on alloc inside a guarded scope. |
| `no_denormals` | 0.3.0 | optional | Sets FTZ/DAZ in a scope. Or do it by hand (see dsp-misc.md). |

Suggested `Cargo.toml` split (engine has no backend/GUI deps per ADR 0003):

```toml
# crates/engine/Cargo.toml   — nothing audio-backend or GUI related
[dependencies]
serde = { version = "1", features = ["derive"] }   # Patch derives
[dev-dependencies]
hound = "3.5"                                        # load test clips

# crates/app/Cargo.toml
[dependencies]  # engine (path), jack = "0.14", eframe = "0.37", egui = "0.37", rtrb = "0.4",
                # triple_buffer = "9", atomic_float = "1", hound = "3.5", serde (derive),
                # toml = "1", dirs = "7"
```

---

## 1. JACK via PipeWire

### This machine

- `/usr/lib/libjack.so.0` is owned by **`pipewire-jack 1.6.9`** — i.e. the system libjack
  *is* PipeWire's. A binary linked/dlopen'ing `libjack.so.0` talks to PipeWire directly.
  **`pw-jack` is not needed** (verified: the probe ran identically with and without it).
  `pw-jack` only matters on distros where the real jackd libjack is the default.
- `jack` 0.14 default features = `["dynamic_loading", "log"]`: libjack is **dlopen'ed at
  runtime** (`libjack.so.0`), so no link-time dependency and no `-dev` package needed to build.
  If libjack is missing, `Client::new` returns `Error::LibraryError` (no panic) → show the
  "audio server not reachable" message (story 6).
- `jack_lsp` is **not installed**; `pw-link -o/-i` shows PipeWire names (≠ JACK names).
  Use `client.ports(..)` (the names below came from that).
- **Server default quantum is 1024** (`clock.quantum=1024`, `min-quantum=32`, rate locked
  to 48000). Without action the client gets **1024-frame buffers** (21 ms). See "Buffer size".

### Real Scarlett port names (JACK names, from `client.ports()` on this machine)

Card profile currently active: `output:analog-surround-21+input:analog-surround-21`
(PipeWire default, not "Pro Audio"). JACK port names are `"<node.description>:<port>"`:

```
capture (physical outputs from JACK's POV — our SOURCES):
  Scarlett 4i4 4th Gen Analog Surround 2.1:capture_FL    <- Scarlett input 1
  Scarlett 4i4 4th Gen Analog Surround 2.1:capture_FR    <- input 2
  Scarlett 4i4 4th Gen Analog Surround 2.1:capture_LFE   <- input 3
playback (physical inputs from JACK's POV — our SINKS):
  Scarlett 4i4 4th Gen Analog Surround 2.1:playback_FL   <- output 1
  Scarlett 4i4 4th Gen Analog Surround 2.1:playback_FR   <- output 2
  Scarlett 4i4 4th Gen Analog Surround 2.1:playback_LFE
  (also ...:monitor_FL/FR/LFE — monitor taps, NOT physical; ignore)
Other devices present: "Built-in Audio Analog Stereo:capture_FL" etc.,
  "Navi 21/23 HDMI/DP Audio Controller Digital Stereo (HDMI 5):playback_FL"
```

**Pitfalls**
- The name embeds the **profile** ("Analog Surround 2.1"). If the user switches the card to
  "Pro Audio", ports become `...:capture_AUX0..` / `playback_AUX0..` with a different client
  name. So **don't hard-code full names**. Default-port picker should be:
  1. physical audio ports (`PortFlags::IS_PHYSICAL | IS_OUTPUT` for sources,
     `IS_PHYSICAL | IS_INPUT` for sinks) whose name contains `"Scarlett"`;
  2. sort; source = first (`capture_FL` / `capture_AUX0`), sinks = first two
     (`playback_FL`,`playback_FR` / `AUX0`,`AUX1`);
  3. persist the chosen *full* names in settings; on startup prefer saved names if present,
     else fall back to the heuristic.
- `client.ports(name_regex, type_regex, flags)` takes **regexes** (`Some("Scarlett")` works).
  Audio type string is `"32 bit float mono audio"` (`jack::jack_sys::FLOAT_MONO_AUDIO`).
- Port aliases came back empty under PipeWire; don't rely on them.

### Buffer size (story 5: 64–128 frames)

Verified on this machine (probe measured `ps.n_frames()` while running):

| How | Result |
|---|---|
| nothing | 1024 |
| `PIPEWIRE_LATENCY=128/48000` env | **128** |
| `PIPEWIRE_QUANTUM=128/48000` env | 128 (forces; heavier hammer) |
| `std::env::set_var("PIPEWIRE_LATENCY","64/48000")` **in `main` before `Client::new`** | **64** |

Recommended: in `main`, before creating the client, if `PIPEWIRE_LATENCY` is unset, set it
to `"128/48000"` (edition 2024: `unsafe { std::env::set_var(..) }` — fine at the top of `main`
before any threads). `PIPEWIRE_LATENCY` is a *request* (`node.latency`); PipeWire picks the
smallest requested latency across the graph, so other desktop audio keeps working (story 4).
Do **not** call `client.set_buffer_size()` — PipeWire treats that as a global quantum change.
The engine must still handle any block size ≤ max (PipeWire can change quantum at runtime
→ `ProcessHandler::buffer_size` callback fires; quantum-limit here is 8192, so size the
engine for 8192 max or chunk larger blocks internally into ≤ max pieces).

### API shapes (jack 0.14.0)

```rust
// Client
let (client, status) = jack::Client::new("synthbass", jack::ClientOptions::NO_START_SERVER)?;
client.sample_rate();  // u32 (48000 here)
client.buffer_size();  // Frames (u32)
let inp  = client.register_port("in",    jack::AudioIn::default())?;
let outl = client.register_port("out_l", jack::AudioOut::default())?;
let outr = client.register_port("out_r", jack::AudioOut::default())?;
let src_names: Vec<String> = client.ports(Some("Scarlett"), Some("audio"),
    jack::PortFlags::IS_PHYSICAL | jack::PortFlags::IS_OUTPUT);
// Activate. N: 'static + Send + Sync + NotificationHandler ; P: 'static + Send + ProcessHandler
let active: jack::AsyncClient<Notif, Proc> = client.activate_async(Notif{..}, Proc{..})?;
// Connect AFTER activation, from a normal (non-callback) thread:
active.as_client().connect_ports_by_name(
    "Scarlett 4i4 4th Gen Analog Surround 2.1:capture_FL", "synthbass:in")?;
active.as_client().connect_ports_by_name("synthbass:out_l",
    "Scarlett 4i4 4th Gen Analog Surround 2.1:playback_FL")?;
// Re-routing: disconnect our own port (Port<..> or by name), then connect new.
active.as_client().disconnect_ports_by_name(src, dst)?;
active.as_client().port_by_name(name) -> Option<Port<Unowned>>;  // existence check
active.deactivate()? -> (Client, N, P);  // gets the handlers back
```

`connect_ports_by_name` returns `Err(Error::PortAlreadyConnected(..))` on EEXIST — treat as Ok.
`Error` is `#[non_exhaustive]` in 0.14 → wildcard arm needed in matches.
Our own port's full name is `"<client.name()>:<short>"`; use `port.name()?` rather than
assuming the client name (PipeWire may uniquify it if another `synthbass` is running unless
`USE_EXACT_NAME`).

Process handler — prefer a plain struct over the closure helper so the engine is owned:

```rust
struct Proc { inp: jack::Port<jack::AudioIn>, l: jack::Port<jack::AudioOut>,
              r: jack::Port<jack::AudioOut>, engine: engine::Engine,
              params: rtrb::Consumer<ParamMsg>, readout: triple_buffer::Input<Readout> }
impl jack::ProcessHandler for Proc {
    fn process(&mut self, _: &jack::Client, ps: &jack::ProcessScope) -> jack::Control {
        while let Ok(m) = self.params.pop() { self.engine.apply(m); }
        let input = self.inp.as_slice(ps);          // &[f32], len == ps.n_frames()
        let l = self.l.as_mut_slice(ps);
        self.engine.process(input, l);              // mono
        self.r.as_mut_slice(ps).copy_from_slice(l); // disjoint fields: compiles (verified)
        self.readout.write(self.engine.readout());  // Copy type, no alloc
        jack::Control::Continue
    }
    fn buffer_size(&mut self, _: &jack::Client, n: jack::Frames) -> jack::Control {
        // Called off the RT path. Must not exceed engine max block or chunk in process().
        jack::Control::Continue
    }
}
```
Closure alternative: `jack::contrib::ClosureProcessHandler::new(|c, ps| ..)` or
`ClosureProcessHandler::with_state(state, process_cb, buffer_cb)`.
Feature `controller` adds `jack::contrib::Controller` (rtrb-backed command/notification
channels) — usable, but our own rtrb + triple_buffer is clearer.

Notification handler (all methods have defaults; runs on a JACK **non-RT** thread):

```rust
trait NotificationHandler: Send {
  fn thread_init(&self, _: &Client) {}
  unsafe fn shutdown(&mut self, status: ClientStatus, reason: &str) {} // signal-safety rules!
  fn sample_rate(&mut self, _: &Client, srate: Frames) -> Control
  fn client_registration(&mut self, _: &Client, name: &str, is_registered: bool) {}
  fn port_registration(&mut self, _: &Client, port_id: PortId, is_registered: bool) {}
  fn ports_connected(&mut self, _: &Client, a: PortId, b: PortId, connected: bool) {}
  fn graph_reorder(&mut self, _: &Client) -> Control
  fn xrun(&mut self, _: &Client) -> Control
}
```
- `activate_async` requires `N: Sync` → put state in atomics / `Mutex` / channel senders.
- **Never connect/disconnect or register ports from inside a callback** (JACK API rule;
  deadlocks). Pattern: callbacks only set an `AtomicBool` "graph changed" / bump an
  `AtomicU32` xrun counter / `send` on a `std::sync::mpsc::Sender` and call
  `egui_ctx.request_repaint()`. The UI thread then re-lists ports and reconnects (story 7).
- `shutdown`: server went away. Only set an atomic flag. UI shows "audio server lost" and
  offers Reconnect (drop the old `AsyncClient` — `deactivate` may fail — make a new client).
- Unplugging the Scarlett: expect `client_registration(.., false)` for the Scarlett node and
  `port_registration(id, false)` for its ports; our ports stay registered, connections vanish.
  On replug, the ports come back (same names if the profile is the same) → auto-reconnect
  to the saved names. (Not tested by unplugging; verify by playing.)

Realtime: user has rtprio 98; pipewire-jack runs `process` on an RT data thread. Nothing to configure.

---

## 2. eframe / egui 0.37

**Breaking vs. older tutorials:** `eframe::App` now has required `fn ui(&mut self, ui: &mut
egui::Ui, frame: &mut eframe::Frame)` and optional `fn logic(&mut self, ctx, frame)`.
Panels take a `&mut Ui`: `CentralPanel::default().show(ui, ..)`; `TopBottomPanel`/`SidePanel`
are now `egui::Panel::top("id")` / `Panel::left("id")` / bottom/right, `.show(ui, ..)`.
(`show_inside` is deprecated → renamed `show`.) Compiled and checked:

```rust
impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _f: &mut eframe::Frame) {
        let r = *self.readout.read();                       // triple_buffer::Output
        egui::Panel::top("status").show(ui, |ui| { ui.label(format!("in {:.1} dB", r.in_db)); });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.columns(3, |cols| {                          // direct | tracked | mix
                cols[0].heading("Direct");
                if cols[0].add(egui::Slider::new(&mut self.cutoff, 20.0..=20_000.0)
                        .logarithmic(true).text("Cutoff")).changed() {
                    let _ = self.tx.push(ParamMsg::DirectCutoff(self.cutoff));
                }
                cols[0].add(egui::DragValue::new(&mut self.gate_db).range(-90.0..=0.0).suffix(" dB"));
            });
            egui::ComboBox::from_label("Input")             // or from_id_salt("in_port")
                .selected_text(self.ports[self.sel].as_str())
                .show_ui(ui, |ui| for (i, p) in self.ports.iter().enumerate() {
                    ui.selectable_value(&mut self.sel, i, p);
                });
        });
        // keep meters/tuner live while audio runs (egui is reactive by default):
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
}
fn main() -> eframe::Result {
    let opts = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 640.0]),
        ..Default::default() };
    eframe::run_native("Synthbass", opts, Box::new(|cc| {
        cc.egui_ctx.set_theme(egui::Theme::Dark);           // story 52
        Ok(Box::new(App::new(cc)))
    }))
}
```

- No built-in rotary knob. Use `Slider` (fine, story 52 says plain panel) or paint a knob via
  `ui.allocate_response(size, Sense::drag())` + `painter().circle_stroke/line_segment`,
  mapping `response.drag_delta().y` to value. Sliders are the pragmatic v1.
- `Slider::new(&mut v, range).logarithmic(true).text("..").suffix(" Hz")`;
  `.changed()` fires per drag step → push each change; the engine smooths.
- Detect "unsaved tweaks" (story 47) by `current_patch != loaded_patch` (derive `PartialEq`).
- While minimized, eframe skips `ui`; audio is unaffected because it never waits on UI
  (story 55). The rtrb queue may fill if the UI pushes but nothing pops — not possible here
  since the audio thread pops; if *audio* stops, `push` returns `Err(Full)` → ignore.
- Default renderer is **wgpu** (first debug build of the probe: ~30 s on 16+ cores). If build
  time or GPU issues bite: `eframe = { version = "0.37", default-features = false, features =
  ["default_fonts", "glow", "wayland", "x11"] }` + `renderer: eframe::Renderer::Glow`.
- Wayland session here (Hyprland/omarchy); `wayland` + `x11` are default features.

---

## 3. UI ↔ audio communication (recommendation)

**Parameter changes UI → audio: `rtrb` SPSC queue of a small `Copy` enum.**
```rust
#[derive(Clone, Copy)] enum ParamMsg { Set(ParamId, f32), LoadPatch(/* see below */) }
let (mut tx, mut rx) = rtrb::RingBuffer::<ParamMsg>::new(1024);
tx.push(msg)            // UI: Result<(), PushError<T>>; ignore Err(Full)
while let Ok(m) = rx.pop() { .. }   // audio, at top of process()
```
- Whole-patch load: send N `Set` messages (one per param, ~40) in a burst — queue of 1024
  covers it, no allocation. Avoid sending a `Box<Patch>` (drop on audio thread = free()).
- The engine applies `Set` to smoothed targets (see dsp-misc.md), so bursts don't click.
- Noise gate (global, not in patch): same queue with its own `ParamId`, or one
  `Arc<atomic_float::AtomicF32>` read each block (`load(Relaxed)`). Pick one; the queue
  keeps one path.

**Readout audio → UI: `triple_buffer` of a fixed-size `Copy` struct.**
```rust
#[derive(Clone, Copy, Default)]
struct Readout { input_peak: f32, tuner: Option<(u8 /*midi*/, f32 /*cents*/)>,
                 voices: [Option<VoiceReadout>; 4] }
let (mut w, mut r) = triple_buffer::triple_buffer(&Readout::default());
w.write(engine.readout());   // audio: once per block; wait-free, no alloc for Copy T
let latest = *r.read();      // UI: newest snapshot; read() takes &mut self, so copy it out
                             // once per frame (two r.read() in one expr = borrow error)
```
Why not rtrb for readout: UI drains at 30 Hz, audio produces at 375–750 Hz; a queue fills or
needs draining loops. Triple buffer gives "latest value" semantics, which is what meters want.
Peak meters: accumulate max-abs in the engine and reset when the snapshot is taken, or keep a
peak-hold with decay inside the engine so dropped intermediate snapshots don't hide clips.

Also checked (compiled + ran live against the Scarlett): a `Proc` struct holding the three
jack ports, `rtrb::Consumer<Msg>` and `triple_buffer::Input<Readout>` satisfies
`ProcessHandler`'s `Send` bound; a `Notif { xruns: AtomicU32 }` satisfies `Send + Sync`.
Measured idle noise on Scarlett input 1 (nothing plugged/playing, current gain): block peak
≈ 6.6e-6 (≈ −104 dBFS). A noise-gate default around −60 dBFS leaves lots of headroom; real
pickup hum will be far higher — let the bassist set it.

---

## 4. hound (WAV)

```rust
let spec = hound::WavSpec { channels: 1, sample_rate: 48_000, bits_per_sample: 32,
                            sample_format: hound::SampleFormat::Float };
let mut w = hound::WavWriter::create(path, spec)?;  // BufWriter<File>
w.write_sample(x)?;   // per sample; f32
w.finalize()?;        // MUST call (writes header sizes); Drop also tries but swallows errors
let mut r = hound::WavReader::open(path)?;
r.spec(); r.duration();  // frames
let xs: Vec<f32> = r.samples::<f32>().collect::<Result<_, _>>()?;
// 16/24-bit int files: r.samples::<i32>() then scale by 1/(1 << (bits-1)) as f32
```
Recording command (story 56): never write the file from the JACK callback. Process callback
pushes samples into an `rtrb::RingBuffer<f32>` sized ~10 s (480k); a writer thread pops in
chunks (`rx.read_chunk(n)` / `pop`) and calls `write_sample`. Count dropped samples on
`push` failure and report them. Ctrl-C / Enter → stop → `finalize()`.
Store test clips as 32-bit float mono 48 kHz (no conversion ambiguity).

---

## 5. serde + toml + dirs (patches & settings)

```rust
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]                 // missing fields -> Default (story 50: older files load)
struct Patch { version: u32, name: String, direct: Direct, tracked: Tracked, mix: Mix }
impl Default for Patch { .. }     // each nested struct also #[serde(default)] + Default
let p: Patch = toml::from_str(&text)?;          // toml::de::Error on corrupt -> skip + message
let text = toml::to_string_pretty(&p)?;
```
Verified: a file with only `name` and `cutoff_hz` loaded with the other fields defaulted.
- Unknown fields are ignored by default (don't add `deny_unknown_fields`) → newer files load
  in older builds too.
- Enums (waveshape, octave) serialize as strings: `#[serde(rename_all = "lowercase")]`.
- Paths: `dirs::config_dir()?.join("synthbass")` → `~/.config/synthbass/`
  - `patches/<sanitized-name>.toml` (one per user patch)
  - `settings.toml` → `last_patch`, `noise_gate_db`, `input_port`, `output_ports`
- Built-ins: `include_str!("../patches/builtin/fat_bass.toml")` in a `const` array; parse at
  startup; mark read-only so save/delete refuse them (story 45).
- Write atomically: write `name.toml.tmp` then `std::fs::rename` → no half-written patch.
- Patch storage belongs in the app crate (file I/O), but the `Patch` struct and its defaults
  can live in the engine so tests share them; serde in the engine is not a backend/GUI dep.
