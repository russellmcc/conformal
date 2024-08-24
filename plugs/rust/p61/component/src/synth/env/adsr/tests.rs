use super::{calc_coeffs, Adsr, Params};
use assert_approx_eq::assert_approx_eq;
use snapshots::assert_snapshot;

#[test]
fn silence_until_turned_on() {
    let mut adsr: Adsr = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.0,
            decay_time: 0.0,
            sustain: 1.0,
            release_time: 0.0,
        },
        48000.0,
    );
    assert_eq!(
        std::iter::repeat_with(|| adsr.process(&coeffs))
            .take(100)
            .collect::<Vec<_>>(),
        std::iter::repeat(0f32).take(100).collect::<Vec<_>>()
    );
}

#[test]
fn reset() {
    let mut adsr: Adsr = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.010,
            decay_time: 0.100,
            sustain: 0.7,
            release_time: 0.200,
        },
        48000.0,
    );
    adsr.on();
    let initial = std::iter::repeat_with(|| adsr.process(&coeffs))
        .take(100)
        .collect::<Vec<_>>();
    adsr.reset();
    adsr.on();
    let reset = std::iter::repeat_with(|| adsr.process(&coeffs))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn snapshot() {
    let mut adsr: Adsr = Default::default();
    let coeffs = calc_coeffs(
        &Params {
            attack_time: 0.010,
            decay_time: 0.100,
            sustain: 0.7,
            release_time: 0.200,
        },
        48000.0,
    );
    adsr.on();
    assert_snapshot!(
        "adsr",
        48000,
        (0..48000).map(|i| {
            if i == 24000 {
                adsr.off();
            }
            adsr.process(&coeffs)
        })
    );
}
