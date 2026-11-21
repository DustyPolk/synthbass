# Hybrid of direct and tracked layers

Pitch tracking alone lags 30–50 ms on low bass notes (low E is 41 Hz, and detection needs 1.5–2 periods), which feels mushy; synthesis shaped directly from the signal has zero added latency but a narrow palette. We combine both: a direct layer gives every note an instant attack, and a tracked layer with real oscillators fades in underneath it for the full synth palette.
