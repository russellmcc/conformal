use super::{Dco1, Shape};
use assert_approx_eq::assert_approx_eq;
use more_asserts::assert_lt;
use snapshots::assert_snapshot;
use util::test_utils::{estimate_aliasing_gen, estimate_tuning_gen};

#[test]
#[cfg_attr(miri, ignore)]
fn saw_tuning() {
    let increment = 482.5 / 44100.0;
    let mut dco1 = Dco1::default();
    assert_approx_eq!(
        estimate_tuning_gen(|| dco1.generate(increment, 10.0, Shape::Saw)),
        increment,
        1e-4
    );
    assert_approx_eq!(
        estimate_tuning_gen(|| dco1.generate(increment, 100.0, Shape::Saw)),
        increment,
        1e-4
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn pulse_tuning() {
    let increment = 482.5 / 44100.0;
    let mut dco1 = Dco1::default();
    assert_approx_eq!(
        estimate_tuning_gen(|| dco1.generate(increment, 10.0, Shape::Pulse { width: 0.25 })),
        increment,
        1e-4
    );
}

#[test]
fn reset_basics() {
    let increment = 482.5 / 44100.0;
    let mut dco1 = Dco1::default();
    let initial = std::iter::repeat_with(|| dco1.generate(increment, 10.0, Shape::Saw))
        .take(100)
        .collect::<Vec<_>>();
    dco1.reset();
    let reset = std::iter::repeat_with(|| dco1.generate(increment, 10.0, Shape::Saw))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn saw_aliasing_suppression() {
    let increment = 0.246246246;
    let mut dco1 = Dco1::default();
    assert_lt!(
        estimate_aliasing_gen(|| dco1.generate(increment, 10.0, Shape::Saw), increment),
        -13.0
    );
    assert_lt!(
        estimate_aliasing_gen(|| dco1.generate(increment, 100.0, Shape::Saw), increment),
        -13.0
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn pulse_aliasing_suppression() {
    let increment = 0.246246246;
    let mut dco1 = Dco1::default();
    assert_lt!(
        estimate_aliasing_gen(
            || dco1.generate(increment, 10.0, Shape::Pulse { width: 0.25 }),
            increment
        ),
        -13.0
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn saw_sweep_snapshot() {
    let mut dco1 = Dco1::default();
    let max_increment = 0.1;
    let num_samples = 48000;

    assert_snapshot!(
        "saw_sweep",
        48000,
        (0..num_samples).map(|i| {
            dco1.generate(
                i as f32 / num_samples as f32 * max_increment,
                10.0 + i as f32 / num_samples as f32 * 100.0,
                Shape::Saw,
            )
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn pulse_sweep_snapshot() {
    let mut dco1 = Dco1::default();
    let max_increment = 0.1;
    let num_samples = 48000;

    assert_snapshot!(
        "pulse_sweep",
        48000,
        (0..num_samples).map(|i| {
            dco1.generate(
                i as f32 / num_samples as f32 * max_increment,
                10.0 + i as f32 / num_samples as f32 * 100.0,
                Shape::Pulse { width: 0.25 },
            )
        })
    );
}
