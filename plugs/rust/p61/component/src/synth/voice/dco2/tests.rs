use util::test_utils::{estimate_aliasing_gen, estimate_tuning_gen};

use super::{Dco2, Octave, Shape};
use assert_approx_eq::assert_approx_eq;
use more_asserts::assert_lt;
use snapshots::assert_snapshot;

#[test]
#[cfg_attr(miri, ignore)]
fn tuning() {
    let increment = 482.5 / 44100.0;
    let mut dco2 = Dco2::default();
    assert_approx_eq!(
        estimate_tuning_gen(|| dco2.generate(increment, Shape::Square, Octave::Medium)),
        increment,
        1e-4
    );
}

#[test]
fn reset_basics() {
    let increment = 482.5 / 44100.0;
    let mut dco2 = Dco2::default();
    let initial =
        std::iter::repeat_with(|| dco2.generate(increment, Shape::Square, Octave::Medium))
            .take(100)
            .collect::<Vec<_>>();
    dco2.reset();
    let reset = std::iter::repeat_with(|| dco2.generate(increment, Shape::Square, Octave::Medium))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
fn reset_saw_basics() {
    let increment = 482.5 / 44100.0;
    let mut dco2 = Dco2::default();
    let initial = std::iter::repeat_with(|| dco2.generate(increment, Shape::Saw, Octave::Medium))
        .take(100)
        .collect::<Vec<_>>();
    dco2.reset();
    let reset = std::iter::repeat_with(|| dco2.generate(increment, Shape::Saw, Octave::Medium))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn aliasing_suppression() {
    let increment = 0.246246246;
    let mut dco2 = Dco2::default();
    assert_lt!(
        estimate_aliasing_gen(
            || dco2.generate(increment, Shape::Square, Octave::Medium),
            increment
        ),
        -15.0
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn sweep() {
    let mut dco2 = Dco2::default();
    let max_increment = 0.1;
    let num_samples = 48000;

    assert_snapshot!(
        "sweep",
        48000,
        (0..num_samples).map(|i| {
            dco2.generate(
                i as f32 / num_samples as f32 * max_increment,
                Shape::Square,
                Octave::Medium,
            )
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn saw_tuning() {
    let increment = 482.5 / 44100.0;
    let mut dco2 = Dco2::default();
    assert_approx_eq!(
        estimate_tuning_gen(|| dco2.generate(increment, Shape::Saw, Octave::Low)),
        increment,
        1e-4
    );
    assert_approx_eq!(
        estimate_tuning_gen(|| dco2.generate(increment, Shape::Saw, Octave::Medium)),
        increment,
        1e-4
    );
    assert_approx_eq!(
        estimate_tuning_gen(|| dco2.generate(increment, Shape::Saw, Octave::High)),
        increment,
        1e-4
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn saw_sweep() {
    let mut dco2 = Dco2::default();
    let max_increment = 0.1;
    let num_samples = 48000;

    assert_snapshot!(
        "saw_sweep",
        48000,
        (0..num_samples).map(|i| {
            dco2.generate(
                i as f32 / num_samples as f32 * max_increment,
                Shape::Saw,
                Octave::Medium,
            )
        })
    );
}

#[test]
#[cfg_attr(miri, ignore)]
fn aliasing_suppression_saw() {
    let increment = 0.246246246;
    let mut dco2 = Dco2::default();
    assert_lt!(
        estimate_aliasing_gen(
            || dco2.generate(increment, Shape::Saw, Octave::Medium),
            increment
        ),
        -15.0
    );
}
