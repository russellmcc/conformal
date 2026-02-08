use std::{iter::Peekable, ops::RangeInclusive};

use conformal_component::parameters::{
    NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint,
};

#[derive(Clone)]
pub struct TimedStateChange<I> {
    pub sample_offset: usize,
    pub state: NumericBufferState<I>,
}

#[derive(Clone)]
enum Fragment<I: Iterator<Item: Clone>> {
    Constant {
        value: f32,
        sample_offset: usize,
    },
    Varying {
        points: Peekable<I>,
        last: Option<PiecewiseLinearCurvePoint>,
    },
}

#[derive(Clone)]
struct SpliceImpl<I: Iterator<Item: Clone>, J: Iterator<Item: Clone>> {
    next_transition_point: Option<PiecewiseLinearCurvePoint>,
    next_initial_point: Option<PiecewiseLinearCurvePoint>,
    next_fragment: Option<Fragment<I>>,
    timed_state_changes: Peekable<J>,
}

// Note that precision loss is acceptable here because the spliced curve
// is only intended to be approximate.
#[allow(clippy::cast_precision_loss)]
fn interpolate_piecewise_linear(
    point_before: &PiecewiseLinearCurvePoint,
    point_after: &PiecewiseLinearCurvePoint,
    interpolation_offset: usize,
) -> PiecewiseLinearCurvePoint {
    // Check preconditions
    assert!(interpolation_offset >= point_before.sample_offset,);
    assert!(interpolation_offset <= point_after.sample_offset,);
    assert!(point_before.sample_offset < point_after.sample_offset,);

    let delta = point_after.value - point_before.value;
    let sample_delta = point_after.sample_offset - point_before.sample_offset;
    // Safety: sample_delta is guaranteed to be non-zero due to the preconditions checked above,
    // so this won't be a divide by zero.
    let delta_per_sample = delta / sample_delta as f32;
    let value = point_before.value
        + delta_per_sample * (interpolation_offset - point_before.sample_offset) as f32;
    PiecewiseLinearCurvePoint {
        sample_offset: interpolation_offset,
        value,
    }
}

/// Skip `samples_to_skip` samples from the start of an iterator of piecewise linear curve points,
/// and return the "initial" point if needed. `iter` will be advanced so that the next result yielded,
/// if any, will be >= `samples_to_skip`.
fn skip_piecewise_curve<I: IntoIterator<Item = PiecewiseLinearCurvePoint> + Clone>(
    curve: PiecewiseLinearCurve<I>,
    samples_to_skip: usize,
) -> (Peekable<I::IntoIter>, Option<PiecewiseLinearCurvePoint>) {
    // Check pre-conditions
    assert!(samples_to_skip > 0);

    let mut iter = curve.into_iter().peekable();
    let mut last = None;
    while iter
        .peek()
        .is_some_and(|p| p.sample_offset < samples_to_skip)
    {
        last = iter.next();
    }

    let initial_point = if let Some(next) = iter.peek() {
        if next.sample_offset == samples_to_skip {
            // We had a curve point already exactly at the transition point!
            // this means we don't need a separate initial point.
            None
        } else {
            // Our next curve point is after the transition point, so we need
            // a synthetic initial point exactly at the transition point.
            Some(interpolate_piecewise_linear(
                // Safety: we know we have a "last" point due to:
                // 1. `PiecewiseLinearCurve`s have a guaranteed point at sample offset 0.
                // 2. `samples_to_skip` must be more than 0.
                last.as_ref().unwrap(),
                next,
                samples_to_skip,
            ))
        }
    } else {
        // This meant we have no points after the transition point!
        // So we need to return an initial point with the same value as the last point.
        Some(PiecewiseLinearCurvePoint {
            sample_offset: samples_to_skip,
            value: last.unwrap().value,
        })
    };
    (iter, initial_point)
}

impl<
    I: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    J: Iterator<Item = TimedStateChange<I>> + Clone,
> SpliceImpl<I, J>
{
    /// Move on to the next queued fragment, returns true if it needs
    /// to be called again (this lets us skip fully constant fragments)
    fn advance_fragment_inner(
        &mut self,
        last_point_of_old_fragment: &PiecewiseLinearCurvePoint,
        future_point_of_old_fragment: Option<&PiecewiseLinearCurvePoint>,
    ) -> bool {
        const EPSILON: f32 = 1e-6;
        // Defensively check precondition
        if self.next_transition_point.is_some() {
            unreachable!(
                "Internal programming error: not allowed to call advance_fragment while a transition point is queued!"
            );
        }

        if let Some(next_change) = self.timed_state_changes.next() {
            // check to see if we can constant-coalesce.
            if let NumericBufferState::Constant(v) = next_change.state
                && future_point_of_old_fragment.is_none()
                && (last_point_of_old_fragment.value - v).abs() < EPSILON
            {
                return true;
            }

            // Decide if we need a transition point. We need a transition point if the last
            // point of the old fragment was further back than 1 sample before the transition.
            // Otherwise, we don't need a transition point.
            if last_point_of_old_fragment.sample_offset + 1 < next_change.sample_offset {
                assert!(next_change.sample_offset > 0); // This should be true due to the invariants for the state change curve.
                let transition_sample_offset = next_change.sample_offset - 1;

                self.next_transition_point = Some(match future_point_of_old_fragment {
                    Some(future_point) => interpolate_piecewise_linear(
                        last_point_of_old_fragment,
                        future_point,
                        transition_sample_offset,
                    ),
                    None => PiecewiseLinearCurvePoint {
                        sample_offset: transition_sample_offset,
                        value: last_point_of_old_fragment.value,
                    },
                });
            }

            // Regardless of if we have a transition point, we must prepare the new fragment.
            self.next_fragment = match next_change.state {
                NumericBufferState::Constant(v) => Some(Fragment::Constant {
                    value: v,
                    sample_offset: next_change.sample_offset,
                }),
                NumericBufferState::PiecewiseLinear(v) => {
                    let (mut points, initial_point) =
                        skip_piecewise_curve(v, next_change.sample_offset);

                    // check for special case curve is constant from the start of the fragment,
                    // with the same value as the previous fragment. In this case, we can
                    // skip the fragment.
                    if points.peek().is_none()
                        && initial_point.is_some_and(|p| {
                            (p.value - last_point_of_old_fragment.value).abs() < EPSILON
                        })
                        && future_point_of_old_fragment.is_none()
                    {
                        self.next_transition_point = None;
                        return true;
                    }

                    self.next_initial_point = initial_point;
                    Some(Fragment::Varying {
                        points,
                        last: initial_point,
                    })
                }
            };
        } else {
            // We're done with state changes, and there's no next fragment to output!
            self.next_fragment = None;
        }
        false
    }

    fn advance_fragment(
        &mut self,
        last_point_of_old_fragment: &PiecewiseLinearCurvePoint,
        future_point_of_old_fragment: Option<&PiecewiseLinearCurvePoint>,
    ) {
        while self.advance_fragment_inner(last_point_of_old_fragment, future_point_of_old_fragment)
        {
        }
    }
}

impl<
    I: Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
    J: Iterator<Item = TimedStateChange<I>> + Clone,
> Iterator for SpliceImpl<I, J>
{
    type Item = PiecewiseLinearCurvePoint;

    fn next(&mut self) -> Option<Self::Item> {
        // If we have a queued transition point, emit it and return.
        if let Some(ret) = self.next_transition_point.take() {
            return Some(ret);
        }

        // If we have a queued initial point, emit it and return.
        if let Some(ret) = self.next_initial_point.take() {
            return Some(ret);
        }

        // Otherwise, work on the next fragment.
        match self.next_fragment.as_mut() {
            Some(Fragment::Constant {
                value,
                sample_offset,
            }) => {
                let ret = PiecewiseLinearCurvePoint {
                    sample_offset: *sample_offset,
                    value: *value,
                };

                self.advance_fragment(&ret, None);
                Some(ret)
            }
            Some(Fragment::Varying { points, last }) => {
                match (points.next(), self.timed_state_changes.peek()) {
                    (Some(next_point), None) => {
                        // No more transitions coming up, so just play this curve out until the end.
                        // Note that in this case, we'll never look at `last` again, so we don't have to update it here.
                        Some(next_point)
                    }
                    (None, None) => {
                        // We exhausted the last fragment and we're totally done.
                        self.next_fragment = None;
                        None
                    }
                    (None, Some(_)) => {
                        // This curve finished before the next transition point!

                        // Safety: we'll always have a last value here, because
                        // Each varying fragment either has an initial point or at least one point.
                        let last = last.unwrap();

                        self.advance_fragment(&last, None);

                        // Recurse to output the first part of the next fragment
                        // This won't infinite recurse because we're guaranteed not to have
                        // more than one state change per sample.
                        self.next()
                    }
                    (Some(next_point), Some(next_change)) => {
                        // We have more points in this curve, and more changes.
                        // What to do depends on which comes first.

                        if next_point.sample_offset < next_change.sample_offset {
                            *last = Some(next_point);
                            Some(next_point)
                        } else {
                            // Safety: we'll always have a last value here, because
                            // Each varying fragment either has an initial point or a point at the last transition point.
                            // since there can't be two changes per sample, this means we'll always have a last value here.
                            let last_point = last.unwrap();
                            self.advance_fragment(&last_point, Some(&next_point));

                            // We recurse to output the first part of the next fragment.
                            // This won't infinite recurse because we're guaranteed not to have
                            // more than one state change per sample.
                            self.next()
                        }
                    }
                }
            }
            None => None,
        }
    }
}

fn spliced_piecewise_linear_curve<I: Iterator<Item = PiecewiseLinearCurvePoint> + Clone>(
    initial_state: NumericBufferState<I>,
    timed_state_changes: impl Iterator<Item = TimedStateChange<I>> + Clone,
) -> impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone {
    SpliceImpl {
        next_fragment: Some(match initial_state {
            NumericBufferState::Constant(v) => Fragment::Constant {
                value: v,
                sample_offset: 0,
            },
            NumericBufferState::PiecewiseLinear(v) => Fragment::Varying {
                points: v.into_iter().peekable(),
                last: None,
            },
        }),
        timed_state_changes: timed_state_changes.peekable(),
        next_transition_point: None,
        next_initial_point: None,
    }
}

// Checks if the spliced curve is fully constant. If so, returns the constant value.
fn maybe_get_fully_constant_value<I: Iterator<Item = PiecewiseLinearCurvePoint> + Clone>(
    initial_state: &NumericBufferState<I>,
    timed_state_changes: impl Iterator<Item = TimedStateChange<I>>,
) -> Option<f32> {
    const EPSILON: f32 = 1e-6;
    if let NumericBufferState::Constant(v) = initial_state {
        for TimedStateChange { state, .. } in timed_state_changes {
            match state {
                NumericBufferState::Constant(v2) if (v - v2).abs() > EPSILON => {
                    return None;
                }
                NumericBufferState::PiecewiseLinear(_) => return None,
                NumericBufferState::Constant(_) => {}
            }
        }
        Some(*v)
    } else {
        None
    }
}

/// Helper for a tricky case related to per-note expressions.
///
/// In particular, in cases where the note a voice is playing changes within
/// a single buffer, we need to be able to splice together the expression
/// buffer states for all the notes involved.
///
/// This function splices together multiple `NumericBufferState`s into a single `NumericBufferState`.
///
/// # Preconditions
///
///  - `timed_state_changes` is sorted by sample offset.
///  - `timed_state_changes` does _not_ have a change at sample offset 0.
///  - `timed_state_changes` does _not_ have more than one change per sample.
///  - All `NumericBufferState`s involved are scoped to the given `buffer_size` and `valid_range`.
///
/// # Panics
///
///  - Only panics if invariants above are violated.
pub fn splice_numeric_buffer_states<I: Iterator<Item = PiecewiseLinearCurvePoint> + Clone>(
    initial_state: NumericBufferState<I>,
    timed_state_changes: impl Iterator<Item = TimedStateChange<I>> + Clone,
    buffer_size: usize,
    valid_range: RangeInclusive<f32>,
) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
    // Optimistically check if this is a fully constant curve.
    //
    // This case is surprisingly common, and it saves a lot of time downstream
    // to return a constant here, so it's worth the cost of checking if we can
    // get away with this.
    if let Some(v) = maybe_get_fully_constant_value(&initial_state, timed_state_changes.clone()) {
        return NumericBufferState::Constant(v);
    }

    let points = spliced_piecewise_linear_curve(initial_state, timed_state_changes);

    // Note that it's expensive to check the curve invariants here, which we guarantee in our implementation, so
    // we only check them in debug mode.
    debug_assert!(
        PiecewiseLinearCurve::new(points.clone(), buffer_size, valid_range.clone()).is_some()
    );

    // Safety: our implementation is supposed to guarantee the invariants.
    NumericBufferState::PiecewiseLinear(unsafe {
        PiecewiseLinearCurve::from_parts_unchecked(points, buffer_size)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_approx_eq::assert_approx_eq;

    #[test]
    fn test_spliced_piecewise_linear_curve_two_constants() {
        let initial_state =
            NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(0.0);
        let result = spliced_piecewise_linear_curve(
            initial_state,
            vec![TimedStateChange {
                sample_offset: 50,
                state: NumericBufferState::Constant(1.0),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 49);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 50);
        assert_approx_eq!(result[2].value, 1.0);
    }

    #[test]
    fn test_spliced_varying_fragment_with_next_change_before_first_curve_point() {
        let initial_state = NumericBufferState::Constant(0.0);
        let result = spliced_piecewise_linear_curve(
            initial_state,
            vec![
                TimedStateChange {
                    sample_offset: 10,
                    state: NumericBufferState::PiecewiseLinear(
                        PiecewiseLinearCurve::new(
                            vec![
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 0,
                                    value: 0.0,
                                },
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 50,
                                    value: 1.0,
                                },
                            ]
                            .into_iter(),
                            64,
                            0.0..=1.0,
                        )
                        .unwrap(),
                    ),
                },
                TimedStateChange {
                    sample_offset: 15,
                    state: NumericBufferState::Constant(0.5),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 9);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 10);
        assert_approx_eq!(result[2].value, 0.2);
        assert_eq!(result[3].sample_offset, 14);
        assert_approx_eq!(result[3].value, 0.28, 1e-4);
        assert_eq!(result[4].sample_offset, 15);
        assert_approx_eq!(result[4].value, 0.5);
    }

    #[test]
    fn test_splice_all_same_constants_returns_constant() {
        let result = splice_numeric_buffer_states(
            NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(0.5),
            vec![
                TimedStateChange {
                    sample_offset: 10,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 30,
                    state: NumericBufferState::Constant(0.5),
                },
            ]
            .into_iter(),
            64,
            0.0..=1.0,
        );
        assert!(matches!(result, NumericBufferState::Constant(v) if (v - 0.5).abs() < 1e-6));
    }

    #[test]
    fn test_constant_no_changes() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(0.5),
            std::iter::empty(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.5);
    }

    #[test]
    fn test_varying_no_changes() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::PiecewiseLinear(
                PiecewiseLinearCurve::new(
                    vec![
                        PiecewiseLinearCurvePoint {
                            sample_offset: 0,
                            value: 0.0,
                        },
                        PiecewiseLinearCurvePoint {
                            sample_offset: 50,
                            value: 1.0,
                        },
                    ]
                    .into_iter(),
                    64,
                    0.0..=1.0,
                )
                .unwrap(),
            ),
            std::iter::empty(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 50);
        assert_approx_eq!(result[1].value, 1.0);
    }

    #[test]
    fn test_constant_to_varying() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::Constant(0.0),
            vec![TimedStateChange {
                sample_offset: 20,
                state: NumericBufferState::PiecewiseLinear(
                    PiecewiseLinearCurve::new(
                        vec![
                            PiecewiseLinearCurvePoint {
                                sample_offset: 0,
                                value: 0.0,
                            },
                            PiecewiseLinearCurvePoint {
                                sample_offset: 50,
                                value: 1.0,
                            },
                        ]
                        .into_iter(),
                        64,
                        0.0..=1.0,
                    )
                    .unwrap(),
                ),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 19);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 20);
        assert_approx_eq!(result[2].value, 0.4);
        assert_eq!(result[3].sample_offset, 50);
        assert_approx_eq!(result[3].value, 1.0);
    }

    #[test]
    fn test_varying_to_constant() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::PiecewiseLinear(
                PiecewiseLinearCurve::new(
                    vec![
                        PiecewiseLinearCurvePoint {
                            sample_offset: 0,
                            value: 0.0,
                        },
                        PiecewiseLinearCurvePoint {
                            sample_offset: 50,
                            value: 1.0,
                        },
                    ]
                    .into_iter(),
                    64,
                    0.0..=1.0,
                )
                .unwrap(),
            ),
            vec![TimedStateChange {
                sample_offset: 25,
                state: NumericBufferState::Constant(0.8),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 24);
        assert_approx_eq!(result[1].value, 0.48);
        assert_eq!(result[2].sample_offset, 25);
        assert_approx_eq!(result[2].value, 0.8);
    }

    #[test]
    fn test_adjacent_constant_change() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(0.0),
            vec![TimedStateChange {
                sample_offset: 1,
                state: NumericBufferState::Constant(1.0),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 1);
        assert_approx_eq!(result[1].value, 1.0);
    }

    #[test]
    fn test_three_different_constants() {
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(0.0),
            vec![
                TimedStateChange {
                    sample_offset: 20,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 40,
                    state: NumericBufferState::Constant(1.0),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 19);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 20);
        assert_approx_eq!(result[2].value, 0.5);
        assert_eq!(result[3].sample_offset, 39);
        assert_approx_eq!(result[3].value, 0.5);
        assert_eq!(result[4].sample_offset, 40);
        assert_approx_eq!(result[4].value, 1.0);
    }

    #[test]
    fn test_varying_to_varying() {
        // Old curve: 0.0 at 0 -> 1.0 at 100 (slope = 0.01/sample)
        // Splice at 50, so transition point at 49 = interp(0->100, 49) = 0.49
        // New curve: 0.0 at 0 -> 2.0 at 100 (same slope), skip to 50 -> initial = 1.0
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::PiecewiseLinear(
                PiecewiseLinearCurve::new(
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
                .unwrap(),
            ),
            vec![TimedStateChange {
                sample_offset: 50,
                state: NumericBufferState::PiecewiseLinear(
                    PiecewiseLinearCurve::new(
                        vec![
                            PiecewiseLinearCurvePoint {
                                sample_offset: 0,
                                value: 0.0,
                            },
                            PiecewiseLinearCurvePoint {
                                sample_offset: 100,
                                value: 2.0,
                            },
                        ]
                        .into_iter(),
                        128,
                        0.0..=2.0,
                    )
                    .unwrap(),
                ),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 49);
        assert_approx_eq!(result[1].value, 0.49);
        assert_eq!(result[2].sample_offset, 50);
        assert_approx_eq!(result[2].value, 1.0);
        assert_eq!(result[3].sample_offset, 100);
        assert_approx_eq!(result[3].value, 2.0);
    }

    #[test]
    fn test_exact_curve_point_at_splice_boundary() {
        // The new curve has a point exactly at the splice offset (50),
        // so skip_piecewise_curve returns initial_point = None.
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::Constant(0.0),
            vec![TimedStateChange {
                sample_offset: 50,
                state: NumericBufferState::PiecewiseLinear(
                    PiecewiseLinearCurve::new(
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
                    .unwrap(),
                ),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 49);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 50);
        assert_approx_eq!(result[2].value, 0.5);
        assert_eq!(result[3].sample_offset, 100);
        assert_approx_eq!(result[3].value, 1.0);
    }

    #[test]
    fn test_constant_coalesce_then_non_matching_change() {
        // Constant(0.5) coalesces with Constant(0.5) at 10,
        // then a different PiecewiseLinear at 20 is not coalesced.
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::Constant(0.5),
            vec![
                TimedStateChange {
                    sample_offset: 10,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 20,
                    state: NumericBufferState::PiecewiseLinear(
                        PiecewiseLinearCurve::new(
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
                        .unwrap(),
                    ),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.5);
        assert_eq!(result[1].sample_offset, 19);
        assert_approx_eq!(result[1].value, 0.5);
        assert_eq!(result[2].sample_offset, 20);
        assert_approx_eq!(result[2].value, 0.2);
        assert_eq!(result[3].sample_offset, 100);
        assert_approx_eq!(result[3].value, 1.0);
    }

    #[test]
    fn test_varying_with_multiple_points_before_splice() {
        // Old curve has several points, splice happens mid-curve.
        // 0.0 at 0, 0.25 at 25, 0.5 at 50, 0.75 at 75.
        // Splice at 60 → transition at 59 = interp(50:0.5 → 75:0.75, 59) = 0.59
        let result = spliced_piecewise_linear_curve(
            NumericBufferState::PiecewiseLinear(
                PiecewiseLinearCurve::new(
                    vec![
                        PiecewiseLinearCurvePoint {
                            sample_offset: 0,
                            value: 0.0,
                        },
                        PiecewiseLinearCurvePoint {
                            sample_offset: 25,
                            value: 0.25,
                        },
                        PiecewiseLinearCurvePoint {
                            sample_offset: 50,
                            value: 0.5,
                        },
                        PiecewiseLinearCurvePoint {
                            sample_offset: 75,
                            value: 0.75,
                        },
                    ]
                    .into_iter(),
                    128,
                    0.0..=1.0,
                )
                .unwrap(),
            ),
            vec![TimedStateChange {
                sample_offset: 60,
                state: NumericBufferState::Constant(1.0),
            }]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 5);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 25);
        assert_approx_eq!(result[1].value, 0.25);
        assert_eq!(result[2].sample_offset, 50);
        assert_approx_eq!(result[2].value, 0.5);
        assert_eq!(result[3].sample_offset, 59);
        assert_approx_eq!(result[3].value, 0.59);
        assert_eq!(result[4].sample_offset, 60);
        assert_approx_eq!(result[4].value, 1.0);
    }

    #[test]
    fn test_partial_constant_coalescing_from_curve() {
        let initial_state = NumericBufferState::PiecewiseLinear(
            PiecewiseLinearCurve::new(
                vec![
                    PiecewiseLinearCurvePoint {
                        sample_offset: 0,
                        value: 0.0,
                    },
                    PiecewiseLinearCurvePoint {
                        sample_offset: 10,
                        value: 0.5,
                    },
                ]
                .into_iter(),
                64,
                0.0..=1.0,
            )
            .unwrap(),
        );
        let result = spliced_piecewise_linear_curve(
            initial_state,
            vec![
                TimedStateChange {
                    sample_offset: 20,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 35,
                    state: NumericBufferState::Constant(0.5),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 10);
        assert_approx_eq!(result[1].value, 0.5);
    }

    #[test]
    fn test_partial_constant_coalescing_from_const() {
        let initial_state = NumericBufferState::Constant(0.5);
        let result = spliced_piecewise_linear_curve(
            initial_state,
            vec![
                TimedStateChange {
                    sample_offset: 20,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 50,
                    state: NumericBufferState::PiecewiseLinear(
                        PiecewiseLinearCurve::new(
                            vec![
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 0,
                                    value: 0.0,
                                },
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 100,
                                    value: 2.0,
                                },
                            ]
                            .into_iter(),
                            128,
                            0.0..=2.0,
                        )
                        .unwrap(),
                    ),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 4);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.5);
        assert_eq!(result[1].sample_offset, 49);
        assert_approx_eq!(result[1].value, 0.5);
        assert_eq!(result[2].sample_offset, 50);
        assert_approx_eq!(result[2].value, 1.0);
        assert_eq!(result[3].sample_offset, 100);
        assert_approx_eq!(result[3].value, 2.0);
    }

    #[test]
    fn test_partial_constant_coalescing_from_expired_curve() {
        let initial_state = NumericBufferState::Constant(0.0);
        let result = spliced_piecewise_linear_curve(
            initial_state,
            vec![
                TimedStateChange {
                    sample_offset: 20,
                    state: NumericBufferState::Constant(0.5),
                },
                TimedStateChange {
                    sample_offset: 50,
                    state: NumericBufferState::PiecewiseLinear(
                        PiecewiseLinearCurve::new(
                            vec![
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 0,
                                    value: 0.0,
                                },
                                PiecewiseLinearCurvePoint {
                                    sample_offset: 25,
                                    value: 0.5,
                                },
                            ]
                            .into_iter(),
                            64,
                            0.0..=1.0,
                        )
                        .unwrap(),
                    ),
                },
            ]
            .into_iter(),
        )
        .collect::<Vec<_>>();
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].sample_offset, 0);
        assert_approx_eq!(result[0].value, 0.0);
        assert_eq!(result[1].sample_offset, 19);
        assert_approx_eq!(result[1].value, 0.0);
        assert_eq!(result[2].sample_offset, 20);
        assert_approx_eq!(result[2].value, 0.5);
    }
}
