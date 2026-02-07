use std::collections::HashMap;

use crate::{
    events::NoteID,
    synth::{
        NumericGlobalExpression, NumericPerNoteExpression, SwitchGlobalExpression,
        SynthParamBufferStates, SynthParamStates,
    },
};

use super::super::{
    BufferState, BufferStates, EnumBufferState, InternalValue, NumericBufferState,
    PiecewiseLinearCurvePoint, States, SwitchBufferState, TimedValue,
};
use super::states_map::{StatesMap, SynthStatesMap};

/// Simple implementation of [`BufferStates`] trait where every parameter is
/// constant throughout the whole buffer.
///
/// This is in general useful for testing or other scenarios where you need
/// to create a [`BufferStates`] object outside of a Conformal wrapper.
#[derive(Clone, Debug, Default)]
pub struct ConstantBufferStates<S> {
    s: S,
}

impl<S: States> BufferStates for ConstantBufferStates<S> {
    fn get_by_hash(
        &self,
        id_hash: super::super::IdHash,
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

impl<S: SynthParamStates> SynthParamBufferStates for ConstantBufferStates<S> {
    fn get_numeric_global_expression(
        &self,
        expression: NumericGlobalExpression,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(
            self.s.get_numeric_global_expression(expression),
        )
    }

    fn get_switch_global_expression(
        &self,
        expression: SwitchGlobalExpression,
    ) -> SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone> {
        SwitchBufferState::<std::iter::Empty<TimedValue<bool>>>::Constant(
            self.s.get_switch_global_expression(expression),
        )
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
    ) -> NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone> {
        NumericBufferState::<std::iter::Empty<PiecewiseLinearCurvePoint>>::Constant(
            self.s.get_numeric_expression_for_note(expression, note_id),
        )
    }
}

impl<S: States> ConstantBufferStates<S> {
    /// Create a new [`ConstantBufferStates`] object from a [`States`] object.
    pub fn new(s: S) -> Self {
        Self { s }
    }
}

impl ConstantBufferStates<StatesMap> {
    /// Create a new [`ConstantBufferStates`] object from a list of `Info`s and `override`s.
    ///
    /// This creates a `ConstantBufferStates` with all parameters set to default values
    /// for the whole buffer.
    ///
    /// Note that if you want to pass this into a synth, you should use
    /// [`Self::new_override_synth_defaults`] instead.
    ///
    /// `overrides` work exactly as in [`override_defaults`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState};
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
    /// let buffer_states = ConstantBufferStates::new_override_defaults(infos, &overrides);
    /// match buffer_states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    /// ```
    pub fn new_override_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = super::super::InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self::new(StatesMap::new_override_defaults(infos, overrides))
    }

    /// Create a new [`ConstantBufferStates`] object from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value for the whole buffer.
    ///
    /// Note that if you want to pass this into a synth, you should use
    /// [`Self::new_synth_defaults`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState};
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
    /// let buffer_states = ConstantBufferStates::new_defaults(infos);
    /// match buffer_states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = super::super::InfoRef<'a, S>> + 'a,
    ) -> Self {
        Self::new_override_defaults(infos, &Default::default())
    }
}

impl ConstantBufferStates<SynthStatesMap> {
    /// Create a new [`ConstantBufferStates`] object to pass to a synth from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`Self::new_override_defaults`], but it also includes expression controllers.
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState, SynthStatesMap};
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
    /// let overrides = vec![
    ///   // You can override declared parameters
    ///   ("numeric", InternalValue::Numeric(0.5)),
    /// ].into_iter().collect();
    ///
    /// // and you can override control parameters
    /// let expression_overrides = vec![
    ///   (NumericGlobalExpression::ModWheel, 0.2),
    /// ].into_iter().collect();
    ///
    /// let buffer_states = ConstantBufferStates::new_override_synth_defaults(infos, &overrides, &expression_overrides, &Default::default());
    ///
    /// // Overridden parameters get the values you passed in
    /// match buffer_states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    /// match buffer_states.get_numeric_global_expression(NumericGlobalExpression::ModWheel) {
    ///   NumericBufferState::Constant(0.2) => (),
    ///   _ => panic!("Expected constant value of 0.2"),
    /// };
    ///
    /// // Other parameters get their default values
    /// match buffer_states.get_numeric_global_expression(NumericGlobalExpression::PitchBend) {
    ///   NumericBufferState::Constant(0.0) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_override_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = super::super::InfoRef<'a, &'b str>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
        numeric_expression_overrides: &HashMap<NumericGlobalExpression, f32>,
        switch_expression_overrides: &HashMap<SwitchGlobalExpression, bool>,
    ) -> Self {
        Self::new(SynthStatesMap::new_override_defaults(
            infos,
            overrides,
            numeric_expression_overrides,
            switch_expression_overrides,
        ))
    }

    /// Create a new [`ConstantBufferStates`] object to pass to a synth from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value for the whole buffer.
    ///
    /// This is similar to [`Self::new_defaults`], but it also includes expression controllers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState, SynthStatesMap};
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
    /// let buffer_states = ConstantBufferStates::new_synth_defaults(infos);
    /// match buffer_states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// match buffer_states.get_numeric_global_expression(NumericGlobalExpression::ModWheel) {
    ///   NumericBufferState::Constant(0.0) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = super::super::InfoRef<'a, &'b str>> + 'a,
    ) -> Self {
        Self::new_override_synth_defaults(
            infos,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
    }
}
