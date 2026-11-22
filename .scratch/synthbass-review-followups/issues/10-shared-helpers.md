# 10: Shared helpers

**What to build:** duplicated logic lives in one place. One helper computes one-pole smoothing coefficients from a time constant (replacing about nine copies). One note-number/frequency conversion pair and one note-name-plus-cents formatter are public from the engine and used by both its tuner and the window. One atomic-file-write helper serves patch storage and settings. The recording command chooses the Scarlett port with the app's matching logic, including its sort. The window's selector widgets share one generic widget, and the engine's fixed-capacity lists share one small generic container.

**Blocked by:** 09 (Glossary names and renames)

**Status:** done

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, `.scratch/synthbass-review-followups/spec.md` and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [x] Each helper exists once and every former copy calls it
- [x] Test: the tuner and the window format the same pitch identically
- [x] Test: the recording command and the app pick the same default port from the same port list
- [x] Pure refactor: no behaviour change; the realtime-safety test still passes
- [x] All existing tests pass, including the recorded-clip suites, with no new exemptions
