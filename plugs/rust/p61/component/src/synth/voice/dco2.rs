use num_derive::FromPrimitive;

use crate::synth::osc_utils::pulse;

mod filter;

#[derive(Debug, Default)]
pub struct Dco2 {
    phase: f32,
    filter: filter::Filter,
}

#[derive(FromPrimitive, Copy, Clone, Debug, PartialEq)]
pub enum Shape {
    Saw,
    Square,
}

#[derive(FromPrimitive, Copy, Clone, Debug, PartialEq)]
pub enum Octave {
    Low,
    Medium,
    High,
}

fn saw(phase: f32, octave: Octave) -> f32 {
    let bits = match octave {
        Octave::Low => 4f32,
        Octave::Medium => 8f32,
        Octave::High => 16f32,
    };
    let crushed = (phase * (bits)).min(bits - 1.0).floor() / (bits - 1.0);
    2.0 * crushed - 1.0
}

impl Dco2 {
    pub fn reset(&mut self) {
        self.phase = 0.0;
        self.filter.reset();
    }

    /// Generates a sample of the DCO2.
    ///  - increment: The increment of the fundamental frequency.
    pub fn generate(&mut self, increment: f32, shape: Shape, octave: Octave) -> f32 {
        if increment > 0.5 {
            // We can't go higher than nyquist!
            return 0.0;
        }
        self.phase += increment;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        match shape {
            Shape::Square => pulse(self.phase, increment, 0.5),
            Shape::Saw => self.filter.process(saw(self.phase, octave), increment),
        }
    }
}

#[cfg(test)]
mod tests;
