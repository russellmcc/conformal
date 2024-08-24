use super::DcBlocker;
use assert_approx_eq::assert_approx_eq;
use more_asserts::{assert_gt, assert_lt};
use snapshots::assert_snapshot;
use util::test_utils::{white_noise, windowed_rfft};

#[test]
fn reset() {
    let mut blocker = DcBlocker::new(48000.0);
    let mut initial = white_noise(100);
    let mut initial_clone = initial.clone();
    for sample in initial.iter_mut() {
        *sample = blocker.process(*sample);
    }
    let processed = initial;
    blocker.reset();
    for sample in initial_clone.iter_mut() {
        *sample = blocker.process(*sample);
    }
    let after_reset = initial_clone;
    for (a, b) in processed.iter().zip(after_reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn lowers_dc() {
    let mut blocker = DcBlocker::new(48000.0);
    let mut input = white_noise(8192);
    // Add artificial DC offset
    for input in input.iter_mut() {
        *input = *input * 0.1 + 0.9;
    }
    let mut processed = input.clone();
    for sample in processed.iter_mut() {
        *sample = blocker.process(*sample);
    }
    let spectrum = windowed_rfft(&mut input);
    let processed_spectrum = windowed_rfft(&mut processed);

    // Check that it's significantly reducing power at DC
    let power_reduction_at_dc = processed_spectrum[0].norm_sqr() / spectrum[0].norm_sqr();
    assert_lt!(power_reduction_at_dc, 0.1);

    // Also, check that it didn't reduce power in the middle of the spectrum.
    let power_reduction_mid_spectrum =
        processed_spectrum[1000].norm_sqr() / spectrum[1000].norm_sqr();
    assert_gt!(power_reduction_mid_spectrum, 0.99);
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot() {
    let mut blocker = DcBlocker::new(48000.0);
    let mut processed = white_noise(48000);
    for sample in processed.iter_mut() {
        *sample = blocker.process(*sample);
    }
    assert_snapshot!("snapshot", 48000, processed);
}
