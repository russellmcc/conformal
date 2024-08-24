use crate::synth::osc_utils::{polyblep2_residual, pulse};

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub struct Dco1 {
    phase: f32,
}

/// This very loosely emulates the waveshape of the DCO1 on the
/// Poly-61, which charges a capacitor with a voltage source
/// (rather than a current source, which would yield a linear ramp).
fn saw_waveshape(phase: f32, note: f32) -> f32 {
    let shaped = 1.0 - (-phase * 10.0 * 2f32.log2()).exp();

    // At low frequencies the effect is more pronounced.
    // We emulate this by blending the unshaped phase with the
    // shaped phase depending on the note.
    let key = 1.0 - ((note - 30.0) / 50.0).clamp(0.0, 1.0);
    2.0 * (key * shaped + (1.0 - key) * phase) - 1.0
}

fn saw(phase: f32, increment: f32, note: f32) -> f32 {
    saw_waveshape(phase, note) - polyblep2_residual(phase, increment)
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Shape {
    Saw,
    Pulse { width: f32 },
}

impl Dco1 {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    /// Generates a sample of the DCO1.
    ///  - increment: The increment of the fundamental frequency.
    ///  - note: The note of the fundamental frequency (MIDI note number)
    ///  - shape: the shape.
    pub fn generate(&mut self, increment: f32, note: f32, shape: Shape) -> f32 {
        if increment > 0.5 {
            // We can't go higher than nyquist!
            return 0.0;
        }
        self.phase += increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        match shape {
            Shape::Saw => saw(self.phase, increment, note),
            Shape::Pulse { width } => pulse(self.phase, increment, width),
        }
    }
}
