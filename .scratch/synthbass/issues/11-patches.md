# 11: Patches

**What to build:** every sound parameter (direct layer, tracked layer, mix) is saved together as a named, versioned patch, one human-readable file per patch in a patches directory under the user's config directory. The window gains a patch picker with save, save-as, and delete, plus a marker when settings differ from the loaded patch. About six built-in Moog-style patches ship embedded in the app and can't be overwritten or deleted. The app reloads the last-used patch at startup and remembers the noise gate threshold (which is not part of any patch) in the settings file.

**Blocked by:** 04 (Direct layer), 08 (Slides, bends and glide), 09 (Moog voice), 10 (Ports and resilience)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [ ] Patch record covers every sound parameter and carries a version; missing or unknown fields fall back to defaults
- [ ] Tests: save then load round-trips every parameter; every built-in patch loads; a corrupt file is reported and skipped without panicking; a file missing newer fields loads with defaults; built-in patches cannot be overwritten or deleted
- [ ] Picker lists built-in and user patches; save, save-as, delete work; unsaved-changes marker appears after any tweak
- [ ] Last-used patch and noise gate threshold persist across launches; first launch falls back to a built-in default
- [ ] About six built-in patches covering classic Moog-style sounds (e.g. pluck, fat sub, squelchy resonant, detuned saw, soft round, octave-up lead)
- [ ] Verified by playing each built-in patch
