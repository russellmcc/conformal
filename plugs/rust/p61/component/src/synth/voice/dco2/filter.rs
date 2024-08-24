//! This holds a tracking filter to cheekily remove the aliasing
//! that occurs under the fundamental frequency of DCO2's sawtooth.
//! This technique of allowing aliasing but postfiltering it to remove
//! "obvious" aliasing below the fundamental is used in the Roland JP-8000.

#[derive(Debug, Default)]
pub struct Filter {
    s0: f32,
    s1: f32,
}

// We process with a fixed Q of sqrt(2).
const TWO_R: f32 = std::f32::consts::SQRT_2;

impl Filter {
    pub fn process(&mut self, input: f32, increment: f32) -> f32 {
        // Here "k" should by rights be tan(tau/2 * increment),
        // but we approximate it with a linear approximation
        // for runtime efficiency. This will make the tuning terrible
        // at high frequencies. If we multiplied this by tau/2,
        // this would match the slope at 0hz. We cheat a little
        // and multiply by tau/3 instead.
        let k = increment * std::f32::consts::TAU / 3.0;

        let m = 1f32 / (1f32 + k * TWO_R + k * k);

        let ds0 = input - self.s1 - (TWO_R + k) * self.s0;
        let k_ds0 = k * ds0;
        self.s1 = m * (self.s1 + 2f32 * k * (self.s0 + k_ds0));
        self.s0 = m * (self.s0 + 2f32 * k_ds0);
        m * ds0
    }

    pub fn reset(&mut self) {
        self.s0 = 0.0;
        self.s1 = 0.0;
    }
}
