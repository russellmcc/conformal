use assert_approx_eq::assert_approx_eq;
use more_asserts::{assert_gt, assert_lt};
use util::test_utils::{white_noise, windowed_rfft};

use super::*;

#[test]
#[cfg_attr(miri, ignore)]
fn lpf_lowers_high_freqs() {
    let mut filter: Svf = Default::default();
    let mut input = white_noise(4096);
    let params = RawParams {
        g: calc_g(0.25),
        two_r: 2.,
    };
    let mut processed: Vec<_> = filter
        .process_low(input.iter().map(|x| Input {
            x: f64::from(*x),
            params,
        }))
        .map(|x| x as f32)
        .collect();
    let spectrum = windowed_rfft(&mut input);
    let processed_spectrum = windowed_rfft(&mut processed);

    // Check that it's significantly reducing power at high frequencies
    let high_freq = 2044;
    let power_reduction_at_high_freq =
        processed_spectrum[high_freq].norm_sqr() / spectrum[high_freq].norm_sqr();
    assert_lt!(power_reduction_at_high_freq, 0.2);

    // Also, check that it didn't reduce power in the low frequencies.
    let low_freq = 50;
    let power_reduction_at_low_freq =
        processed_spectrum[low_freq].norm_sqr() / spectrum[low_freq].norm_sqr();
    assert_gt!(power_reduction_at_low_freq, 0.99);
}

#[test]
#[cfg_attr(miri, ignore)]
fn hpf_lowers_low_freqs() {
    let mut filter: Svf = Default::default();
    let mut input = white_noise(4096);
    let params = RawParams {
        g: calc_g(0.25),
        two_r: 2.,
    };
    let mut processed: Vec<_> = filter
        .process_high(input.iter().map(|x| Input {
            x: f64::from(*x),
            params,
        }))
        .map(|x| x as f32)
        .collect();
    let spectrum = windowed_rfft(&mut input);
    let processed_spectrum = windowed_rfft(&mut processed);

    // Check that it didn't reduce power in the high frequencies.
    let high_freq = 2044;
    let power_reduction_at_high_freq =
        processed_spectrum[high_freq].norm_sqr() / spectrum[high_freq].norm_sqr();
    assert_gt!(power_reduction_at_high_freq, 0.99);

    // Also, check that it's significantly reducing power at low frequencies
    let low_freq = 50;
    let power_reduction_at_low_freq =
        processed_spectrum[low_freq].norm_sqr() / spectrum[low_freq].norm_sqr();
    assert_lt!(power_reduction_at_low_freq, 0.2);
}

#[test]
#[cfg_attr(miri, ignore)]
fn bpf_lowers_both() {
    let mut filter: Svf = Default::default();
    let mut input = white_noise(4096);
    let params = RawParams {
        g: calc_g(0.25),
        two_r: 2.,
    };
    let mut processed: Vec<_> = filter
        .process_band(input.iter().map(|x| Input {
            x: f64::from(*x),
            params,
        }))
        .map(|x| x as f32)
        .collect();
    let spectrum = windowed_rfft(&mut input);
    let processed_spectrum = windowed_rfft(&mut processed);

    // Check that it's significantly reducing power at high frequencies
    let high_freq = 2044;
    let power_reduction_at_high_freq =
        processed_spectrum[high_freq].norm_sqr() / spectrum[high_freq].norm_sqr();
    assert_lt!(power_reduction_at_high_freq, 0.2);

    // Also, check that it's significantly reducing power at low frequencies.
    let low_freq = 50;
    let power_reduction_at_low_freq =
        processed_spectrum[low_freq].norm_sqr() / spectrum[low_freq].norm_sqr();
    assert_lt!(power_reduction_at_low_freq, 0.2);
}

#[test]
fn reset() {
    let mut filter: Svf = Default::default();
    let params = RawParams {
        g: calc_g(0.25),
        two_r: 2.,
    };
    let input = white_noise(100);
    let processed: Vec<_> = filter
        .process_band(input.iter().map(|x| Input {
            x: f64::from(*x),
            params,
        }))
        .map(|x| x as f32)
        .collect();
    filter.reset();
    let after_reset: Vec<_> = filter
        .process_band(input.iter().map(|x| Input {
            x: f64::from(*x),
            params,
        }))
        .map(|x| x as f32)
        .collect();
    for (a, b) in processed.iter().zip(after_reset.iter()) {
        assert_approx_eq!(a, b);
    }
}
