# 10: Ports and resilience

**What to build:** the bassist can pick input and output ports from dropdowns, sees a clear message when the JACK server is unreachable or the Scarlett is absent, and can recover after unplugging and replugging the interface without restarting the app. A small app settings file remembers the chosen ports across launches. The audio keeps running smoothly even when the window is busy or minimised.

**Blocked by:** 01 (Dry signal passthrough)

**Status:** ready-for-agent

**Context budget:** finish within one fresh context, well under 150K tokens. Read `GLOSSARY.md`, the spec, and the relevant ADRs; skip anything unrelated. Keep test and build output terse (filter to failures).

- [ ] Input and output dropdowns list available JACK ports and reconnect on change
- [ ] Clear in-window messages for: audio server unreachable, Scarlett not found, chosen port vanished
- [ ] Unplug then replug the Scarlett: the app reconnects (automatically or via one click) without restart
- [ ] Settings file under the user's config directory stores last-used ports; a missing or corrupt settings file falls back to defaults without crashing
- [ ] Verified by hand: unplug/replug, minimise while playing, launch with the Scarlett absent
