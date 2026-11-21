# Rust, JACK and egui, with a pure DSP core

The app is written in Rust, using the `jack` crate (through PipeWire's JACK API) for audio and egui for the window. All sound processing lives in a library with no audio-backend or GUI dependencies, so it can be tested offline against recorded bass clips and later wrapped as a plugin. We chose plain crates over nih-plug because its standalone mode is secondary, and over C++/Qt or JUCE for easier realtime-safe code.
