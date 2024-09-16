use crate::audio::all_approx_eq;

use super::super::{
    PiecewiseLinearCurve, PiecewiseLinearCurvePoint, TimedEnumValues, TimedSwitchValues, TimedValue,
};
use super::{piecewise_linear_curve_per_sample, timed_enum_per_sample, timed_switch_per_sample};

const TEST_EPSILON: f32 = 1e-7;

#[test]
fn piecewise_linear_curve_per_sample_basics() {
    let vals = piecewise_linear_curve_per_sample(
        PiecewiseLinearCurve::new(
            (&[
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 5,
                    value: 5.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 7,
                    value: 5.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 8,
                    value: 10.0,
                },
            ])
                .iter()
                .cloned(),
            10,
            0.0..=10.0,
        )
        .unwrap(),
    )
    .collect::<Vec<_>>();
    assert!(all_approx_eq(
        vals.iter().copied(),
        ([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 10.0, 10.0])
            .iter()
            .copied(),
        TEST_EPSILON
    ));
}

#[test]
fn timed_enum_per_sample_basics() {
    let vals = timed_enum_per_sample(
        TimedEnumValues::new(
            (&[
                TimedValue {
                    sample_offset: 0,
                    value: 0,
                },
                TimedValue {
                    sample_offset: 7,
                    value: 2,
                },
                TimedValue {
                    sample_offset: 8,
                    value: 3,
                },
            ])
                .iter()
                .cloned(),
            10,
            0..4,
        )
        .unwrap(),
    )
    .collect::<Vec<_>>();
    assert!(vals
        .iter()
        .copied()
        .zip(([0, 0, 0, 0, 0, 0, 0, 2, 3, 3]).iter().copied())
        .all(|(a, b)| a == b));
}

#[test]
fn timed_switch_per_sample_basics() {
    let vals = timed_switch_per_sample(
        TimedSwitchValues::new(
            (&[
                TimedValue {
                    sample_offset: 0,
                    value: false,
                },
                TimedValue {
                    sample_offset: 7,
                    value: true,
                },
                TimedValue {
                    sample_offset: 8,
                    value: false,
                },
            ])
                .iter()
                .cloned(),
            10,
        )
        .unwrap(),
    )
    .collect::<Vec<_>>();
    assert!(vals
        .iter()
        .copied()
        .zip(
            ([false, false, false, false, false, false, false, true, false, false])
                .iter()
                .copied()
        )
        .all(|(a, b)| a == b));
}
