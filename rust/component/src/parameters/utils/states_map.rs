use std::{collections::HashMap, hash::BuildHasher};

use crate::{
    events::NoteID,
    synth::{
        NumericGlobalExpression, NumericPerNoteExpression, SwitchGlobalExpression, SynthParamStates,
    },
};

use super::super::{IdHash, InfoRef, InternalValue, States, TypeSpecificInfoRef, hash_id};

/// Helper function to get a map of param values based on the default values from a list of `Info`s.
///
/// Note that if you are passing these parameters to a synth, likely
/// you want to use [`override_synth_defaults`] instead.
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, override_defaults};
/// # use std::collections::HashMap;
/// let infos = vec![
///    StaticInfoRef {
///      title: "Numeric",
///      short_title: "Numeric",
///      unique_id: "numeric",
///      flags: Default::default(),
///      type_specific: TypeSpecificInfoRef::Numeric {
///        default: 0.0,
///        valid_range: 0.0..=1.0,
///        units: None,
///      },
///    },
/// ];
///
/// // Without overriding, we'll just get a map containing
/// // the default values.
/// assert_eq!(
///   override_defaults(infos.iter().cloned(), &HashMap::new()).get("numeric"),
///   Some(&InternalValue::Numeric(0.0))
/// );
///
/// // If we override the default value, we'll get that instead.
/// assert_eq!(
///   override_defaults(
///     infos.iter().cloned(),
///     &vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect::<HashMap<_, _>>()
///   ).get("numeric"),
///   Some(&InternalValue::Numeric(0.5))
///  );
/// ```
pub fn override_defaults<'a, S: AsRef<str> + 'a, H: BuildHasher>(
    infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
    overrides: &HashMap<&'_ str, InternalValue, H>,
) -> HashMap<String, InternalValue> {
    infos
        .into_iter()
        .map(|info| {
            let id = info.unique_id;
            let value = overrides
                .get(id)
                .copied()
                .unwrap_or(match info.type_specific {
                    TypeSpecificInfoRef::Enum { default, .. } => InternalValue::Enum(default),
                    TypeSpecificInfoRef::Numeric { default, .. } => InternalValue::Numeric(default),
                    TypeSpecificInfoRef::Switch { default, .. } => InternalValue::Switch(default),
                });
            (id.to_string(), value)
        })
        .collect()
}

/// A simple implementation of [`States`] that is backed by a [`HashMap`].
///
/// This is useful for testing or other places when you want to pass a [`States`]
/// to a component outside of a Conformal wrapper.
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

impl StatesMap {
    /// Create a new [`StatesMap`] from a list of `Info`s and `override`s.
    ///
    /// This creates a `StatesMap` with all parameters set to default values,
    /// except for the ones that are overridden by the `override`s.
    ///
    /// Note that if you want to pass this into a synth, you should use
    /// [`SynthStatesMap::new_override_defaults`] instead.
    ///
    /// `overrides` work exactly as in [`override_defaults`].
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, StatesMap, States};
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
    ///
    /// let states = StatesMap::new_override_defaults(infos.iter().cloned(), &overrides);
    /// assert_eq!(states.get_numeric("numeric"), Some(0.5));
    /// ```
    pub fn new_override_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self {
            map: override_defaults(infos, overrides)
                .into_iter()
                .map(|(k, v)| (hash_id(&k), v))
                .collect(),
        }
    }

    /// Create a new [`StatesMap`] from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value.
    ///
    /// Note that if you want to pass this into a synth, you should use
    /// [`Self::new_synth_defaults`] instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, StatesMap, States};
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
    /// let states = StatesMap::new_defaults(infos.iter().cloned());
    /// assert_eq!(states.get_numeric("numeric"), Some(0.0));
    /// ```
    pub fn new_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
    ) -> Self {
        Self::new_override_defaults(infos, &Default::default())
    }
}

impl States for StatesMap {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue> {
        self.map.get(&id_hash).copied()
    }
}

/// A simple implementation of [`SynthParamStates`] that is backed by a [`HashMap`].
///
/// This is useful for testing or other places when you want to pass a [`SynthParamStates`]
/// to a component outside of a Conformal wrapper.
#[derive(Clone, Debug, Default)]
pub struct SynthStatesMap {
    states: StatesMap,
    numeric_expressions: HashMap<NumericGlobalExpression, f32>,
    switch_expressions: HashMap<SwitchGlobalExpression, bool>,
    per_note_expressions: HashMap<(NumericPerNoteExpression, NoteID), f32>,
}

impl SynthStatesMap {
    /// Create a new [`SynthStatesMap`] to pass to a synth from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`StatesMap::new_override_defaults`], but it also includes expression controllers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthStatesMap, States};
    /// # use conformal_component::synth::{SynthParamStates, NumericGlobalExpression};
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
    /// let overrides = vec![
    ///   // You can override declared parameters
    ///   ("numeric", InternalValue::Numeric(0.5)),
    /// ].into_iter().collect();
    /// let numeric_expression_overrides = vec![
    ///   // Or you can override control parameters
    ///   (NumericGlobalExpression::ModWheel, 0.2),
    /// ].into_iter().collect();
    /// let states = SynthStatesMap::new_override_defaults(infos.iter().cloned(), &overrides, &numeric_expression_overrides, &Default::default());
    ///
    /// // Overridden parameters get the values you passed in
    /// assert_eq!(states.get_numeric("numeric"), Some(0.5));
    /// assert_eq!(states.get_numeric_global_expression(NumericGlobalExpression::ModWheel), 0.2);
    ///
    /// // Other parameters get their default values
    /// assert_eq!(states.get_numeric_global_expression(NumericGlobalExpression::PitchBend), 0.0);
    /// ```
    pub fn new_override_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
        numeric_expression_overrides: &HashMap<NumericGlobalExpression, f32>,
        switch_expression_overrides: &HashMap<SwitchGlobalExpression, bool>,
    ) -> Self {
        Self {
            states: StatesMap::from(override_defaults(infos, overrides)),
            numeric_expressions: numeric_expression_overrides.clone(),
            switch_expressions: switch_expression_overrides.clone(),
            per_note_expressions: Default::default(),
        }
    }

    /// Create a new [`SynthStatesMap`] with per-note expression overrides.
    ///
    /// This is similar to [`Self::new_override_defaults`], but also allows specifying
    /// per-note expression values for specific notes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthStatesMap, States};
    /// # use conformal_component::synth::{SynthParamStates, NumericGlobalExpression, NumericPerNoteExpression};
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
    /// let per_note_overrides = vec![
    ///   ((NumericPerNoteExpression::PitchBend, note_id), 1.5),
    /// ].into_iter().collect();
    ///
    /// let states = SynthStatesMap::new_with_per_note(
    ///   infos.iter().cloned(),
    ///   &Default::default(),
    ///   &Default::default(),
    ///   &Default::default(),
    ///   &per_note_overrides,
    /// );
    ///
    /// assert_eq!(states.get_numeric_expression_for_note(NumericPerNoteExpression::PitchBend, note_id), 1.5);
    /// ```
    pub fn new_with_per_note<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
        numeric_expression_overrides: &HashMap<NumericGlobalExpression, f32>,
        switch_expression_overrides: &HashMap<SwitchGlobalExpression, bool>,
        per_note_expression_overrides: &HashMap<(NumericPerNoteExpression, NoteID), f32>,
    ) -> Self {
        Self {
            states: StatesMap::from(override_defaults(infos, overrides)),
            numeric_expressions: numeric_expression_overrides.clone(),
            switch_expressions: switch_expression_overrides.clone(),
            per_note_expressions: per_note_expression_overrides.clone(),
        }
    }

    /// Create a new [`SynthStatesMap`] to pass to a synth from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value.
    ///
    /// This is similar to [`StatesMap::new_defaults`], but it also includes expression controllers.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, SynthStatesMap, States};
    /// # use conformal_component::synth::{SynthParamStates, NumericGlobalExpression};
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
    /// let states = SynthStatesMap::new_defaults(infos.iter().cloned());
    /// assert_eq!(states.get_numeric("numeric"), Some(0.0));
    ///
    /// // Controller parameters will also be included
    /// assert_eq!(states.get_numeric_global_expression(NumericGlobalExpression::ModWheel), 0.0);
    /// ```
    pub fn new_defaults<'a, S: AsRef<str> + 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
    ) -> Self {
        Self::new_override_defaults(
            infos,
            &Default::default(),
            &Default::default(),
            &Default::default(),
        )
    }
}

impl States for SynthStatesMap {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue> {
        self.states.get_by_hash(id_hash)
    }
}

impl SynthParamStates for SynthStatesMap {
    fn get_numeric_global_expression(&self, expression: NumericGlobalExpression) -> f32 {
        self.numeric_expressions
            .get(&expression)
            .copied()
            .unwrap_or_default()
    }
    fn get_switch_global_expression(&self, expression: SwitchGlobalExpression) -> bool {
        self.switch_expressions
            .get(&expression)
            .copied()
            .unwrap_or_default()
    }

    fn get_numeric_expression_for_note(
        &self,
        expression: NumericPerNoteExpression,
        note_id: NoteID,
    ) -> f32 {
        self.per_note_expressions
            .get(&(expression, note_id))
            .copied()
            .unwrap_or_default()
    }
}
