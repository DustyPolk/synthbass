# 01: Parameter table in one place

**What to build:** each sound parameter's range, default, label, display format and patch key are defined once, and everything that needs those facts reads them from there: the app's patch field mapping, the window's knob formatting, and range/default lookups. Looking up a parameter by its index is constant-time. Nothing changes for the bassist; adding a parameter afterwards is one edit instead of six. The engine's per-layer dispatch on parameters may stay, since each layer reacts differently.

**Blocked by:** None (can start immediately)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] One definition gives every parameter's range, default, label, display format and patch key
- [x] The app's patch encode/decode and the window's knob formatting use it; no other hand-written per-parameter lists remain outside the engine's layer dispatch
- [x] Parameter index lookup is constant-time (no linear search of the parameter list)
- [x] A test proves every parameter has metadata and that its default lies within its range
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions
