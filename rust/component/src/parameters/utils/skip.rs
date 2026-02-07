use crate::parameters::{PiecewiseLinearCurve, PiecewiseLinearCurvePoint};

/// Skips a number of samples from the start of a piecewise linear curve.
///
/// Note that this outputs a points iterator, but it should satisfy
/// all the invariants of a piecewise linear curve, so can be used
/// to construct a new [`PiecewiseLinearCurve`].
///
/// If `samples_to_skip` goes past the end of the buffer, a single-point
/// curve with the last value in the curve will be returned.
///
/// # Example
///
/// ```
/// # use conformal_component::parameters::{PiecewiseLinearCurve, PiecewiseLinearCurvePoint, skip_piecewise_linear};
/// # use assert_approx_eq::assert_approx_eq;
/// let curve = PiecewiseLinearCurve::new(vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
///                                            PiecewiseLinearCurvePoint { sample_offset: 100, value: 1.0 }].into_iter(),
///                                            128, 0.0..=1.0).unwrap();
/// let points = skip_piecewise_linear(curve, 50).collect::<Vec<_>>();
/// assert_eq!(points.len(), 2);
/// assert_eq!(points[0].sample_offset, 0);
/// assert_eq!(points[1].sample_offset, 50);
/// assert_approx_eq!(points[0].value, 0.5);
/// assert_approx_eq!(points[1].value, 1.0);
/// ```
pub fn skip_piecewise_linear(
    curve: PiecewiseLinearCurve<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>,
    samples_to_skip: usize,
) -> impl Iterator<Item = PiecewiseLinearCurvePoint> {
    let mut i = curve.into_iter();

    // Safety: we know there is at least one point in the curve due to the
    // invariants of `PiecewiseLinearCurve` (specifically the requirement for
    // a point at zero)
    #[allow(clippy::missing_panics_doc)]
    let mut prev = i.next().unwrap();
    let mut next = i.next();

    // Skip until we are out of points or we the next point is past the skip point.
    // Safety: we know there is a next point to unwrap in the loop because of the `is_some_and` check.
    #[allow(clippy::missing_panics_doc)]
    while next
        .as_ref()
        .is_some_and(|curr| curr.sample_offset <= samples_to_skip)
    {
        prev = next.unwrap();
        next = i.next();
    }

    // Note that precision loss is acceptable here because the curve reconstruction is
    // only approximate.
    #[allow(clippy::cast_precision_loss)]
    let new_value_at_zero = if let Some(curr) = next.as_ref() {
        let delta = curr.value - prev.value;
        let sample_delta = curr.sample_offset - prev.sample_offset;
        // Safety: sample_delta is guaranteed to be non-zero due to the invariants of `PiecewiseLinearCurve`,
        // i.e., there are no two points on the same sample offset.
        let delta_per_sample = delta / sample_delta as f32;
        let remaining_samples_to_skip = samples_to_skip - prev.sample_offset;
        prev.value + delta_per_sample * remaining_samples_to_skip as f32
    } else {
        prev.value
    };
    let new_point_at_zero = PiecewiseLinearCurvePoint {
        sample_offset: 0,
        value: new_value_at_zero,
    };
    std::iter::once(new_point_at_zero).chain(next.into_iter().chain(i).map(move |p| {
        PiecewiseLinearCurvePoint {
            sample_offset: p.sample_offset - samples_to_skip,
            value: p.value,
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn piecewise_linear_skip_0() {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 100,
                    value: 1.0,
                },
            ]
            .into_iter(),
            128,
            0.0..=1.0,
        )
        .unwrap();
        let points = skip_piecewise_linear(curve, 0).collect::<Vec<_>>();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].sample_offset, 0);
        assert_eq!(points[1].sample_offset, 100);
        assert_approx_eq!(points[0].value, 0.0);
        assert_approx_eq!(points[1].value, 1.0);
    }

    #[test]
    fn piecewise_linear_skip_all() {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 100,
                    value: 1.0,
                },
            ]
            .into_iter(),
            128,
            0.0..=1.0,
        )
        .unwrap();
        let points = skip_piecewise_linear(curve, 110).collect::<Vec<_>>();
        assert_eq!(points.len(), 1);
        assert_eq!(points[0].sample_offset, 0);
        assert_approx_eq!(points[0].value, 1.0);
    }

    #[test]
    fn piecewise_linear_skip_to_point() {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 50,
                    value: 0.75,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 100,
                    value: 1.0,
                },
            ]
            .into_iter(),
            128,
            0.0..=1.0,
        )
        .unwrap();
        let points = skip_piecewise_linear(curve, 50).collect::<Vec<_>>();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].sample_offset, 0);
        assert_eq!(points[1].sample_offset, 50);
        assert_approx_eq!(points[0].value, 0.75);
        assert_approx_eq!(points[1].value, 1.0);
    }

    #[test]
    fn piecewise_linear_skip_before_point() {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 50,
                    value: 0.5,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 100,
                    value: 1.0,
                },
            ]
            .into_iter(),
            128,
            0.0..=1.0,
        )
        .unwrap();
        let points = skip_piecewise_linear(curve, 49).collect::<Vec<_>>();
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].sample_offset, 0);
        assert_eq!(points[1].sample_offset, 1);
        assert_eq!(points[2].sample_offset, 51);
        assert_approx_eq!(points[0].value, 0.49);
        assert_approx_eq!(points[1].value, 0.5);
        assert_approx_eq!(points[2].value, 1.0);
    }

    #[test]
    fn piecewise_linear_skip_after_point() {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 50,
                    value: 0.5,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 100,
                    value: 1.0,
                },
            ]
            .into_iter(),
            128,
            0.0..=1.0,
        )
        .unwrap();
        let points = skip_piecewise_linear(curve, 51).collect::<Vec<_>>();
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].sample_offset, 0);
        assert_eq!(points[1].sample_offset, 49);
        assert_approx_eq!(points[0].value, 0.51);
        assert_approx_eq!(points[1].value, 1.0);
    }
}
