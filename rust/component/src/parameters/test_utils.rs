use std::{
    collections::HashMap,
    ops::{Range, RangeInclusive},
};

use super::{
    hash_id, BufferState, BufferStates, EnumBufferState, IdHash, InfoRef, InternalValue,
    NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint, States, SwitchBufferState,
    TimedEnumValues, TimedSwitchValues, TimedValue, TypeSpecificInfoRef,
};

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

impl States for StatesMap {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue> {
        self.map.get(&id_hash).cloned()
    }
}
