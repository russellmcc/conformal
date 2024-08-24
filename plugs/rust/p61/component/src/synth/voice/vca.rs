//! "Vibe"-level emulation of the poly-61 VCA, which is a single-transistor.
//! This uses some pretty obscure transistor modes, ([reference](https://www.timstinchcombe.co.uk/synth/MS20_study.pdf) - section 3.1)
//!
//! We "capture the vibe" of this just by making a waveshape that is non-linear
//! in both the control and signal inputs - it's just made up.

use iir::dc_blocker;
use util::f32::rescale;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct Vca {
    dc_blocker: dc_blocker::DcBlocker,
}

impl Vca {
    pub fn new(sampling_rate: f32) -> Self {
        Self {
            dc_blocker: dc_blocker::DcBlocker::new(sampling_rate),
        }
    }

    pub fn reset(&mut self) {
        self.dc_blocker.reset();
    }

    pub fn process(&mut self, input: f32, control: f32) -> f32 {
        let y = rescale(input, -1.0..=1.0, 0.0..=1.0);
        let input_shaped = rescale(y * y, 0.0..=1.0, -1.0..=1.0);
        let input_dc_blocked = self.dc_blocker.process(input_shaped);
        input_dc_blocked * control * control
    }
}
