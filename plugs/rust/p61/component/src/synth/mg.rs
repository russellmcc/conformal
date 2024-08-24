#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub struct Mg {
    phase: f32,
}

impl Mg {
    pub fn reset(&mut self) {
        self.phase = 0.0;
    }

    pub fn generate(&mut self, incr: f32) -> f32 {
        // Optimization opportunity - use complex numbers to generate sin
        let ret = (self.phase * std::f32::consts::TAU).sin();
        self.phase += incr.clamp(0.0, 1.0);
        if self.phase > 1.0 {
            self.phase -= 1.0;
        }
        ret
    }
}
