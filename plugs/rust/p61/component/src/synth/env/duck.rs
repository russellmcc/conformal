//! Weird ducking envelope use for MG delay.

use super::{calc_coeff, Coeff};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub struct Params {
    pub attack_time: f32,
    pub release_time: f32,
}

#[derive(Debug)]
pub struct Coeffs {
    attack: Coeff,
    release: Coeff,
}

pub fn calc_coeffs(params: &Params, sampling_rate: f32) -> Coeffs {
    Coeffs {
        attack: calc_coeff(params.attack_time, sampling_rate),
        release: calc_coeff(params.release_time, sampling_rate),
    }
}

#[derive(Debug, Default, Clone)]
enum Stage {
    Attack {
        value: f32,
    },
    #[default]
    On,
    Release {
        value: f32,
    },
}

#[derive(Debug, Default)]
pub struct Ar {
    stage: Stage,
    note_count: usize,
}

impl Ar {
    pub fn reset(&mut self) {
        self.stage = Default::default();
        self.note_count = 0;
    }

    pub fn on(&mut self) {
        match self.stage.clone() {
            Stage::Attack { .. } | Stage::Release { .. } => {
                self.note_count += 1;
            }
            Stage::On => {
                if self.note_count > 0 {
                    self.note_count -= 1;
                } else {
                    self.stage = Stage::Release { value: 1.0 };
                    self.note_count = 1;
                }
            }
        }
    }

    pub fn off(&mut self) {
        self.note_count = self.note_count.saturating_sub(1);
    }

    pub fn process(&mut self, coeffs: &Coeffs) -> f32 {
        let (out, new_stage) = match self.stage {
            Stage::Attack { value } => match coeffs.attack {
                Coeff::Instant => (1.0, Stage::On),
                Coeff::Increment(incr) => {
                    let value = value + incr;
                    if value >= 1.0 {
                        (1.0, Stage::On)
                    } else {
                        (value, Stage::Attack { value })
                    }
                }
            },
            Stage::On => (1.0, Stage::On),
            Stage::Release { value } => match coeffs.release {
                Coeff::Instant => (0.0, Stage::Attack { value: 0.0 }),
                Coeff::Increment(incr) => {
                    let value = value - incr;
                    if value <= 0.0 {
                        (0.0, Stage::Attack { value: 0.0 })
                    } else {
                        (value, Stage::Release { value })
                    }
                }
            },
        };
        self.stage = new_stage;
        out
    }
}
