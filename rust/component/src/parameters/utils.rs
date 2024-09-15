#[cfg(test)]
mod tests;

use std::{
    collections::HashMap,
    ops::{Range, RangeInclusive},
};

use crate::synth::CONTROLLER_PARAMETERS;

use super::{
    hash_id, BufferState, BufferStates, EnumBufferState, IdHash, InfoRef, InternalValue,
    NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint, States, SwitchBufferState,
    TimedEnumValues, TimedSwitchValues, TimedValue, TypeSpecificInfoRef,
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

/// Converts a [`NumericBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
///
/// # Example
///
/// ```
/// # use conformal_component::parameters::{numeric_per_sample, NumericBufferState, PiecewiseLinearCurvePoint, PiecewiseLinearCurve };
/// # use conformal_component::audio::all_approx_eq;
/// let state = NumericBufferState::PiecewiseLinear(PiecewiseLinearCurve::new(
///   vec![
///     PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
///     PiecewiseLinearCurvePoint { sample_offset: 10, value: 1.0 },
///   ],
///   13,
///   0.0..=1.0,
/// ).unwrap());
/// assert!(
///   all_approx_eq(
///     numeric_per_sample(state),
///     [0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.0, 1.0],
///     1e-6
///   )
/// );
/// ```
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

/// Converts an [`EnumBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
///
/// # Example
///
/// ```
/// # use conformal_component::parameters::{enum_per_sample, EnumBufferState, TimedEnumValues, TimedValue };
/// let state = EnumBufferState::Varying(TimedEnumValues::new(
///   vec![
///     TimedValue { sample_offset: 0, value: 0 },
///     TimedValue { sample_offset: 3, value: 1 },
///   ],
///   5,
///   0..2,
/// ).unwrap());
/// assert!(
///   enum_per_sample(state).eq([0, 0, 0, 1, 1].iter().cloned())
/// );
/// ```
pub fn enum_per_sample<I: IntoIterator<Item = TimedValue<u32>, IntoIter: Clone>>(
    state: EnumBufferState<I>,
) -> impl Iterator<Item = u32> + Clone {
    match state {
        EnumBufferState::Constant(v) => ConstantOrIterating::Constant(v),
        EnumBufferState::Varying(c) => ConstantOrIterating::Iterating(timed_enum_per_sample(c)),
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

/// Converts a [`SwitchBufferState`] into a per-sample iterator.
///
/// This provides the value of the parameter at each sample in the buffer.
///
/// # Example
///
/// ```
/// # use conformal_component::parameters::{switch_per_sample, SwitchBufferState, TimedSwitchValues, TimedValue };
/// let state = SwitchBufferState::Varying(TimedSwitchValues::new(
///   vec![
///     TimedValue { sample_offset: 0, value: false },
///     TimedValue { sample_offset: 3, value: true },
///   ],
///   5,
/// ).unwrap());
/// assert!(
///   switch_per_sample(state).eq([false, false, false, true, true].iter().cloned())
/// );
/// ```
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

#[derive(Clone, Debug, Default)]
pub struct ConstantBufferStates<S> {
    s: S,
}

impl<S: States> BufferStates for ConstantBufferStates<S> {
    fn get_by_hash(
        &self,
        id_hash: IdHash,
    ) -> std::option::Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        match self.s.get_by_hash(id_hash) {
            Some(InternalValue::Numeric(n)) => {
                Some(BufferState::Numeric(NumericBufferState::<
                    std::iter::Empty<PiecewiseLinearCurvePoint>,
                >::Constant(n)))
            }
            Some(InternalValue::Enum(e)) => Some(BufferState::Enum(EnumBufferState::<
                std::iter::Empty<TimedValue<u32>>,
            >::Constant(e))),
            Some(InternalValue::Switch(s)) => Some(BufferState::Switch(SwitchBufferState::<
                std::iter::Empty<TimedValue<bool>>,
            >::Constant(s))),
            None => None,
        }
    }
}

impl<S: States> ConstantBufferStates<S> {
    pub fn new(s: S) -> Self {
        Self { s }
    }
}

#[derive(Clone, Debug, Default)]
pub struct StatesMap {
    map: HashMap<IdHash, InternalValue>,
}

impl<S: AsRef<str>> From<HashMap<S, InternalValue>> for StatesMap {
    fn from(map: HashMap<S, InternalValue>) -> Self {
        Self {
            map: map
                .into_iter()
                .map(|(k, v)| (hash_id(k.as_ref()), v))
                .collect(),
        }
    }
}

#[derive(Clone, Debug)]
enum RampedState {
    Constant(InternalValue),
    RampedNumeric {
        start: f32,
        end: f32,
        range: RangeInclusive<f32>,
    },
    RampedEnum {
        start: u32,
        end: u32,
        range: Range<u32>,
    },
    RampedSwitch {
        start: bool,
        end: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct RampedStatesMap {
    buffer_size: usize,
    map: HashMap<IdHash, RampedState>,
}

impl RampedStatesMap {
    pub fn new<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        start_overrides: &HashMap<&'_ str, InternalValue>,
        end_overrides: &HashMap<&'_ str, InternalValue>,
        buffer_size: usize,
    ) -> Self {
        let map = infos
            .into_iter()
            .map(|info| {
                let id = hash_id(info.unique_id);
                let value = match (
                    info.type_specific,
                    start_overrides.get(info.unique_id),
                    end_overrides.get(info.unique_id),
                ) {
                    (
                        TypeSpecificInfoRef::Numeric { valid_range, .. },
                        Some(InternalValue::Numeric(start)),
                        Some(InternalValue::Numeric(end)),
                    ) => {
                        if start == end {
                            RampedState::Constant(InternalValue::Numeric(*start))
                        } else {
                            RampedState::RampedNumeric {
                                start: *start,
                                end: *end,
                                range: valid_range,
                            }
                        }
                    }
                    (
                        TypeSpecificInfoRef::Numeric {
                            default,
                            valid_range,
                            ..
                        },
                        None,
                        Some(InternalValue::Numeric(end)),
                    ) => RampedState::RampedNumeric {
                        start: default,
                        end: *end,
                        range: valid_range,
                    },
                    (
                        TypeSpecificInfoRef::Numeric {
                            default,
                            valid_range,
                            ..
                        },
                        Some(InternalValue::Numeric(start)),
                        None,
                    ) => RampedState::RampedNumeric {
                        start: *start,
                        end: default,
                        range: valid_range,
                    },
                    (TypeSpecificInfoRef::Numeric { default, .. }, None, None) => {
                        RampedState::Constant(InternalValue::Numeric(default))
                    }
                    (
                        TypeSpecificInfoRef::Enum { values, .. },
                        Some(InternalValue::Enum(start)),
                        Some(InternalValue::Enum(end)),
                    ) => {
                        if start == end {
                            RampedState::Constant(InternalValue::Enum(*start))
                        } else {
                            RampedState::RampedEnum {
                                start: *start,
                                end: *end,
                                range: 0..values.len() as u32,
                            }
                        }
                    }
                    (
                        TypeSpecificInfoRef::Enum {
                            default, values, ..
                        },
                        None,
                        Some(InternalValue::Enum(end)),
                    ) => RampedState::RampedEnum {
                        start: default,
                        end: *end,
                        range: 0..values.len() as u32,
                    },
                    (
                        TypeSpecificInfoRef::Enum {
                            default, values, ..
                        },
                        Some(InternalValue::Enum(start)),
                        None,
                    ) => RampedState::RampedEnum {
                        start: *start,
                        end: default,
                        range: 0..values.len() as u32,
                    },
                    (TypeSpecificInfoRef::Enum { default, .. }, None, None) => {
                        RampedState::Constant(InternalValue::Enum(default))
                    }
                    (
                        TypeSpecificInfoRef::Switch { .. },
                        Some(InternalValue::Switch(start)),
                        Some(InternalValue::Switch(end)),
                    ) => {
                        if start == end {
                            RampedState::Constant(InternalValue::Switch(*start))
                        } else {
                            RampedState::RampedSwitch {
                                start: *start,
                                end: *end,
                            }
                        }
                    }
                    (
                        TypeSpecificInfoRef::Switch { default },
                        None,
                        Some(InternalValue::Switch(end)),
                    ) => RampedState::RampedSwitch {
                        start: default,
                        end: *end,
                    },
                    (
                        TypeSpecificInfoRef::Switch { default },
                        Some(InternalValue::Switch(start)),
                        None,
                    ) => RampedState::RampedSwitch {
                        start: *start,
                        end: default,
                    },
                    (TypeSpecificInfoRef::Switch { default }, None, None) => {
                        RampedState::Constant(InternalValue::Switch(default))
                    }
                    _ => panic!(),
                };
                (id, value)
            })
            .collect();
        Self { buffer_size, map }
    }

    pub fn new_const<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self::new(infos, overrides, overrides, 0)
    }
}

fn ramp_numeric(
    start: f32,
    end: f32,
    buffer_size: usize,
) -> impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone {
    return [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size };
        PiecewiseLinearCurvePoint {
            sample_offset,
            value,
        }
    });
}

fn ramp_enum(
    start: u32,
    end: u32,
    buffer_size: usize,
) -> impl Iterator<Item = TimedValue<u32>> + Clone {
    return [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size / 2 };
        TimedValue {
            sample_offset,
            value,
        }
    });
}

fn ramp_switch(
    start: bool,
    end: bool,
    buffer_size: usize,
) -> impl Iterator<Item = TimedValue<bool>> + Clone {
    return [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size / 2 };
        TimedValue {
            sample_offset,
            value,
        }
    });
}

impl BufferStates for RampedStatesMap {
    fn get_by_hash(
        &self,
        id_hash: IdHash,
    ) -> std::option::Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        let param = self.map.get(&id_hash)?;
        match param {
            RampedState::Constant(value) => match value {
                InternalValue::Numeric(n) => {
                    Some(BufferState::Numeric(NumericBufferState::Constant(*n)))
                }
                InternalValue::Enum(e) => Some(BufferState::Enum(EnumBufferState::Constant(*e))),
                InternalValue::Switch(s) => {
                    Some(BufferState::Switch(SwitchBufferState::Constant(*s)))
                }
            },
            RampedState::RampedNumeric { start, end, range } => Some(BufferState::Numeric(
                NumericBufferState::PiecewiseLinear(PiecewiseLinearCurve::new(
                    ramp_numeric(*start, *end, self.buffer_size),
                    self.buffer_size,
                    range.clone(),
                )?),
            )),
            RampedState::RampedEnum { start, end, range } => Some(BufferState::Enum(
                EnumBufferState::Varying(TimedEnumValues::new(
                    ramp_enum(*start, *end, self.buffer_size),
                    self.buffer_size,
                    range.clone(),
                )?),
            )),
            RampedState::RampedSwitch { start, end } => Some(BufferState::Switch(
                SwitchBufferState::Varying(TimedSwitchValues::new(
                    ramp_switch(*start, *end, self.buffer_size),
                    self.buffer_size,
                )?),
            )),
        }
    }
}

pub fn override_defaults<'a, S: AsRef<str> + 'a>(
    infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
    overrides: &HashMap<&'_ str, InternalValue>,
) -> HashMap<String, InternalValue> {
    HashMap::from_iter(infos.into_iter().map(|info| {
        let id = info.unique_id;
        let value = overrides
            .get(id)
            .cloned()
            .unwrap_or_else(|| match info.type_specific {
                TypeSpecificInfoRef::Enum { default, .. } => InternalValue::Enum(default),
                TypeSpecificInfoRef::Numeric { default, .. } => InternalValue::Numeric(default),
                TypeSpecificInfoRef::Switch { default, .. } => InternalValue::Switch(default),
            });
        (id.to_string(), value)
    }))
}

pub fn override_synth_defaults<'a, 'b: 'a>(
    infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
    overrides: &HashMap<&'_ str, InternalValue>,
) -> HashMap<String, InternalValue> {
    override_defaults(infos.into_iter().chain(CONTROLLER_PARAMETERS), overrides)
}

impl States for StatesMap {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue> {
        self.map.get(&id_hash).cloned()
    }
}
