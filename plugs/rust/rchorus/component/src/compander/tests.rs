use super::*;
use assert_approx_eq::assert_approx_eq;

#[test]
#[cfg_attr(miri, ignore)]
fn roughly_accurate_for_sine() {
    let sampling_rate = 48000.;
    let mut detector = PeakLevelDetector::new(sampling_rate);

    let test_sig = util::test_utils::sine(48000, 1123. / 48000.);
    let detected = test_sig
        .iter()
        .map(|x| detector.detect_level(*x))
        .last()
        .unwrap();
    assert_approx_eq!(detected, 1.0, 0.1);
}

#[test]
#[cfg_attr(miri, ignore)]
fn roughly_accurate_for_sine_2() {
    let sampling_rate = 48000.;
    let mut detector = PeakLevelDetector::new(sampling_rate);

    let test_sig = util::test_utils::sine(48000, 1123. / 48000.);
    let detected = test_sig
        .iter()
        .map(|x| detector.detect_level(*x * 0.3))
        .last()
        .unwrap();
    assert_approx_eq!(detected, 0.3, 5e-2);
}

#[test]
fn reset() {
    let sampling_rate = 48000.;
    let mut detector = PeakLevelDetector::new(sampling_rate);

    let test_sig = util::test_utils::sine(100, 440. / 48000.);
    let detected = test_sig
        .iter()
        .map(|x| detector.detect_level(*x * 0.3))
        .collect::<Vec<_>>();
    detector.reset();
    let after_reset = test_sig
        .iter()
        .map(|x| detector.detect_level(*x * 0.3))
        .collect::<Vec<_>>();

    for (a, b) in detected.iter().zip(after_reset.iter()) {
        assert_approx_eq!(a, b);
    }
}
