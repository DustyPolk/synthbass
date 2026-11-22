# 02: Engine owns the patch

**What to build:** the engine library owns the patch record (every sound parameter, a version, defaults for missing fields, human-readable text encode/decode backward compatible with existing patch files) and can apply a whole patch in one call, smoothed like individual parameter changes and allocation-free. The app keeps patch storage (files, built-in patches, protection, last-used patch) and settings, but uses the engine's record. The noise gate stays outside the patch. The engine keeps no audio-backend or GUI dependencies; if encoding needs a dependency, keep it small and confined to the patch record.

**Blocked by:** 01 (Parameter table in one place)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] The patch record and its encode/decode live in the engine library; the app no longer defines its own
- [x] The engine applies a whole patch in one call; applying it mid-stream produces no click and allocates nothing (realtime-safety test covers it)
- [x] A whole patch applied at once yields the same output as the same values applied one by one
- [x] A patch file written by v1 loads identically (test with a v1-format fixture)
- [x] The app loads, saves and switches patches through the engine's record and the whole-patch call; the existing patch storage tests pass
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions
