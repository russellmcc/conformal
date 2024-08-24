use super::{
    hash_id, IdHash, InternalValue, PiecewiseLinearCurve, PiecewiseLinearCurvePoint, States,
};

struct MyState {}
impl States for MyState {
    fn get_by_hash(&self, param_hash: IdHash) -> Option<InternalValue> {
        if param_hash == hash_id("numeric") {
            return Some(InternalValue::Numeric(0.5));
        } else if param_hash == hash_id("enum") {
            return Some(InternalValue::Enum(2));
        } else if param_hash == hash_id("switch") {
            return Some(InternalValue::Switch(true));
        } else {
            return None;
        }
    }
}

#[test]
fn parameter_states_default_functions() {
    let state = MyState {};
    assert_eq!(state.get_numeric("numeric"), Some(0.5));
    assert_eq!(state.get_numeric("enum"), None);
    assert_eq!(state.get_enum("numeric"), None);
    assert_eq!(state.get_enum("enum"), Some(2));
    assert_eq!(state.get_switch("switch"), Some(true));
    assert_eq!(state.get_switch("numeric"), None);
}

#[test]
fn valid_curve() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 0,
                value: 0.5
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 3,
                value: 0.4
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 4,
                value: 0.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_some())
}

#[test]
fn out_of_order_curve_points_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 0,
                value: 0.5
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 4,
                value: 0.4
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 3,
                value: 0.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_none())
}

#[test]
fn empty_curves_rejected() {
    assert!(PiecewiseLinearCurve::new((&[]).iter().cloned(), 10, 0.0..=1.0).is_none())
}

#[test]
fn zero_length_buffers_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[PiecewiseLinearCurvePoint {
            sample_offset: 0,
            value: 0.2
        }])
            .iter()
            .cloned(),
        0,
        0.0..=1.0
    )
    .is_none())
}

#[test]
fn out_of_bounds_sample_counts_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 0,
                value: 0.2
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 12,
                value: 0.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_none())
}

#[test]
fn out_of_bounds_curve_values_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 0,
                value: 0.2
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 3,
                value: 1.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_none())
}

#[test]
fn curve_does_not_start_at_zero_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 3,
                value: 0.5
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 6,
                value: 0.4
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 7,
                value: 0.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_none())
}

#[test]
fn curve_multiple_points_same_sample_rejected() {
    assert!(PiecewiseLinearCurve::new(
        (&[
            PiecewiseLinearCurvePoint {
                sample_offset: 0,
                value: 0.5
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 6,
                value: 0.4
            },
            PiecewiseLinearCurvePoint {
                sample_offset: 6,
                value: 0.3
            }
        ])
            .iter()
            .cloned(),
        10,
        0.0..=1.0
    )
    .is_none())
}
