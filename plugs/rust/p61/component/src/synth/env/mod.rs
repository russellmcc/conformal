//! Holds linear envelope generators

pub mod adsr;
pub mod duck;

#[derive(Debug)]
enum Coeff {
    Instant,
    Increment(f32),
}

fn calc_coeff(time: f32, sampling_rate: f32) -> Coeff {
    let period = 1.0 / sampling_rate;
    if time < period {
        Coeff::Instant
    } else {
        Coeff::Increment(1.0 / (time * sampling_rate))
    }
}
