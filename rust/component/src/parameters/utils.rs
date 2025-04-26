use std::{
    collections::HashMap,
    hash::BuildHasher,
    ops::{Range, RangeInclusive},
};

use crate::{audio::approx_eq, synth::CONTROLLER_PARAMETERS};

use super::{
    BufferState, BufferStates, EnumBufferState, IdHash, InfoRef, InternalValue, NumericBufferState,
    PiecewiseLinearCurve, PiecewiseLinearCurvePoint, States, SwitchBufferState, TimedEnumValues,
    TimedSwitchValues, TimedValue, TypeSpecificInfoRef, hash_id,
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
    (numeric $path:literal $params:ident) => {{
        use conformal_component::parameters::BufferStates;
        conformal_component::parameters::numeric_per_sample($params.get_numeric($path).unwrap())
    }};
    (enum $path:literal $params:ident) => {{
        use conformal_component::parameters::BufferStates;
        conformal_component::parameters::enum_per_sample($params.get_enum($path).unwrap())
    }};
    (switch $path:literal $params:ident) => {{
        use conformal_component::parameters::BufferStates;
        conformal_component::parameters::switch_per_sample($params.get_switch($path).unwrap())
    }};
}

// Optimization opportunity - add maps here that only apply to the control points
// in the linear curves!

/// Utility to get a per-sample iterator including the state of multiple parameters.
///
/// This is a convenient way to consume a [`BufferStates`] object if you intend
/// to track the per-sample state of multiple parameters.
///
/// This macro indexes into a [`BufferStates`] object with a list of parameter
/// ids and their types. See the examples below for usage.
///
/// # Examples
///
/// ```
/// # use conformal_component::pzip;
/// # use conformal_component::parameters::{ConstantBufferStates, StaticInfoRef, TypeSpecificInfoRef, InternalValue};
/// let params = ConstantBufferStates::new_defaults(
///   vec![
///     StaticInfoRef {
///       title: "Numeric",
///       short_title: "Numeric",
///       unique_id: "gain",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Numeric {
///         default: 0.0,
///         valid_range: 0.0..=1.0,
///         units: None,
///       },
///     },
///     StaticInfoRef {
///       title: "Enum",
///       short_title: "Enum",
///       unique_id: "letter",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Enum {
///         default: 1,
///         values: &["A", "B", "C"],
///       },
///     },
///     StaticInfoRef {
///       title: "Switch",
///       short_title: "Switch",
///       unique_id: "my special switch",
///       flags: Default::default(),
///       type_specific: TypeSpecificInfoRef::Switch {
///         default: false,
///       },
///     },
///   ],
/// );
///
/// let samples: Vec<_> = pzip!(params[
///   numeric "gain",
///   enum "letter",
///   switch "my special switch"
/// ]).take(2).collect();
///
/// assert_eq!(samples, vec![(0.0, 1, false), (0.0, 1, false)]);
/// ```
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

/// Helper function to get a map of synth param values based on the default values from a list of `Info`s.
///
/// This is similar to [`override_defaults`], but it also includes the controller parameters
/// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
///
/// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, override_synth_defaults};
/// # use conformal_component::synth::MOD_WHEEL_PARAMETER;
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
/// // Without overrides, we'll get the default value.
/// assert_eq!(
///   override_synth_defaults(infos.iter().cloned(), &HashMap::new()).get("numeric"),
///   Some(&InternalValue::Numeric(0.0)),
/// );
///
/// // Note that control parameters are included in the result.
/// assert_eq!(
///   override_synth_defaults(infos.iter().cloned(), &HashMap::new()).get(MOD_WHEEL_PARAMETER),
///   Some(&InternalValue::Numeric(0.0)),
/// );
///
/// // If we override the default value of a parameter, we'll get that instead.
/// assert_eq!(
///   override_synth_defaults(
///     infos.iter().cloned(),
///     &vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect::<HashMap<_, _>>()
///   ).get("numeric"),
///   Some(&InternalValue::Numeric(0.5)),
/// );
///
/// // We can also override control parameters
/// assert_eq!(
///   override_synth_defaults(
///     infos.iter().cloned(),
///     &vec![(MOD_WHEEL_PARAMETER, InternalValue::Numeric(0.5))].into_iter().collect::<HashMap<_, _>>()
///   ).get(MOD_WHEEL_PARAMETER),
///   Some(&InternalValue::Numeric(0.5)),
/// );
/// ```
pub fn override_synth_defaults<'a, 'b: 'a, H: BuildHasher>(
    infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
    overrides: &HashMap<&'_ str, InternalValue, H>,
) -> HashMap<String, InternalValue> {
    override_defaults(infos.into_iter().chain(CONTROLLER_PARAMETERS), overrides)
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
    /// [`Self::new_override_synth_defaults`] instead.
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

    /// Create a new [`StatesMap`] to pass to a synth from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`Self::new_override_defaults`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, StatesMap, States};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER, PITCH_BEND_PARAMETER};
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
    ///   // Or you can override control parameters
    ///   (MOD_WHEEL_PARAMETER, InternalValue::Numeric(0.2)),
    /// ].into_iter().collect();
    /// let states = StatesMap::new_override_synth_defaults(infos.iter().cloned(), &overrides);
    ///
    /// // Overridden parameters get the values you passed in
    /// assert_eq!(states.get_numeric("numeric"), Some(0.5));
    /// assert_eq!(states.get_numeric(MOD_WHEEL_PARAMETER), Some(0.2));
    ///
    /// // Other parameters get their default values
    /// assert_eq!(states.get_numeric(PITCH_BEND_PARAMETER), Some(0.0));
    /// ```
    pub fn new_override_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self {
            map: override_synth_defaults(infos, overrides)
                .into_iter()
                .map(|(k, v)| (hash_id(&k), v))
                .collect(),
        }
    }

    /// Create a new [`StatesMap`] to pass to a synth from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value.
    ///
    /// This is similar to [`Self::new_defaults`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, StatesMap, States};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER};
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
    /// let states = StatesMap::new_synth_defaults(infos.iter().cloned());
    /// assert_eq!(states.get_numeric("numeric"), Some(0.0));
    ///
    /// // Controller parameters will also be included
    /// assert_eq!(states.get_numeric(MOD_WHEEL_PARAMETER), Some(0.0));
    /// ```
    pub fn new_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
    ) -> Self {
        Self::new_override_synth_defaults(infos, &Default::default())
    }
}

impl States for StatesMap {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue> {
        self.map.get(&id_hash).copied()
    }
}

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
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
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
        infos: impl IntoIterator<Item = InfoRef<'a, S>> + 'a,
    ) -> Self {
        Self::new_override_defaults(infos, &Default::default())
    }

    /// Create a new [`ConstantBufferStates`] object to pass to a synth from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`Self::new_override_defaults`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER, PITCH_BEND_PARAMETER};
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
    ///   // Or you can override control parameters
    ///   (MOD_WHEEL_PARAMETER, InternalValue::Numeric(0.2)),
    /// ].into_iter().collect();
    ///
    /// let buffer_states = ConstantBufferStates::new_override_synth_defaults(infos, &overrides);
    ///
    /// // Overridden parameters get the values you passed in
    /// match buffer_states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    /// match buffer_states.get_numeric(MOD_WHEEL_PARAMETER) {
    ///   Some(NumericBufferState::Constant(0.2)) => (),
    ///   _ => panic!("Expected constant value of 0.2"),
    /// };
    ///
    /// // Other parameters get their default values
    /// match buffer_states.get_numeric(PITCH_BEND_PARAMETER) {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_override_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self::new(StatesMap::new_override_synth_defaults(infos, overrides))
    }

    /// Create a new [`ConstantBufferStates`] object to pass to a synth from a list of `Info`s.
    ///
    /// Each parameter in `Info`s will be set to its default value for the whole buffer.
    ///
    /// This is similar to [`Self::new_defaults`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, ConstantBufferStates, BufferStates, NumericBufferState};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER};
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
    /// match buffer_states.get_numeric(MOD_WHEEL_PARAMETER) {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_synth_defaults<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
    ) -> Self {
        Self::new_override_synth_defaults(infos, &Default::default())
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
        RampedState::RampedNumeric { start, end, range }
    }
}
fn ramped_enum(start: u32, end: u32, num_vaules: usize) -> RampedState {
    if start == end {
        RampedState::Constant(InternalValue::Enum(start))
    } else {
        RampedState::RampedEnum {
            start,
            end,
            range: 0..u32::try_from(num_vaules).unwrap(),
        }
    }
}
fn ramped_switch(start: bool, end: bool) -> RampedState {
    if start == end {
        RampedState::Constant(InternalValue::Switch(start))
    } else {
        RampedState::RampedSwitch { start, end }
    }
}

impl RampedStatesMap {
    /// Constructor that creates a `RampedStatesMap`
    /// from a list of `Info`s and `override`s.at the start and end of the buffer.
    ///
    /// These overrides work the same way as in [`override_defaults`].
    ///
    /// Note that if you want to pass this into a synth, you should use [`Self::new_synth`] instead.
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
                let value = match (
                    info.type_specific,
                    start_overrides.get(info.unique_id),
                    end_overrides.get(info.unique_id),
                ) {
                    (
                        TypeSpecificInfoRef::Numeric { valid_range, .. },
                        Some(InternalValue::Numeric(start)),
                        Some(InternalValue::Numeric(end)),
                    ) => ramped_numeric(*start, *end, valid_range),
                    (
                        TypeSpecificInfoRef::Numeric {
                            default,
                            valid_range,
                            ..
                        },
                        None,
                        Some(InternalValue::Numeric(end)),
                    ) => ramped_numeric(default, *end, valid_range),
                    (
                        TypeSpecificInfoRef::Numeric {
                            default,
                            valid_range,
                            ..
                        },
                        Some(InternalValue::Numeric(start)),
                        None,
                    ) => ramped_numeric(*start, default, valid_range),
                    (TypeSpecificInfoRef::Numeric { default, .. }, None, None) => {
                        RampedState::Constant(InternalValue::Numeric(default))
                    }
                    (
                        TypeSpecificInfoRef::Enum { values, .. },
                        Some(InternalValue::Enum(start)),
                        Some(InternalValue::Enum(end)),
                    ) => ramped_enum(*start, *end, values.len()),
                    (
                        TypeSpecificInfoRef::Enum {
                            default, values, ..
                        },
                        None,
                        Some(InternalValue::Enum(end)),
                    ) => ramped_enum(default, *end, values.len()),
                    (
                        TypeSpecificInfoRef::Enum {
                            default, values, ..
                        },
                        Some(InternalValue::Enum(start)),
                        None,
                    ) => ramped_enum(*start, default, values.len()),
                    (TypeSpecificInfoRef::Enum { default, .. }, None, None) => {
                        RampedState::Constant(InternalValue::Enum(default))
                    }
                    (
                        TypeSpecificInfoRef::Switch { .. },
                        Some(InternalValue::Switch(start)),
                        Some(InternalValue::Switch(end)),
                    ) => ramped_switch(*start, *end),
                    (
                        TypeSpecificInfoRef::Switch { default },
                        None,
                        Some(InternalValue::Switch(end)),
                    ) => ramped_switch(default, *end),
                    (
                        TypeSpecificInfoRef::Switch { default },
                        Some(InternalValue::Switch(start)),
                        None,
                    ) => ramped_switch(*start, default),
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

    /// Create a new [`RampedStatesMap`] for synths from a list of `Info`s and `override`s.
    ///
    /// This is similar to [`Self::new`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, RampedStatesMap, NumericBufferState, BufferStates};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER, PITCH_BEND_PARAMETER};
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
    /// let start_overrides = vec![(MOD_WHEEL_PARAMETER, InternalValue::Numeric(1.0))].into_iter().collect();
    /// let end_overrides = vec![("numeric", InternalValue::Numeric(0.5))].into_iter().collect();
    /// let states = RampedStatesMap::new_synth(
    ///   infos.iter().cloned(),
    ///   &start_overrides,
    ///   &end_overrides,
    ///   10
    /// );
    ///
    /// // If we only overrode a value at the beginning or end
    /// // it should be ramped
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::PiecewiseLinear(_)) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    /// match states.get_numeric(MOD_WHEEL_PARAMETER) {
    ///   Some(NumericBufferState::PiecewiseLinear(_)) => (),
    ///   _ => panic!("Expected a ramped value"),
    /// };
    ///
    /// // Params left at default should be constants
    /// match states.get_numeric(PITCH_BEND_PARAMETER) {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected a constant value"),
    /// };
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if `start_overrides` or `end_overrides` do not match the type of the parameter
    /// specified in `infos`.
    pub fn new_synth<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
        start_overrides: &HashMap<&'_ str, InternalValue>,
        end_overrides: &HashMap<&'_ str, InternalValue>,
        buffer_size: usize,
    ) -> Self {
        Self::new(
            infos.into_iter().chain(CONTROLLER_PARAMETERS),
            start_overrides,
            end_overrides,
            buffer_size,
        )
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

    /// Create a new [`RampedStatesMap`] for synths with all parameters constant.
    ///
    /// This is useful for _performance_ testing because while the parameters
    /// are constant at run-time, the `RampedStatesMap` has the ability to
    /// ramp between values, so consumers cannot be specialized to handle constant
    /// values only
    ///
    /// This is similar to [`Self::new_const`], but it also includes the controller parameters
    /// that are common to all synths. ([`crate::synth::CONTROLLER_PARAMETERS`]).
    ///
    /// Thus, this is more appropriate to use if you plan to pass the parameters to a synth.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{StaticInfoRef, InternalValue, TypeSpecificInfoRef, RampedStatesMap, NumericBufferState, BufferStates};
    /// # use conformal_component::synth::{MOD_WHEEL_PARAMETER};
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
    /// let states = RampedStatesMap::new_const_synth(infos.iter().cloned(), &overrides);
    ///
    /// // Overridden parameters get the values you passed in
    /// match states.get_numeric("numeric") {
    ///   Some(NumericBufferState::Constant(0.5)) => (),
    ///   _ => panic!("Expected constant value of 0.5"),
    /// };
    ///
    /// // Controller parameters will also be included
    /// match states.get_numeric(MOD_WHEEL_PARAMETER) {
    ///   Some(NumericBufferState::Constant(0.0)) => (),
    ///   _ => panic!("Expected constant value of 0.0"),
    /// };
    /// ```
    pub fn new_const_synth<'a, 'b: 'a>(
        infos: impl IntoIterator<Item = InfoRef<'a, &'b str>> + 'a,
        overrides: &HashMap<&'_ str, InternalValue>,
    ) -> Self {
        Self::new_synth(infos, overrides, overrides, 0)
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

#[cfg(test)]
mod tests {
    use crate::audio::all_approx_eq;

    use super::super::{
        PiecewiseLinearCurve, PiecewiseLinearCurvePoint, TimedEnumValues, TimedSwitchValues,
        TimedValue,
    };
    use super::{
        piecewise_linear_curve_per_sample, timed_enum_per_sample, timed_switch_per_sample,
    };

    const TEST_EPSILON: f32 = 1e-7;

    #[test]
    fn piecewise_linear_curve_per_sample_basics() {
        let vals = piecewise_linear_curve_per_sample(
            PiecewiseLinearCurve::new(
                (&[
                    PiecewiseLinearCurvePoint {
                        sample_offset: 0,
                        value: 0.0,
                    },
                    PiecewiseLinearCurvePoint {
                        sample_offset: 5,
                        value: 5.0,
                    },
                    PiecewiseLinearCurvePoint {
                        sample_offset: 7,
                        value: 5.0,
                    },
                    PiecewiseLinearCurvePoint {
                        sample_offset: 8,
                        value: 10.0,
                    },
                ])
                    .iter()
                    .cloned(),
                10,
                0.0..=10.0,
            )
            .unwrap(),
        )
        .collect::<Vec<_>>();
        assert!(all_approx_eq(
            vals.iter().copied(),
            ([0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0, 10.0, 10.0])
                .iter()
                .copied(),
            TEST_EPSILON
        ));
    }

    #[test]
    fn timed_enum_per_sample_basics() {
        let vals = timed_enum_per_sample(
            TimedEnumValues::new(
                (&[
                    TimedValue {
                        sample_offset: 0,
                        value: 0,
                    },
                    TimedValue {
                        sample_offset: 7,
                        value: 2,
                    },
                    TimedValue {
                        sample_offset: 8,
                        value: 3,
                    },
                ])
                    .iter()
                    .cloned(),
                10,
                0..4,
            )
            .unwrap(),
        )
        .collect::<Vec<_>>();
        assert!(
            vals.iter()
                .copied()
                .zip(([0, 0, 0, 0, 0, 0, 0, 2, 3, 3]).iter().copied())
                .all(|(a, b)| a == b)
        );
    }

    #[test]
    fn timed_switch_per_sample_basics() {
        let vals = timed_switch_per_sample(
            TimedSwitchValues::new(
                (&[
                    TimedValue {
                        sample_offset: 0,
                        value: false,
                    },
                    TimedValue {
                        sample_offset: 7,
                        value: true,
                    },
                    TimedValue {
                        sample_offset: 8,
                        value: false,
                    },
                ])
                    .iter()
                    .cloned(),
                10,
            )
            .unwrap(),
        )
        .collect::<Vec<_>>();
        assert!(
            vals.iter()
                .copied()
                .zip(
                    ([
                        false, false, false, false, false, false, false, true, false, false
                    ])
                    .iter()
                    .copied()
                )
                .all(|(a, b)| a == b)
        );
    }
}
