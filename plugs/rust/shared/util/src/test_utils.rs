#![allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)]

use rand::{Rng, SeedableRng};
use rand_xoshiro::Xoshiro256PlusPlus;

pub fn fill_with_white_noise(buffer: &mut [f32]) {
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(420);
    for sample in buffer.iter_mut() {
        *sample = rng.gen_range(-1.0..1.0);
    }
}

#[must_use]
pub fn white_noise(len: usize) -> Vec<f32> {
    let mut buffer = vec![0.0; len];
    fill_with_white_noise(&mut buffer);
    buffer
}

#[must_use]
pub fn linear_sine_sweep(
    len: usize,
    sampling_rate: f32,
    start_freq: f32,
    end_freq: f32,
) -> Vec<f32> {
    let mut buffer = vec![0f32; len];
    let mut phase = 0f64;
    let mut increment = f64::from(start_freq) / f64::from(sampling_rate) * std::f64::consts::TAU;
    let dincrement = (f64::from(end_freq) / f64::from(sampling_rate) * std::f64::consts::TAU
        - increment)
        / len as f64;
    for sample in &mut buffer {
        *sample = phase.sin() as f32;
        phase += increment;
        increment += dincrement;
    }
    buffer
}

#[must_use]
pub fn sine(len: usize, increment: f32) -> Vec<f32> {
    let mut buffer = vec![0f32; len];
    let mut phase = 0f64;
    let increment = f64::from(increment) * std::f64::consts::TAU;
    for sample in &mut buffer {
        *sample = phase.sin() as f32;
        phase += increment;
    }
    buffer
}

use num::Complex;
use realfft::RealFftPlanner;

use crate::window;

fn rfft(data: &mut [f32]) -> Vec<Complex<f32>> {
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(data.len());
    let mut spectrum = r2c.make_output_vec();
    r2c.process(data, &mut spectrum).unwrap();
    spectrum
}

/// Note that this will thrash `data`.
pub fn windowed_rfft(data: &mut [f32]) -> Vec<Complex<f32>> {
    window::hamming(data);
    rfft(data)
}

/// Estimates the tuning of the input data.
///
/// # Panics
///
/// This function panics if the input data is empty.
pub fn estimate_tuning(data: &mut [f32]) -> f32 {
    let spectrum = windowed_rfft(data);
    // Here we don't count the first 10 bins as a crude DC blocking filter.
    let max_index = spectrum[10..spectrum.len() - 1]
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.norm().total_cmp(&b.norm()))
        .unwrap()
        .0
        + 10;

    // Use lagrange quadratic interpolation to refine the estimate.
    let y1 = spectrum[max_index - 1].norm();
    let y2 = spectrum[max_index].norm();
    let y3 = spectrum[max_index + 1].norm();
    let fine_tuned = 0.5 * (y1 - y3) / (y1 - y2 + y2 + y3);
    let max_index = if fine_tuned.abs() > 0.5 {
        // Something went wrong with the fine tuning!
        max_index as f32
    } else {
        max_index as f32 + fine_tuned
    };
    max_index / data.len() as f32
}

pub fn estimate_tuning_gen(gen: impl FnMut() -> f32) -> f32 {
    let mut data = std::iter::repeat_with(gen).take(4096).collect::<Vec<_>>();
    estimate_tuning(&mut data)
}

fn energy_to_decibels(energy: f32) -> f32 {
    10.0 * energy.log10()
}

/// Estimates the ratio of energy that is below the fundamental frequency
/// in decibels. This is a crude measure of "aliasing".
pub fn estimate_aliasing(data: &mut [f32], increment: f32) -> f32 {
    let spectrum = windowed_rfft(data);

    let first_signal_bin = (increment * data.len() as f32).floor() as usize;
    // Note we skip the first 10 bins to not count any DC as aliasing.
    let total_energy = spectrum[10..spectrum.len() - 1]
        .iter()
        .map(num::Complex::norm_sqr)
        .sum::<f32>();
    let aliasing_energy = spectrum[10..first_signal_bin]
        .iter()
        .map(num::Complex::norm_sqr)
        .sum::<f32>();
    energy_to_decibels(aliasing_energy / total_energy)
}

/// Estimates the ratio of energy that is below the fundamental frequency
/// in decibels. This is a crude measure of "aliasing".
pub fn estimate_aliasing_gen(gen: impl FnMut() -> f32, increment: f32) -> f32 {
    let mut data = std::iter::repeat_with(gen).take(4096).collect::<Vec<_>>();
    estimate_aliasing(&mut data, increment)
}
