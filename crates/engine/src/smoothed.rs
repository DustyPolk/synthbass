/// A value that glides toward its target with a one-pole lowpass, so
/// parameter changes never step (no clicks).
pub struct Smoothed {
    current: f32,
    target: f32,
    coefficient: f32,
}

/// Time to cover ~63% of a change.
const TIME_CONSTANT_SECONDS: f32 = 0.01;
const SNAP: f32 = 1e-9;

impl Smoothed {
    pub fn new(value: f32, sample_rate: f32) -> Self {
        Smoothed {
            current: value,
            target: value,
            coefficient: (-1.0 / (TIME_CONSTANT_SECONDS * sample_rate)).exp(),
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn next(&mut self) -> f32 {
        self.current = self.target + (self.current - self.target) * self.coefficient;
        // Snap once inaudibly close: otherwise the gap shrinks into
        // subnormals (slow) and can stick at the smallest one, leaving a
        // level of "zero" that still leaks signal.
        if (self.current - self.target).abs() < SNAP {
            self.current = self.target;
        }
        self.current
    }
}
