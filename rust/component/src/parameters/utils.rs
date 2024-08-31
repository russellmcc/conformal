//! This module contains utilities to help consume parameters easier.

#[cfg(test)]
mod tests;

use super::{
    EnumBufferState, NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint,
    SwitchBufferState, TimedEnumValues, TimedSwitchValues, TimedValue,
};

#[derive(Clone)]
enum ConstantOrIterating<V, I> {
    Constant(V),
    Iterating(I),
}

impl<V: Copy, I: Iterator<Item = V>> Iterator for ConstantOrIterating<V, I> {
    type Item = V;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ConstantOrIterating::Constant(v) => Some(*v),
            ConstantOrIterating::Iterating(i) => i.next(),
        }
    }
}

/// Convert a piecewise linear curve into a per-sample iterator for a buffer.
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

/// Helper for getting the value of a parameter at every sample offset
pub fn numeric_per_sample<I: IntoIterator<Item = PiecewiseLinearCurvePoint, IntoIter: Clone>>(
    state: NumericBufferState<I>,
) -> impl Iterator<Item = f32> + Clone {
    match state {
        NumericBufferState::Constant(v) => ConstantOrIterating::Constant(v),
        NumericBufferState::PiecewiseLinear(c) => {
            ConstantOrIterating::Iterating(piecewise_linear_curve_per_sample(c))
        }
    }
}

#[allow(clippy::missing_panics_doc)] // We only panic when invariants are broken.
pub fn timed_enum_per_sample<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
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

pub fn enum_per_sample<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
    state: EnumBufferState<I>,
) -> impl Iterator<Item = u32> + Clone {
    match state {
        EnumBufferState::Constant(v) => ConstantOrIterating::Constant(v),
        EnumBufferState::Varying(c) => ConstantOrIterating::Iterating(timed_enum_per_sample(c)),
    }
}

#[allow(clippy::missing_panics_doc)] // We only panic when invariants are broken.
pub fn timed_switch_per_sample<I: IntoIterator<Item = TimedValue<bool>, IntoIter: Clone>>(
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

pub fn switch_per_sample<I: IntoIterator<Item = TimedValue<bool>, IntoIter: Clone>>(
    state: SwitchBufferState<I>,
) -> impl Iterator<Item = bool> + Clone {
    match state {
        SwitchBufferState::Constant(v) => ConstantOrIterating::Constant(v),
        SwitchBufferState::Varying(c) => ConstantOrIterating::Iterating(timed_switch_per_sample(c)),
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! pzip_part {
    (numeric $path:literal $params:ident) => {
        conformal_component::parameters::utils::numeric_per_sample(
            $params.get_numeric($path).unwrap(),
        )
    };
    (enum $path:literal $params:ident) => {
        conformal_component::parameters::utils::enum_per_sample($params.get_enum($path).unwrap())
    };
    (switch $path:literal $params:ident) => {
        conformal_component::parameters::utils::switch_per_sample(
            $params.get_switch($path).unwrap(),
        )
    };
}

// Optimization opportunity - add maps here that only apply to the control points
// in the linear curves!

#[macro_export]
macro_rules! pzip {
    ($params:ident[$($kind:ident $path:literal),+]) => {
        conformal_component::itertools::izip!(
            $(
                conformal_component::pzip_part!($kind $path $params),
            )+
        )
    };
}
