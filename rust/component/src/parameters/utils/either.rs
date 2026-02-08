use itertools::Either;

use crate::parameters::{NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint};

/// Wraps a [`NumericBufferState<A>`] as a [`NumericBufferState<Either<A, B>>`]
/// by placing the inner iterator on the `Left` side.
///
/// Useful when you need to return a single [`NumericBufferState`] type
/// but the underlying buffer may come from one of two differently-typed sources.
pub fn left_numeric_buffer<
    A: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    B: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
>(
    state: NumericBufferState<A>,
) -> NumericBufferState<Either<A, B>> {
    match state {
        NumericBufferState::Constant(value) => NumericBufferState::Constant(value),
        NumericBufferState::PiecewiseLinear(curve) => {
            let buffer_size = curve.buffer_size();
            // Note we're sure that `curve` is valid, so so must be Either::Left(curve)
            NumericBufferState::PiecewiseLinear(unsafe {
                PiecewiseLinearCurve::from_parts_unchecked(
                    Either::Left(curve.into_iter()),
                    buffer_size,
                )
            })
        }
    }
}

/// Wraps a [`NumericBufferState<B>`] as a [`NumericBufferState<Either<A, B>>`]
/// by placing the inner iterator on the `Right` side.
///
/// Useful when you need to return a single [`NumericBufferState`] type
/// but the underlying buffer may come from one of two differently-typed sources.
pub fn right_numeric_buffer<
    A: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    B: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
>(
    state: NumericBufferState<B>,
) -> NumericBufferState<Either<A, B>> {
    match state {
        NumericBufferState::Constant(value) => NumericBufferState::Constant(value),
        NumericBufferState::PiecewiseLinear(curve) => {
            let buffer_size = curve.buffer_size();
            NumericBufferState::PiecewiseLinear(unsafe {
                // Note we're sure that `curve` is valid, so so must be Either::Right(curve)
                PiecewiseLinearCurve::from_parts_unchecked(
                    Either::Right(curve.into_iter()),
                    buffer_size,
                )
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parameters::{PiecewiseLinearCurve, PiecewiseLinearCurvePoint};

    type Iter = std::vec::IntoIter<PiecewiseLinearCurvePoint>;

    fn make_curve() -> PiecewiseLinearCurve<Iter> {
        let curve = PiecewiseLinearCurve::new(
            vec![
                PiecewiseLinearCurvePoint {
                    sample_offset: 0,
                    value: 0.0,
                },
                PiecewiseLinearCurvePoint {
                    sample_offset: 50,
                    value: 1.0,
                },
            ],
            128,
            0.0..=1.0,
        )
        .unwrap();
        let buffer_size = curve.buffer_size();
        unsafe { PiecewiseLinearCurve::from_parts_unchecked(curve.into_iter(), buffer_size) }
    }

    #[test]
    fn left_constant_preserves_value() {
        let state: NumericBufferState<Iter> = NumericBufferState::Constant(0.5);
        let result = left_numeric_buffer::<_, Iter>(state);
        assert_eq!(result.value_at_start_of_buffer(), 0.5);
    }

    #[test]
    fn right_constant_preserves_value() {
        let state: NumericBufferState<Iter> = NumericBufferState::Constant(0.75);
        let result = right_numeric_buffer::<Iter, _>(state);
        assert_eq!(result.value_at_start_of_buffer(), 0.75);
    }

    #[test]
    fn left_piecewise_linear_preserves_points() {
        let state = NumericBufferState::PiecewiseLinear(make_curve());
        let result = left_numeric_buffer::<_, Iter>(state);
        match result {
            NumericBufferState::PiecewiseLinear(curve) => {
                assert_eq!(curve.buffer_size(), 128);
                let points: Vec<_> = curve.into_iter().collect();
                assert_eq!(points.len(), 2);
                assert_eq!(points[0].value, 0.0);
                assert_eq!(points[1].value, 1.0);
            }
            _ => panic!("expected PiecewiseLinear"),
        }
    }

    #[test]
    fn right_piecewise_linear_preserves_points() {
        let state = NumericBufferState::PiecewiseLinear(make_curve());
        let result = right_numeric_buffer::<Iter, _>(state);
        match result {
            NumericBufferState::PiecewiseLinear(curve) => {
                assert_eq!(curve.buffer_size(), 128);
                let points: Vec<_> = curve.into_iter().collect();
                assert_eq!(points.len(), 2);
                assert_eq!(points[0].value, 0.0);
                assert_eq!(points[1].value, 1.0);
            }
            _ => panic!("expected PiecewiseLinear"),
        }
    }
}
