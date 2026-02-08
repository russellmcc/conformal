use std::{
    collections::{HashMap, HashSet},
    hash::BuildHasher,
    ops::{Range, RangeInclusive},
};

use crate::{
    audio::approx_eq,
    events::NoteID,
    synth::{
        NumericGlobalExpression, NumericPerNoteExpression, SwitchGlobalExpression,
        SynthParamBufferStates, valid_range_for_per_note_expression,
    },
};

use super::super::{
    BufferState, BufferStates, EnumBufferState, IdHash, InfoRef, InternalValue, NumericBufferState,
    PiecewiseLinearCurve, PiecewiseLinearCurvePoint, SwitchBufferState, TimedEnumValues,
    TimedSwitchValues, TimedValue, TypeSpecificInfoRef, hash_id,
};

#[derive(Clone, Debug)]
struct RampedNumeric {
    start: f32,
    end: f32,
    range: RangeInclusive<f32>,
}

#[derive(Clone, Debug)]
struct RampedEnum {
    start: u32,
    end: u32,
    range: Range<u32>,
}

#[derive(Clone, Debug)]
struct RampedSwitch {
    start: bool,
    end: bool,
}

#[derive(Clone, Debug)]
enum RampedState {
    Constant(InternalValue),
    Numeric(RampedNumeric),
    Enum(RampedEnum),
    Switch(RampedSwitch),
}

/// A simple implementation of a [`BufferStates`] that allows
/// for parameters to change between the start and end of a buffer.
///
/// Each parameter can be either constant or ramped between two values.
///
/// For numeric parameters, the ramp is linear, for other parameter types
/// the value changes half-way through the buffer.
#[derive(Clone, Debug, Default)]
pub struct RampedStatesMap {
    buffer_size: usize,
    map: HashMap<IdHash, RampedState>,
}

fn ramped_numeric(start: f32, end: f32, range: RangeInclusive<f32>) -> RampedState {
    if approx_eq(start, end, 1e-6) {
        RampedState::Constant(InternalValue::Numeric(start))
    } else {
        RampedState::Numeric(RampedNumeric { start, end, range })
    }
}

fn ramped_enum(start: u32, end: u32, num_values: usize) -> RampedState {
    if start == end {
        RampedState::Constant(InternalValue::Enum(start))
    } else {
        RampedState::Enum(RampedEnum {
            start,
            end,
            range: 0..u32::try_from(num_values).unwrap(),
        })
    }
}

fn ramped_switch(start: bool, end: bool) -> RampedState {
    if start == end {
        RampedState::Constant(InternalValue::Switch(start))
    } else {
        RampedState::Switch(RampedSwitch { start, end })
    }
}

fn ramp_for_numeric(
    default: f32,
    valid_range: RangeInclusive<f32>,
    start_override: Option<InternalValue>,
    end_override: Option<InternalValue>,
) -> RampedState {
    let start = match start_override {
        Some(InternalValue::Numeric(v)) => v,
        None => default,
        _ => panic!(),
    };
    let end = match end_override {
        Some(InternalValue::Numeric(v)) => v,
        None => default,
        _ => panic!(),
    };
    ramped_numeric(start, end, valid_range)
}

fn ramp_for_enum(
    default: u32,
    num_values: usize,
    start_override: Option<InternalValue>,
    end_override: Option<InternalValue>,
) -> RampedState {
    let start = match start_override {
        Some(InternalValue::Enum(v)) => v,
        None => default,
        _ => panic!(),
    };
    let end = match end_override {
        Some(InternalValue::Enum(v)) => v,
        None => default,
        _ => panic!(),
    };
    ramped_enum(start, end, num_values)
}

fn ramp_for_switch(
    default: bool,
    start_override: Option<InternalValue>,
    end_override: Option<InternalValue>,
) -> RampedState {
    let start = match start_override {
        Some(InternalValue::Switch(v)) => v,
        None => default,
        _ => panic!(),
    };
    let end = match end_override {
        Some(InternalValue::Switch(v)) => v,
        None => default,
        _ => panic!(),
    };
    ramped_switch(start, end)
}

impl RampedStatesMap {
    /// Constructor that creates a `RampedStatesMap`
    /// from a list of `Info`s and `override`s.at the start and end of the buffer.
    ///
    /// These overrides work the same way as in [`override_defaults`].
    ///
    /// Note for a synth, you should use [`SynthRampedStatesMap::new`] instead.
    ///
    /// # Examples
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, RampedStatesMap, NumericBufferState, BufferStates};
    /// # use std::collections::HashMap;
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    ///
    /// let start_overrides: HashMap<_, _> = vec![].into_iter().collect();
    /// let end_overrides: HashMap<_, _> = vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect();
    /// let states = RampedStatesMap::new(infos.iter().cloned(), &start_overrides, &end_overrides, 10);
    ///
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::PiecewiseLinear(_)) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `start_overrides` or `end_overrides` do not match the type of the parameter
    /// specified in `infos`.
    ///
    /// Also panics if any of the enum parameters in `infos` has a number of values
    /// that will not fit into a `u32`.
    pub fn new<'a, S: AsRef<str> + 'a, H: BuildHasher, H_: BuildHasher>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        start_overrides: &HashMap<&'_ str, InternalValue, H>,
        end_overrides: &HashMap<&'_ str, InternalValue, H_>,
        buffer_size: usize,
    ) -> Self {
        let map = infos
            .into_iter()
            .map(|info| {
                let id = hash_id(info.unique_id);
                let start_override = start_overrides.get(info.unique_id);
                let end_override = end_overrides.get(info.unique_id);
                let value = match info.type_specific {
                    TypeSpecificInfoRef::Numeric {
                        default,
                        valid_range,
                        ..
                    } => ramp_for_numeric(
                        default,
                        valid_range,
                        start_override.copied(),
                        end_override.copied(),
                    ),
                    TypeSpecificInfoRef::Enum {
                        default, values, ..
                    } => ramp_for_enum(
                        default,
                        values.len(),
                        start_override.copied(),
                        end_override.copied(),
                    ),
                    TypeSpecificInfoRef::Switch { default } => {
                        ramp_for_switch(default, start_override.copied(), end_override.copied())
                    }
                };
                (id, value)
            })
            .collect();
        Self { buffer_size, map }
    }

    /// Helper to make a `RampedStatesMap` with all parameters constant.
    ///
    /// This is useful for _performance_ testing because while the parameters
    /// are constant at run-time, the `RampedStatesMap` has the ability to
    /// ramp between values, so consumers cannot be specialized to handle constant
    /// values only
    ///
    /// Note that if you want to pass this into a synth, you should use [`Self::new_const_synth`]
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, RampedStatesMap, NumericBufferState, BufferStates};
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    ///
    /// let overrides = vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect();
    /// let states = RampedStatesMap::new_const(infos.iter().cloned(), &overrides);
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    /// ```
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
    [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size - 1 };
        PiecewiseLinearCurvePoint {
            sample_offset,
            value,
        }
    })
}

fn ramp_enum(
    start: u32,
    end: u32,
    buffer_size: usize,
) -> impl Iterator<Item = TimedValue<u32>> + Clone {
    [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size / 2 };
        TimedValue {
            sample_offset,
            value,
        }
    })
}

fn ramp_switch(
    start: bool,
    end: bool,
    buffer_size: usize,
) -> impl Iterator<Item = TimedValue<bool>> + Clone {
    [0, 1].iter().map(move |i| {
        let value = if *i == 0 { start } else { end };
        let sample_offset = if *i == 0 { 0 } else { buffer_size / 2 };
        TimedValue {
            sample_offset,
            value,
        }
    })
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
            RampedState::Numeric(RampedNumeric { start, end, range }) => {
                Some(BufferState::Numeric(NumericBufferState::PiecewiseLinear(
                    PiecewiseLinearCurve::new(
                        ramp_numeric(*start, *end, self.buffer_size),
                        self.buffer_size,
                        range.clone(),
                    )?,
                )))
            }
            RampedState::Enum(RampedEnum { start, end, range }) => Some(BufferState::Enum(
                EnumBufferState::Varying(TimedEnumValues::new(
                    ramp_enum(*start, *end, self.buffer_size),
                    self.buffer_size,
                    range.clone(),
                )?),
            )),
            RampedState::Switch(RampedSwitch { start, end }) => Some(BufferState::Switch(
                SwitchBufferState::Varying(TimedSwitchValues::new(
                    ramp_switch(*start, *end, self.buffer_size),
                    self.buffer_size,
                )?),
            )),
        }
    }
}

fn valid_range_for_numeric_global_expression(
    expression: NumericGlobalExpression,
) -> RangeInclusive<f32> {
    match expression {
        NumericGlobalExpression::PitchBend => -1.0..=1.0,
        NumericGlobalExpression::Timbre
        | NumericGlobalExpression::Aftertouch
        | NumericGlobalExpression::ExpressionPedal
        | NumericGlobalExpression::ModWheel => 0.0..=1.0,
    }
}

fn ramp_numeric_expressions(
    start_overrides: &HashMap<NumericGlobalExpression, f32>,
    end_overrides: &HashMap<NumericGlobalExpression, f32>,
) -> HashMap<NumericGlobalExpression, RampedState> {
    let all_expressions = start_overrides
        .keys()
        .chain(end_overrides.keys())
        .collect::<HashSet<_>>();
    all_expressions
        .into_iter()
        .map(|expression| {
            (
                *expression,
                ramp_for_numeric(
                    Default::default(),
                    valid_range_for_numeric_global_expression(*expression),
                    start_overrides
                        .get(expression)
                        .copied()
                        .map(InternalValue::Numeric),
                    end_overrides
                        .get(expression)
                        .copied()
                        .map(InternalValue::Numeric),
                ),
            )
        })
        .collect()
}

fn ramp_switch_expressions(
    start_overrides: &HashMap<SwitchGlobalExpression, bool>,
    end_overrides: &HashMap<SwitchGlobalExpression, bool>,
) -> HashMap<SwitchGlobalExpression, RampedState> {
    let all_expressions = start_overrides
        .keys()
        .chain(end_overrides.keys())
        .collect::<HashSet<_>>();
    all_expressions
        .into_iter()
        .map(|expression| {
            (
                *expression,
                ramp_for_switch(
                    Default::default(),
                    start_overrides
                        .get(expression)
                        .copied()
                        .map(InternalValue::Switch),
                    end_overrides
                        .get(expression)
                        .copied()
                        .map(InternalValue::Switch),
                ),
            )
        })
        .collect()
}

fn ramp_per_note_expressions(
    start_overrides: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
    end_overrides: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
) -> HashMap<(NumericPerNoteExpression, NoteID), RampedState> {
    let all_keys = start_overrides
        .keys()
        .chain(end_overrides.keys())
        .collect::<HashSet<_>>();
    all_keys
        .into_iter()
        .map(|key| {
            let (expression, _) = *key;
            (
                *key,
                ramp_for_numeric(
                    Default::default(),
                    valid_range_for_per_note_expression(expression),
                    start_overrides
                        .get(key)
                        .copied()
                        .map(InternalValue::Numeric),
                    end_overrides.get(key).copied().map(InternalValue::Numeric),
                ),
            )
        })
        .collect()
}

/// A simple implementation of a [`SynthParamBufferStates`] that allows
/// for parameters to change between the start and end of a buffer.
///
/// This is similar to [`RampedStatesMap`], but it also includes the expression controller parameters
/// needed for synths.
///
/// Each parameter can be either constant or ramped between two values.
///
/// For numeric parameters, the ramp is linear, for other parameter types
/// the value changes half-way through the buffer.
pub struct SynthRampedStatesMap {
    states: RampedStatesMap,
    numeric_expressions: HashMap<NumericGlobalExpression, RampedState>,
    switch_expressions: HashMap<SwitchGlobalExpression, RampedState>,
    per_note_expressions: HashMap<(NumericPerNoteExpression, NoteID), RampedState>,
}

/// Params for [`SynthRampedStatesMap::new`]
pub struct SynthRampedOverrides<'a, 'b> {
    /// Overrides for parameters at the start of the buffer
    pub start_params: &'a HashMap<&'b str, InternalValue>,
    /// Overrides for parameters at the end of the buffer
    pub end_params: &'a HashMap<&'b str, InternalValue>,
    /// Overrides for numeric global expression controllerss at the start of the buffer
    pub start_numeric_expressions: &'a HashMap<NumericGlobalExpression, f32>,
    /// Overrides for numeric global expression controllers at the end of the buffer
    pub end_numeric_expressions: &'a HashMap<NumericGlobalExpression, f32>,
    /// Overrides for switch global expression controllers at the start of the buffer
    pub start_switch_expressions: &'a HashMap<SwitchGlobalExpression, bool>,
    /// Overrides for switch global expression controllers at the end of the buffer
    pub end_switch_expressions: &'a HashMap<SwitchGlobalExpression, bool>,
}

impl SynthRampedStatesMap {
    /// Create a new [`SynthRampedStatesMap`] for synths from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`RampedStatesMap::new`], but it also includes the expression controller parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthRampedStatesMap, NumericBufferState, BufferStates, SynthRampedOverrides};
    /// # use conformal_component::synth::{SynthParamBufferStates, NumericGlobalExpression};
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    ///
    /// let start_expression_overrides = vec![(NumericGlobalExpression::ModWheel, 1.0)].into_iter().collect();
    /// let end_param_overrides = vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect();
    /// let states = SynthRampedStatesMap::new(
    ///   infos.iter().cloned(),
    ///   SynthRampedOverrides {
    ///     start_params: &Default::default(),
    ///     end_params: &end_param_overrides,
    ///     start_numeric_expressions: &start_expression_overrides,
    ///     end_numeric_expressions: &Default::default(),
    ///     start_switch_expressions: &Default::default(),
    ///     end_switch_expressions: &Default::default(),
    ///   },
    ///   10
    /// );
    ///
    /// // If we only overrode a value at the beginning or end
    /// // it should be ramped
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::PiecewiseLinear(_)) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    /// match states.get_numeric_global_expression(NumericGlobalExpression::ModWheel) {
    ///   NumericBufferState::PiecewiseLinear(_) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    ///
    /// // Params left at default should be constants
    /// match states.get_numeric_global_expression(NumericGlobalExpression::PitchBend) {
    ///   NumericBufferState::Constant(0.0) => (),
    ///   _ => panic!("Expected a constant value"),
    /// };
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `start_overrides` or `end_overrides` do not match the type of the parameter
    /// specified in `infos`.
    pub fn new<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        SynthRampedOverrides {
            start_params,
            end_params,
            start_numeric_expressions,
            end_numeric_expressions,
            start_switch_expressions,
            end_switch_expressions,
        }: SynthRampedOverrides<'_, '_>,
        buffer_size: usize,
    ) -> Self {
        Self {
            states: RampedStatesMap::new(infos, start_params, end_params, buffer_size),
            numeric_expressions: ramp_numeric_expressions(
                start_numeric_expressions,
                end_numeric_expressions,
            ),
            switch_expressions: ramp_switch_expressions(
                start_switch_expressions,
                end_switch_expressions,
            ),
            per_note_expressions: Default::default(),
        }
    }

    /// Create a new [`SynthRampedStatesMap`] with per-note expression overrides.
    ///
    /// This is similar to [`Self::new`], but also allows specifying per-note expression
    /// values for specific notes at the start and end of the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthRampedStatesMap, NumericBufferState, BufferStates, SynthRampedOverrides};
    /// # use conformal_component::synth::{SynthParamBufferStates, NumericGlobalExpression, NumericPerNoteExpression};
    /// # use conformal_component::events::{NoteID, NoteIDInternals};
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    ///
    /// let note_id = NoteID { internals: NoteIDInternals::NoteIDFromPitch(60) };
    /// let start_per_note = vec![
    ///   ((NumericPerNoteExpression::PitchBend, note_id), 0.0),
    /// ].into_iter().collect();
    /// let end_per_note = vec![
    ///   ((NumericPerNoteExpression::PitchBend, note_id), 2.0),
    /// ].into_iter().collect();
    ///
    /// let states = SynthRampedStatesMap::new_with_per_note(
    ///   infos.iter().cloned(),
    ///   SynthRampedOverrides {
    ///     start_params: &Default::default(),
    ///     end_params: &Default::default(),
    ///     start_numeric_expressions: &Default::default(),
    ///     end_numeric_expressions: &Default::default(),
    ///     start_switch_expressions: &Default::default(),
    ///     end_switch_expressions: &Default::default(),
    ///   },
    ///   &start_per_note,
    ///   &end_per_note,
    ///   10,
    /// );
    ///
    /// match states.get_numeric_expression_for_note(NumericPerNoteExpression::PitchBend, note_id) {
    ///   NumericBufferState::PiecewiseLinear(_) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    /// ```
    pub fn new_with_per_note<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        SynthRampedOverrides {
            start_params,
            end_params,
            start_numeric_expressions,
            end_numeric_expressions,
            start_switch_expressions,
            end_switch_expressions,
        }: SynthRampedOverrides<'_, '_>,
        start_per_note_expressions: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
        end_per_note_expressions: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
        buffer_size: usize,
    ) -> Self {
        Self {
            states: RampedStatesMap::new(infos, start_params, end_params, buffer_size),
            numeric_expressions: ramp_numeric_expressions(
                start_numeric_expressions,
                end_numeric_expressions,
            ),
            switch_expressions: ramp_switch_expressions(
                start_switch_expressions,
                end_switch_expressions,
            ),
            per_note_expressions: ramp_per_note_expressions(
                start_per_note_expressions,
                end_per_note_expressions,
            ),
        }
    }

    /// Create a new [`SynthRampedStatesMap`] for synths with all parameters constant.
    ///
    /// This is useful for _performance_ testing because while the parameters
    /// are constant at run-time, the `SynthRampedStatesMap` has the ability to
    /// ramp between values, so consumers cannot be specialized to handle constant
    /// values only
    ///
    /// This is similar to [`RampedStatesMap::new_const`], but it also includes the expression controller parameters.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthRampedStatesMap, NumericBufferState, BufferStates};
    /// # use conformal_component::synth::{SynthParamBufferStates, NumericGlobalExpression};
    ///
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    /// let overrides = vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect();
    /// let states = SynthRampedStatesMap::new_const(infos.iter().cloned(), &overrides, &Default::default(), &Default::default());
    ///
    /// // Overridden parameters get the values you passed in
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    ///
    /// // Controller parameters will also be included
    /// match states.get_numeric_global_expression(NumericGlobalExpression::ModWheel) {
    ///   NumericBufferState::Constant(0.0) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_const<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
        numeric_expression_overrides: &HashMap<NumericGlobalExpression, f32>,
        switch_expression_overrides: &HashMap<SwitchGlobalExpression, bool>,
    ) -> Self {
        Self::new_with_per_note(
            infos,
            SynthRampedOverrides {
                start_params: overrides,
                end_params: overrides,
                start_numeric_expressions: numeric_expression_overrides,
                end_numeric_expressions: numeric_expression_overrides,
                start_switch_expressions: switch_expression_overrides,
                end_switch_expressions: switch_expression_overrides,
            },
            &Default::default(),
            &Default::default(),
            0,
        )
    }

    /// Create a new [`SynthRampedStatesMap`] for synths with all parameters constant,
    /// including per-note expression overrides.
    ///
    /// This is similar to [`Self::new_const`], but also allows specifying per-note
    /// expression values for specific notes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthRampedStatesMap, NumericBufferState, BufferStates};
    /// # use conformal_component::synth::{SynthParamBufferStates, NumericGlobalExpression, NumericPerNoteExpression};
    /// # use conformal_component::events::{NoteID, NoteIDInternals};
    ///
    /// let infos = vec![
    ///   StaticInfoRef {
    ///     title: "Numeric",
    ///     short_title: "Numeric",
    ///     unique_id: "numeric",
    ///     flags: Default::default(),
    ///     type_specific: TypeSpecificInfoRef::Numeric {
    ///       default: 0.0,
    ///       valid_range: 0.0..=1.0,
    ///       units: None,
    ///     },
    ///   },
    /// ];
    ///
    /// let note_id = NoteID { internals: NoteIDInternals::NoteIDFromPitch(60) };
    /// let per_note_overrides = vec![
    ///   ((NumericPerNoteExpression::PitchBend, note_id), 1.5),
    /// ].into_iter().collect();
    ///
    /// let states = SynthRampedStatesMap::new_const_with_per_note(
    ///   infos.iter().cloned(),
    ///   &Default::default(),
    ///   &Default::default(),
    ///   &Default::default(),
    ///   &per_note_overrides,
    /// );
    ///
    /// match states.get_numeric_expression_for_note(NumericPerNoteExpression::PitchBend, note_id) {
    ///   NumericBufferState::Constant(v) if (v - 1.5).abs() < 1e-6 => (),
    ///   _ => panic!("Expected constant value of 1.5"),
    /// };
    /// ```
    pub fn new_const_with_per_note<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
        numeric_expression_overrides: &HashMap<NumericGlobalExpression, f32>,
        switch_expression_overrides: &HashMap<SwitchGlobalExpression, bool>,
        per_note_expression_overrides: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
    ) -> Self {
        Self::new_with_per_note(
            infos,
            SynthRampedOverrides {
                start_params: overrides,
                end_params: overrides,
                start_numeric_expressions: numeric_expression_overrides,
                end_numeric_expressions: numeric_expression_overrides,
                start_switch_expressions: switch_expression_overrides,
                end_switch_expressions: switch_expression_overrides,
            },
            per_note_expression_overrides,
            per_note_expression_overrides,
            0,
        )
    }
}

impl BufferStates for SynthRampedStatesMap {
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
        self.states.get_by_hash(id_hash)
    }
}

impl SynthParamBufferStates for SynthRampedStatesMap {
    fn get_numeric_global_expression(
        &self,
        expression: NumericGlobalExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        match self.numeric_expressions.get(&expression) {
            Some(RampedState::Constant(InternalValue::Numeric(v))) => {
                NumericBufferState::Constant(*v)
            }
            Some(RampedState::Numeric(RampedNumeric { start, end, range })) => {
                let curve = PiecewiseLinearCurve::new(
                    ramp_numeric(*start, *end, self.states.buffer_size),
                    self.states.buffer_size,
                    range.clone(),
                );
                if let Some(curve) = curve {
                    NumericBufferState::PiecewiseLinear(curve)
                } else {
                    panic!(
                        "{start} -> {end} is not a valid ramp for {expression:?} (range: {range:?})"
                    );
                }
            }

            None => NumericBufferState::Constant(Default::default()),
            _ => unreachable!(
                "internal invariant violation: expected a numeric global expression to be either constant or ramped numeric"
            ),
        }
    }

    fn get_switch_global_expression(
        &self,
        expression: SwitchGlobalExpression,
    ) -> SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone> {
        match self.switch_expressions.get(&expression) {
            Some(RampedState::Constant(InternalValue::Switch(v))) => {
                SwitchBufferState::Constant(*v)
            }
            Some(RampedState::Switch(RampedSwitch { start, end })) => {
                let values = TimedSwitchValues::new(
                    ramp_switch(*start, *end, self.states.buffer_size),
                    self.states.buffer_size,
                );
                if let Some(values) = values {
                    SwitchBufferState::Varying(values)
                } else {
                    unreachable!(
                        "TimedSwitchValues invariant violated when ramping {expression:?} from {start} to {end}"
                    )
                }
            }
            None => SwitchBufferState::Constant(Default::default()),
            _ => unreachable!(
                "internal invariant violation: expected a switch global expression to be either constant or ramped switch"
            ),
        }
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        match self.per_note_expressions.get(&(expression, note_id)) {
            Some(RampedState::Constant(InternalValue::Numeric(v))) => {
                NumericBufferState::Constant(*v)
            }
            Some(RampedState::Numeric(RampedNumeric { start, end, range })) => {
                let curve = PiecewiseLinearCurve::new(
                    ramp_numeric(*start, *end, self.states.buffer_size),
                    self.states.buffer_size,
                    range.clone(),
                );
                if let Some(curve) = curve {
                    NumericBufferState::PiecewiseLinear(curve)
                } else {
                    panic!(
                        "{start} -> {end} is not a valid ramp for {expression:?} (range: {range:?})"
                    );
                }
            }
            None => NumericBufferState::Constant(Default::default()),
            _ => unreachable!(
                "internal invariant violation: expected a per-note expression to be either constant or ramped numeric"
            ),
        }
    }
}
