use util::{
    f32::rescale_inverted,
    test_utils::{linear_sine_sweep, white_noise},
};

use snapshots::assert_snapshot;

use super::Vca;

use assert_approx_eq::assert_approx_eq;
use more_asserts::assert_gt;

#[test]
fn reset() {
    let mut vca = Vca::new(48000.0);
    let mut initial = white_noise(100);
    let mut initial_clone = initial.clone();
    for sample in initial.iter_mut() {
        *sample = vca.process(*sample, 0.5);
    }
    let processed = initial;
    vca.reset();
    for sample in initial_clone.iter_mut() {
        *sample = vca.process(*sample, 0.5);
    }
    let after_reset = initial_clone;
    for (a, b) in processed.iter().zip(after_reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
fn control_signal_effects_volume() {
    let mut vca = Vca::new(48000.0);
    let mut initial = white_noise(100);
    let mut initial_clone = initial.clone();
    for sample in initial.iter_mut() {
        *sample = vca.process(*sample, 0.8);
    }
    let processed_loud = initial;
    vca.reset();
    for sample in initial_clone.iter_mut() {
        *sample = vca.process(*sample, 0.25);
    }
    let processed_quiet = initial_clone;
    let processed_loud_power = processed_loud.iter().map(|x| x * x).sum::<f32>();
    let processed_quiet_power = processed_quiet.iter().map(|x| x * x).sum::<f32>();
    let power_ratio_db = 10.0 * (processed_loud_power / processed_quiet_power).log10();
    assert_gt!(power_ratio_db, 10.0);
}

#[test]
fn silent_at_zero_control() {
    let mut vca = Vca::new(48000.0);
    let mut initial = white_noise(100);
    for sample in initial.iter_mut() {
        *sample = vca.process(*sample, 0.0);
    }
    let processed = initial;
    for sample in processed.iter() {
        assert_approx_eq!(*sample, 0.0);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot() {
    let mut vca = Vca::new(48000.0);
    let mut processed = linear_sine_sweep(48000, 48000.0, 20.0, 10000.0);
    for (index, sample) in processed.iter_mut().enumerate() {
        *sample = vca.process(
            *sample,
            rescale_inverted(index as f32, 0.0..=48000.0, 0.0..=1.0),
        );
    }
    assert_snapshot!("sweep", 48000, processed);
}
