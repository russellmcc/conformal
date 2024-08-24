use super::{calc_coeffs, Ar, Params};
use assert_approx_eq::assert_approx_eq;
use snapshots::assert_snapshot;

#[test]
fn starts_fully_on() {
    let mut ar: Ar = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.0,
            release_time: 0.0,
        },
        48000.0,
    );
    assert_eq!(
        std::iter::repeat_with(|| ar.process(&coeffs))
            .take(100)
            .collect::<Vec<_>>(),
        std::iter::repeat(1f32).take(100).collect::<Vec<_>>()
    );
}

#[test]
fn reset() {
    let mut ar: Ar = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.010,
            release_time: 0.200,
        },
        48000.0,
    );
    ar.on();
    let initial = std::iter::repeat_with(|| ar.process(&coeffs))
        .take(100)
        .collect::<Vec<_>>();
    ar.reset();
    ar.on();
    let reset = std::iter::repeat_with(|| ar.process(&coeffs))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
fn handles_multiple_notes() {
    let mut ar: Ar = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.0,
            release_time: 0.0,
        },
        48000.0,
    );
    ar.on();
    ar.on();
    assert_approx_eq!(ar.process(&coeffs), 0.0);
    ar.off();
    assert_approx_eq!(ar.process(&coeffs), 1.0);
    ar.on();
    assert_approx_eq!(ar.process(&coeffs), 1.0);
    ar.off();
    assert_approx_eq!(ar.process(&coeffs), 1.0);
    ar.off();
    assert_approx_eq!(ar.process(&coeffs), 1.0);
    ar.on();
    assert_approx_eq!(ar.process(&coeffs), 0.0);
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot() {
    let mut ar: Ar = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.200,
            release_time: 0.010,
        },
        48000.0,
    );
    ar.on();
    assert_snapshot!(
        "ar",
        48000,
        (0..48000).map(|i| {
            if i == 24000 {
                ar.off();
            }
            ar.process(&coeffs)
        })
    );
}
