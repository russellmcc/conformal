use super::super::{
    EnumBufferState, NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint,
    SwitchBufferState, TimedEnumValues, TimedSwitchValues, TimedValue,
};

fn piecewise_linear_curve_per_sample<
    I: IntoIterator<Item = PiecewiseLinearCurvePoint, IntoIter: Clone>,
>(
    curve: PiecewiseLinearCurve<I>,
) -> impl Iterator<Item = f32> + Clone {
    let buffer_size = curve.buffer_size();
    let mut i = curve.into_iter();
    let mut next = i.next();
    let mut last = None;
    let mut last_sample_offset = 0;
    (0..buffer_size).map(move |idx| {
        if let Some(PiecewiseLinearCurvePoint {
            sample_offset,
            value,
        }) = next
        {
            if sample_offset == idx {
                last = Some(value);
                last_sample_offset = sample_offset;
                next = i.next();
                value
            } else {
                // unwrap is safe here because we know there is a point at zero.
                let delta = value - last.unwrap();

                // Note that we will fix any rounding errors when we hit the next point,
                // so we allow a lossy cast in the next block.
                #[allow(clippy::cast_precision_loss)]
                {
                    let delta_per_sample = delta / ((sample_offset - last_sample_offset) as f32);

                    last.unwrap() + delta_per_sample * ((idx - last_sample_offset) as f32)
                }
            }
        } else {
            // Unwrap is safe here because we know that there is at least one point in curve.
            last.unwrap()
        }
    })
}

#[doc(hidden)]
pub fn decompose_numeric<I: IntoIterator<Item = PiecewiseLinearCurvePoint, IntoIter: Clone>>(
    state: NumericBufferState<I>,
) -> (f32, Option<impl Iterator<Item = f32> + Clone>) {
    match state {
        NumericBufferState::Constant(v) => (v, None),
        NumericBufferState::PiecewiseLinear(c) => (0.0, Some(piecewise_linear_curve_per_sample(c))),
    }
}

/// Converts a [`NumericBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
/// Note: for constant values, this returns an infinite iterator.
pub fn numeric_per_sample<I: IntoIterator<Item = PiecewiseLinearCurvePoint, IntoIter: Clone>>(
    state: NumericBufferState<I>,
) -> impl Iterator<Item = f32> + Clone {
    match state {
        NumericBufferState::Constant(v) => itertools::Either::Left(core::iter::repeat(v)),
        NumericBufferState::PiecewiseLinear(c) => {
            itertools::Either::Right(piecewise_linear_curve_per_sample(c))
        }
    }
}

#[allow(clippy::missing_panics_doc)] // We only panic when invariants are broken.
fn timed_enum_per_sample<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
    values: TimedEnumValues<I>,
) -> impl Iterator<Item = u32> + Clone {
    let buffer_size = values.buffer_size();
    let mut i = values.into_iter();
    let mut next = i.next();
    let mut last = None;
    (0..buffer_size).map(move |idx| {
        if let Some(TimedValue {
            sample_offset,
            value,
        }) = next
        {
            if sample_offset == idx {
                last = Some(value);
                next = i.next();
                value
            } else {
                // unwrap is safe here because we know there is a point at zero.
                last.unwrap()
            }
        } else {
            // Unwrap is safe here because we know that there is at least one point in curve.
            last.unwrap()
        }
    })
}

#[doc(hidden)]
pub fn decompose_enum<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
    state: EnumBufferState<I>,
) -> (u32, Option<impl Iterator<Item = u32> + Clone>) {
    match state {
        EnumBufferState::Constant(v) => (v, None),
        EnumBufferState::Varying(c) => (0, Some(timed_enum_per_sample(c))),
    }
}

/// Converts an [`EnumBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
/// Note: for constant values, this returns an infinite iterator.
pub fn enum_per_sample<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
    state: EnumBufferState<I>,
) -> impl Iterator<Item = u32> + Clone {
    match state {
        EnumBufferState::Constant(v) => itertools::Either::Left(core::iter::repeat(v)),
        EnumBufferState::Varying(c) => itertools::Either::Right(timed_enum_per_sample(c)),
    }
}

#[allow(clippy::missing_panics_doc)] // We only panic when invariants are broken.
fn timed_switch_per_sample<I: IntoIterator<Item = TimedValue<bool>, IntoIter: Clone>>(
    values: TimedSwitchValues<I>,
) -> impl Iterator<Item = bool> + Clone {
    let buffer_size = values.buffer_size();
    let mut i = values.into_iter();
    let mut next = i.next();
    let mut last = None;
    (0..buffer_size).map(move |idx| {
        if let Some(TimedValue {
            sample_offset,
            value,
        }) = next
        {
            if sample_offset == idx {
                last = Some(value);
                next = i.next();
                value
            } else {
                // unwrap is safe here because we know there is a point at zero.
                last.unwrap()
            }
        } else {
            // Unwrap is safe here because we know that there is at least one point in curve.
            last.unwrap()
        }
    })
}

#[doc(hidden)]
pub fn decompose_switch<I: IntoIterator<Item = TimedValue<bool>, IntoIter: Clone>>(
    state: SwitchBufferState<I>,
) -> (bool, Option<impl Iterator<Item = bool> + Clone>) {
    match state {
        SwitchBufferState::Constant(v) => (v, None),
        SwitchBufferState::Varying(c) => (false, Some(timed_switch_per_sample(c))),
    }
}

/// Converts a [`SwitchBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
/// Note: for constant values, this returns an infinite iterator.
pub fn switch_per_sample<I: IntoIterator<Item = TimedValue<bool>, IntoIter: Clone>>(
    state: SwitchBufferState<I>,
) -> impl Iterator<Item = bool> + Clone {
    match state {
        SwitchBufferState::Constant(v) => itertools::Either::Left(core::iter::repeat(v)),
        SwitchBufferState::Varying(c) => itertools::Either::Right(timed_switch_per_sample(c)),
    }
}

#[cfg(test)]
mod tests {
    use crate::audio::all_approx_eq;

    use super::super::super::{
        PiecewiseLinearCurve, PiecewiseLinearCurvePoint, TimedEnumValues, TimedSwitchValues,
        TimedValue,
    };
    use super::{
        piecewise_linear_curve_per_sample, timed_enum_per_sample, timed_switch_per_sample,
    };

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
        assert!(
            vals.iter()
                .copied()
                .zip(([0, 0, 0, 0, 0, 0, 0, 2, 3, 3]).iter().copied())
                .all(|(a, b)| a == b)
        );
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
        assert!(
            vals.iter()
                .copied()
                .zip(
                    ([
                        false, false, false, false, false, false, false, true, false, false
                    ])
                    .iter()
                    .copied()
                )
                .all(|(a, b)| a == b)
        );
    }
}
