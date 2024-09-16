//! Code related to the _parameters_ of a processor.
//!
//! A processor has a number of _parameters_ that can be changed over time.
//!
//! The parameters state is managed by Conformal, with changes ultimately coming
//! from either the UI or the hosting application.
//! The parameters form the "logical interface" of the processor.
//!
//! Each parameter is one of the following types:
//!
//! - Numeric: A numeric value that can vary within a range of possible values.
//! - Enum: An value that can take one of a discrete set of named values.
//! - Switch: A value that can be either on or off.
//!
//! Note that future versions may add more types of parameters!
//!
//! Components tell Conformal about which parameters exist in their [`crate::Component::parameter_infos`] method.
//!
//! Conformal will then provide the current state to the processor during processing,
//! either [`crate::synth::Synth::process`] or [`crate::effect::Effect::process`].
//!
//! Note that conformal may also change parameters outside of processing and call
//! the [`crate::synth::Synth::handle_events`] or
//! [`crate::effect::Effect::handle_parameters`] methods, Components can update any
//! internal state in these methods.
use std::{
    ops::{Range, RangeBounds, RangeInclusive},
    string::ToString,
};

mod utils;
pub use utils::*;

#[cfg(test)]
mod tests;

macro_rules! info_enum_doc {
    () => {
        "Information specific to an enum parameter."
    };
}

macro_rules! info_enum_default_doc {
    () => {
        "Index of the default value.

Note that this _must_ be less than the length of `values`."
    };
}

macro_rules! info_enum_values_doc {
    () => {
        "A list of possible values for the parameter.

Note that values _must_ contain at least 2 elements."
    };
}

macro_rules! info_numeric_doc {
    () => {
        "Information specific to a numeric parameter."
    };
}

macro_rules! info_numeric_default_doc {
    () => {
        "The default value of the parameter.

This value _must_ be within the `valid_range`."
    };
}

macro_rules! info_numeric_valid_range_doc {
    () => {
        "The valid range of the parameter."
    };
}

macro_rules! info_numeric_units_doc {
    () => {
        "The units of the parameter.

Here an empty string indicates unitless values, while a non-empty string
indicates the logical units of a parmater, e.g., \"hz\""
    };
}

macro_rules! info_switch_doc {
    () => {
        "Information specific to a switch parameter."
    };
}

macro_rules! info_switch_default_doc {
    () => {
        "The default value of the parameter."
    };
}

/// Contains information specific to a certain type of parameter.
///
/// This is a non-owning reference type, pointing to data with lifetime `'a`.
///
/// Here the `S` represents the type of strings, this generally will be
/// either `&'a str` or `String`.
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{TypeSpecificInfoRef};
/// let enum_info = TypeSpecificInfoRef::Enum {
///    default: 0,
///    values: &["A", "B", "C"],
/// };
///
/// let numeric_info: TypeSpecificInfoRef<'static, &'static str> = TypeSpecificInfoRef::Numeric {
///   default: 0.0,
///   valid_range: 0.0..=1.0,
///   units: None,
/// };
///
/// let switch_info: TypeSpecificInfoRef<'static, &'static str> = TypeSpecificInfoRef::Switch {
///  default: false,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpecificInfoRef<'a, S> {
    #[doc = info_enum_doc!()]
    Enum {
        #[doc = info_enum_default_doc!()]
        default: u32,

        #[doc = info_enum_values_doc!()]
        values: &'a [S],
    },

    #[doc = info_numeric_doc!()]
    Numeric {
        #[doc = info_numeric_default_doc!()]
        default: f32,

        #[doc = info_numeric_valid_range_doc!()]
        valid_range: RangeInclusive<f32>,

        #[doc = info_numeric_units_doc!()]
        units: Option<&'a str>,
    },

    #[doc = info_switch_doc!()]
    Switch {
        #[doc = info_switch_default_doc!()]
        default: bool,
    },
}

/// Contains information specific to a certain type of parameter.
///
/// This is an owning version of [`TypeSpecificInfoRef`].
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{TypeSpecificInfo};
/// let enum_info = TypeSpecificInfo::Enum {
///   default: 0,
///   values: vec!["A".to_string(), "B".to_string(), "C".to_string()],
/// };
/// let numeric_info = TypeSpecificInfo::Numeric {
///   default: 0.0,
///   valid_range: 0.0..=1.0,
///   units: None,
/// };
/// let switch_info = TypeSpecificInfo::Switch {
///   default: false,
/// };
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpecificInfo {
    #[doc = info_enum_doc!()]
    Enum {
        #[doc = info_enum_default_doc!()]
        default: u32,

        #[doc = info_enum_values_doc!()]
        values: Vec<String>,
    },

    #[doc = info_numeric_doc!()]
    Numeric {
        #[doc = info_numeric_default_doc!()]
        default: f32,

        #[doc = info_numeric_valid_range_doc!()]
        valid_range: std::ops::RangeInclusive<f32>,

        #[doc = info_numeric_units_doc!()]
        units: Option<String>,
    },

    #[doc = info_switch_doc!()]
    Switch {
        #[doc = info_switch_default_doc!()]
        default: bool,
    },
}

impl<'a, S: AsRef<str>> From<&'a TypeSpecificInfoRef<'a, S>> for TypeSpecificInfo {
    fn from(v: &'a TypeSpecificInfoRef<'a, S>) -> Self {
        match v {
            TypeSpecificInfoRef::Enum { default, values } => {
                let values: Vec<String> = values.iter().map(|s| s.as_ref().to_string()).collect();
                assert!(values.len() < i32::MAX as usize);
                TypeSpecificInfo::Enum {
                    default: *default,
                    values,
                }
            }
            TypeSpecificInfoRef::Numeric {
                default,
                valid_range,
                units,
            } => TypeSpecificInfo::Numeric {
                default: *default,
                valid_range: valid_range.clone(),
                units: (*units).map(ToString::to_string),
            },
            TypeSpecificInfoRef::Switch { default } => {
                TypeSpecificInfo::Switch { default: *default }
            }
        }
    }
}

impl<'a> From<&'a TypeSpecificInfo> for TypeSpecificInfoRef<'a, String> {
    fn from(v: &'a TypeSpecificInfo) -> Self {
        match v {
            TypeSpecificInfo::Enum { default, values } => TypeSpecificInfoRef::Enum {
                default: *default,
                values: values.as_slice(),
            },
            TypeSpecificInfo::Numeric {
                default,
                valid_range,
                units,
            } => TypeSpecificInfoRef::Numeric {
                default: *default,
                valid_range: valid_range.clone(),
                units: units.as_ref().map(String::as_str),
            },
            TypeSpecificInfo::Switch { default } => {
                TypeSpecificInfoRef::Switch { default: *default }
            }
        }
    }
}

/// Metadata about a parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flags {
    /// Whether the parameter can be automated.
    ///
    /// In some hosting applications, parameters can be _automated_,
    /// that is, users are provided with a UI to program the parameter
    /// to change over time. If this is `true` (the default), then
    /// this parameter will appear in the automation UI. Otherwise,
    /// it will not.
    ///
    /// You may want to set a parameter to `false` here if it does not
    /// sound good when it is change frequently, or if it is a parameter
    /// that may be confusing to users if it appeared in an automation UI.
    pub automatable: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Flags { automatable: true }
    }
}

macro_rules! unique_id_doc {
    () => {
        "The unique ID of the parameter.

As the name implies, each parameter's id must be unique within
the comonent's parameters.

Note that this ID will not be presented to the user, it is only
used to refer to the parameter in code."
    };
}

macro_rules! title_doc {
    () => {
        "Human-readable title of the parameter."
    };
}

macro_rules! short_title_doc {
    () => {
        "A short title of the parameter.

In some hosting applications, this may appear as an
abbreviated version of the title. If the title is already
short, it's okay to use the same value for `title` and `short_title`."
    };
}

macro_rules! flags_doc {
    () => {
        "Metadata about the parameter"
    };
}

macro_rules! type_specific_doc {
    () => {
        "Information specific to the type of parameter."
    };
}

/// Information about a parameter.
///
/// This is a non-owning reference type.
///
/// If you are referencing static data, use [`StaticInfoRef`] below for simplicity.
///
/// This references data with lifetime `'a`.
/// Here the `S` represents the type of strings, this generally will be
/// either `&'a str` or `String`.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoRef<'a, S> {
    #[doc = unique_id_doc!()]
    pub unique_id: &'a str,

    #[doc = title_doc!()]
    pub title: &'a str,

    #[doc = short_title_doc!()]
    pub short_title: &'a str,

    #[doc = flags_doc!()]
    pub flags: Flags,

    #[doc = type_specific_doc!()]
    pub type_specific: TypeSpecificInfoRef<'a, S>,
}

/// Owning version of [`InfoRef`].
#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    #[doc = unique_id_doc!()]
    pub unique_id: String,

    #[doc = title_doc!()]
    pub title: String,

    #[doc = short_title_doc!()]
    pub short_title: String,

    #[doc = flags_doc!()]
    pub flags: Flags,

    #[doc = type_specific_doc!()]
    pub type_specific: TypeSpecificInfo,
}

impl<'a, S: AsRef<str>> From<&'a InfoRef<'a, S>> for Info {
    fn from(v: &'a InfoRef<'a, S>) -> Self {
        Info {
            title: v.title.to_string(),
            short_title: v.short_title.to_string(),
            unique_id: v.unique_id.to_string(),
            flags: v.flags.clone(),
            type_specific: (&v.type_specific).into(),
        }
    }
}

impl<'a> From<&'a Info> for InfoRef<'a, String> {
    fn from(v: &'a Info) -> Self {
        InfoRef {
            title: &v.title,
            short_title: &v.short_title,
            unique_id: &v.unique_id,
            flags: v.flags.clone(),
            type_specific: (&v.type_specific).into(),
        }
    }
}

/// [`InfoRef`] of static data
///
/// In many cases, the `InfoRef` will be a reference to static data,
/// in which case the type parameters can seem noisy. This type
/// alias is here for convenience!
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{TypeSpecificInfoRef, StaticInfoRef};
/// let enum_info = StaticInfoRef {
///   title: "Enum",
///   short_title: "Enum",
///   unique_id: "enum",
///   flags: Default::default(),
///   type_specific: TypeSpecificInfoRef::Enum {
///     default: 0,
///     values: &["A", "B", "C"],
///   },
/// };
/// let numeric_info = StaticInfoRef {
///   title: "Numeric",
///   short_title: "Num",
///   unique_id: "numeric",
///   flags: Default::default(),
///   type_specific: TypeSpecificInfoRef::Numeric {
///     default: 0.0,
///     valid_range: 0.0..=1.0,
///     units: None,
///   },
/// };
/// let switch_info = StaticInfoRef {
///   title: "Switch",
///   short_title: "Switch",
///   unique_id: "switch",
///   flags: Default::default(),
///   type_specific: TypeSpecificInfoRef::Switch {
///     default: false,
///   },
/// };
/// ```
pub type StaticInfoRef = InfoRef<'static, &'static str>;

/// Converts a slice of [`InfoRef`]s to a vector of [`Info`]s.
///
/// # Examples
///
/// ```
/// # use conformal_component::parameters::{StaticInfoRef, TypeSpecificInfoRef, Info, to_infos};
/// let infos: Vec<Info> = to_infos(&[
///   StaticInfoRef {
///     title: "Switch",
///     short_title: "Switch",
///     unique_id: "switch",
///     flags: Default::default(),
///     type_specific: TypeSpecificInfoRef::Switch {
///       default: false,
///     },
///   },
/// ]);
/// ```
pub fn to_infos(v: &[InfoRef<'_, &'_ str>]) -> Vec<Info> {
    v.iter().map(Into::into).collect()
}

/// A numeric hash of a parameter's ID.
///
/// In contexts where performance is critical, we refer to parameters
/// by a numeric hash of their `unique_id`.
#[derive(Eq, Hash, PartialEq, Clone, Copy, Debug)]
pub struct IdHash {
    internal_hash: u32,
}

#[doc(hidden)]
#[must_use]
pub fn id_hash_from_internal_hash(internal_hash: u32) -> IdHash {
    IdHash {
        internal_hash: internal_hash & 0x7fff_ffff,
    }
}

impl IdHash {
    #[doc(hidden)]
    #[must_use]
    pub fn internal_hash(&self) -> u32 {
        self.internal_hash
    }
}

/// Creates a hash from a unique ID.
///
/// This converts a parameter's `unique_id` into an [`IdHash`].
///
/// # Examples
///
/// ```
/// use conformal_component::parameters::hash_id;
/// let hash = hash_id("my_parameter");
/// ```
#[must_use]
pub fn hash_id(unique_id: &str) -> IdHash {
    id_hash_from_internal_hash(fxhash::hash32(unique_id) & 0x7fff_ffff)
}

/// A value of a parameter used in performance-critical ocntexts.
///
/// This is used when performance is critical and we don't want to
/// refer to enums by their string values.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum InternalValue {
    /// A numeric value.
    Numeric(f32),

    /// The _index_ of an enum value.
    ///
    /// This refers to the index of the current value in the `values`
    /// array of the parameter.
    Enum(u32),

    /// A switch value.
    Switch(bool),
}

/// A value of a parameter
///
/// Outside of performance-critical contexts, we use this to refer
/// to parameter values.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A numeric value.
    Numeric(f32),

    /// An enum value.
    Enum(String),

    /// A switch value.
    Switch(bool),
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Numeric(v)
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::Enum(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Switch(v)
    }
}

/// Represents a snapshot of all valid parameters at a given point in time.
///
/// We use this trait to provide information about parameters when we are
/// _not_ processing a buffer (for that, we use [`BufferStates`]).
///
/// This is passed into [`crate::synth::Synth::handle_events`] and
/// [`crate::effect::Effect::handle_parameters`].
///
/// For convenience, we provide [`States::get_numeric`], [`States::get_enum`],
/// and [`States::get_switch`] functions, which return the value of the parameter
/// if it is of the correct type, or `None` otherwise.
/// Note that all parmeter types re-use the same `ID` space, so only one of the
/// specialized `get` methods will return a value for a given `ParameterID`.
///
/// Note that in general, the Conformal wrapper will implement this trait
/// for you, but we provide a simple implementation called [`StatesMap`]
/// that's appropriate to use in tests or other cases where you need to
/// create this trait outside of a Conformal wrapper.
pub trait States {
    /// Get the current value of a parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If there is no parameter with the given ID, this will return `None`.
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue>;

    /// Get the current value of a parameter by it's unique ID.
    ///
    /// If there is no parameter with the given ID, this will return `None`.
    fn get(&self, unique_id: &str) -> Option<InternalValue> {
        self.get_by_hash(hash_id(unique_id))
    }

    /// Get the current numeric value of a parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If the parameter is not present or is not numeric, this will return `None`.
    fn numeric_by_hash(&self, id_hash: IdHash) -> Option<f32> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Numeric(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the current numeric value of a parameter by it's unique ID.
    ///
    /// If the parameter is not present or is not numeric, this will return `None`.
    fn get_numeric(&self, unique_id: &str) -> Option<f32> {
        self.numeric_by_hash(hash_id(unique_id))
    }

    /// Get the current enum value of a parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If the parameter is not present or is not an enum, this will return `None`.
    fn enum_by_hash(&self, id_hash: IdHash) -> Option<u32> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Enum(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the current enum value of a parameter by it's unique ID.
    ///
    /// If the parameter is not present or is not an enum, this will return `None`.
    fn get_enum(&self, unique_id: &str) -> Option<u32> {
        self.enum_by_hash(hash_id(unique_id))
    }

    /// Get the current switch value of a parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If the parameter is not present or is not a switch, this will return `None`.
    fn switch_by_hash(&self, id_hash: IdHash) -> Option<bool> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Switch(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the current switch value of a parameter by it's unique ID.
    ///
    /// If the parameter is not present or is not a switch, this will return `None`.
    fn get_switch(&self, unique_id: &str) -> Option<bool> {
        self.switch_by_hash(hash_id(unique_id))
    }
}

/// Represents a single point of a piecewise linear curve.
#[derive(Debug, Clone, PartialEq)]
pub struct PiecewiseLinearCurvePoint {
    /// The number of samples from the start of the buffer this point occurs at.
    pub sample_offset: usize,

    /// The value of the curve at this point.
    pub value: f32,
}

/// Represents a numeric value that changes over the course of the buffer.
///
/// We represent values changing over the course of the buffer as a piecewise
/// linear curve, where the curve moving linearly from point to point.
///
/// Note that the curve is _guaranteed_ to begin at 0, however it
/// may end before the end of the buffer - in this case, the value
/// remains constant until the end of the buffer.
///
/// Some invariants:
///  - There will always be at least one point
///  - The first point's `sample_offset` will be 0
///  - `sample_offset`s will be monotonically increasing and only one
///    point will appear for each `sample_offset`
///  - All point's `value` will be between the parameter's `min` and `max`
pub struct PiecewiseLinearCurve<I> {
    points: I,

    buffer_size: usize,
}

trait ValueAndSampleOffset<V> {
    fn value(&self) -> &V;
    fn sample_offset(&self) -> usize;
}

impl ValueAndSampleOffset<f32> for PiecewiseLinearCurvePoint {
    fn value(&self) -> &f32 {
        &self.value
    }

    fn sample_offset(&self) -> usize {
        self.sample_offset
    }
}

fn check_curve_invariants<
    V: PartialOrd + PartialEq + core::fmt::Debug,
    P: ValueAndSampleOffset<V>,
    I: Iterator<Item = P>,
>(
    iter: I,
    buffer_size: usize,
    valid_range: impl RangeBounds<V>,
) -> bool {
    let mut last_sample_offset = None;
    for point in iter {
        if point.sample_offset() >= buffer_size {
            return false;
        }
        if let Some(last) = last_sample_offset {
            if point.sample_offset() <= last {
                return false;
            }
        } else if point.sample_offset() != 0 {
            return false;
        }
        if !valid_range.contains(point.value()) {
            return false;
        }
        last_sample_offset = Some(point.sample_offset());
    }
    last_sample_offset.is_some()
}

impl<I: IntoIterator<Item = PiecewiseLinearCurvePoint> + Clone> PiecewiseLinearCurve<I> {
    /// Construct a new [`PiecewiseLinearCurve`] from an iterator of points.
    ///
    /// This will check the invariants for the curve, and if any are invalid, this will
    /// return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{PiecewiseLinearCurve, PiecewiseLinearCurvePoint};
    /// assert!(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 100, value: 1.0 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).is_some());
    ///
    /// // Curves must include at least one point
    /// assert!(PiecewiseLinearCurve::new(vec![], 128, 0.0..=1.0).is_none());
    ///
    /// // Curves can't go outside the valid range.
    /// assert!(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 100, value: 2.0 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).is_none());
    ///
    /// // The curve must not go past the end of the buffer
    /// assert!(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 128, value: 1.0 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).is_none());
    ///
    /// // The first point must be at 0
    /// assert!(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 50, value: 0.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 100, value: 1.0 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).is_none());
    ///
    /// // Sample offsets must monotonically increase
    /// assert!(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 100, value: 1.0 },
    ///        PiecewiseLinearCurvePoint { sample_offset: 50, value: 0.5 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).is_none());
    /// ```
    pub fn new(points: I, buffer_size: usize, valid_range: RangeInclusive<f32>) -> Option<Self> {
        if buffer_size == 0 {
            return None;
        }
        if check_curve_invariants(points.clone().into_iter(), buffer_size, valid_range) {
            Some(Self {
                points,
                buffer_size,
            })
        } else {
            None
        }
    }
}

impl<I> PiecewiseLinearCurve<I> {
    /// Get the size of the buffer this curve is defined over.
    ///
    /// Note that the last point may occur _before_ the end of the buffer,
    /// in which case the value remains constant from that point until the
    /// end of the buffer.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl<I: IntoIterator<Item = PiecewiseLinearCurvePoint>> IntoIterator for PiecewiseLinearCurve<I> {
    type Item = PiecewiseLinearCurvePoint;
    type IntoIter = I::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.points.into_iter()
    }
}

/// Represents a value at a specific point in time in a buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct TimedValue<V> {
    /// The number of samples from the start of the buffer.
    pub sample_offset: usize,

    /// The value at this point in time.
    pub value: V,
}

impl<V> ValueAndSampleOffset<V> for TimedValue<V> {
    fn value(&self) -> &V {
        &self.value
    }

    fn sample_offset(&self) -> usize {
        self.sample_offset
    }
}

/// Represents an enum value that changes over the course of a buffer.
///
/// Each point represents a change in value at a given sample offset -
/// the value remains constant until the next point (or the end of the buffer)
///
/// Some invariants:
///  - There will always be at least one point
///  - The first point's `sample_offset` will be 0
///  - `sample_offset`s will be monotonically increasing and only one
///    point will appear for each `sample_offset`
///  - All point's `value` will be valid
pub struct TimedEnumValues<I> {
    points: I,
    buffer_size: usize,
}

impl<I: IntoIterator<Item = TimedValue<u32>> + Clone> TimedEnumValues<I> {
    /// Construct a new [`TimedEnumValues`] from an iterator of points.
    ///
    /// This will check the invariants for the curve, and if any are invalid, this will
    /// return `None`.
    ///
    /// Note that here we refer to the enum by the _index_ of the value,
    /// that is, the index of the value in the `values` array of the parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{TimedEnumValues, TimedValue};
    /// assert!(TimedEnumValues::new(
    ///   vec![TimedValue { sample_offset: 0, value: 0 },
    ///        TimedValue { sample_offset: 100, value: 1 }],
    ///   128,
    ///   0..2,
    /// ).is_some());
    /// ```
    pub fn new(points: I, buffer_size: usize, valid_range: Range<u32>) -> Option<Self> {
        if buffer_size == 0 {
            return None;
        }
        if check_curve_invariants(points.clone().into_iter(), buffer_size, valid_range) {
            Some(Self {
                points,
                buffer_size,
            })
        } else {
            None
        }
    }
}

impl<I> TimedEnumValues<I> {
    /// Get the size of the buffer this curve is defined over.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl<I: IntoIterator<Item = TimedValue<u32>>> IntoIterator for TimedEnumValues<I> {
    type Item = TimedValue<u32>;
    type IntoIter = I::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.points.into_iter()
    }
}

/// Represents a switched value that changes over the course of a buffer.
///
/// Each point represents a change in value at a given sample offset -
/// the value remains constant until the next point (or the end of the buffer)
///
/// Some invariants:
///  - There will always be at least one point
///  - The first point's `sample_offset` will be 0
///  - `sample_offset`s will be monotonically increasing and only one
///    point will appear for each `sample_offset`
pub struct TimedSwitchValues<I> {
    points: I,
    buffer_size: usize,
}

impl<I: IntoIterator<Item = TimedValue<bool>> + Clone> TimedSwitchValues<I> {
    /// Construct a new [`TimedSwitchValues`] from an iterator of points.
    ///
    /// This will check the invariants for the curve, and if any are invalid, this will
    /// return `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{TimedSwitchValues, TimedValue};
    /// assert!(TimedSwitchValues::new(
    ///   vec![TimedValue { sample_offset: 0, value: false },
    ///        TimedValue { sample_offset: 100, value: true }],
    ///   128,
    /// ).is_some());
    /// ```
    pub fn new(points: I, buffer_size: usize) -> Option<Self> {
        if buffer_size == 0 {
            return None;
        }
        if check_curve_invariants(points.clone().into_iter(), buffer_size, false..=true) {
            Some(Self {
                points,
                buffer_size,
            })
        } else {
            None
        }
    }
}

impl<I> TimedSwitchValues<I> {
    /// Get the size of the buffer this curve is defined over.
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl<I: IntoIterator<Item = TimedValue<bool>>> IntoIterator for TimedSwitchValues<I> {
    type Item = TimedValue<bool>;
    type IntoIter = I::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.points.into_iter()
    }
}

/// Represents the state of a numeric value across a buffer
pub enum NumericBufferState<I> {
    /// The value is constant across the buffer.
    Constant(f32),

    /// The value changes over the course of the buffer, represented by a
    /// [`PiecewiseLinearCurve`].
    PiecewiseLinear(PiecewiseLinearCurve<I>),
}

impl<I: IntoIterator<Item = PiecewiseLinearCurvePoint>> NumericBufferState<I> {
    /// Get the value of the parameter at the start of the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{NumericBufferState, PiecewiseLinearCurve, PiecewiseLinearCurvePoint};
    /// assert_eq!(NumericBufferState::PiecewiseLinear(PiecewiseLinearCurve::new(
    ///   vec![PiecewiseLinearCurvePoint { sample_offset: 0, value: 0.5 },
    ///       PiecewiseLinearCurvePoint { sample_offset: 100, value: 1.0 }],
    ///   128,
    ///   0.0..=1.0,
    /// ).unwrap()).value_at_start_of_buffer(), 0.5);
    /// ```
    #[allow(clippy::missing_panics_doc)] // Only panics when invariants are broken.
    pub fn value_at_start_of_buffer(self) -> f32 {
        match self {
            NumericBufferState::Constant(v) => v,
            NumericBufferState::PiecewiseLinear(v) => v.points.into_iter().next().unwrap().value,
        }
    }
}

/// Represents the state of an enum value across a buffer
///
/// Here we refer to the enum by the _index_ of the value,
/// that is, the index of the value in the `values` array of the parameter.
pub enum EnumBufferState<I> {
    /// The value is constant across the buffer.
    Constant(u32),

    /// The value changes over the course of the buffer, represented by a
    /// [`TimedEnumValues`].
    Varying(TimedEnumValues<I>),
}

impl<I: IntoIterator<Item = TimedValue<u32>>> EnumBufferState<I> {
    /// Get the value of the parameter at the start of the buffer,
    /// represented by the index of the value in the `values` array of the parameter.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{EnumBufferState, TimedEnumValues, TimedValue};
    /// assert_eq!(EnumBufferState::Varying(TimedEnumValues::new(
    ///   vec![TimedValue { sample_offset: 0, value: 1 },
    ///        TimedValue { sample_offset: 100, value: 2 }],
    ///   128,
    ///   0..3
    /// ).unwrap()).value_at_start_of_buffer(), 1);
    /// ```
    #[allow(clippy::missing_panics_doc)] // Only panics when invariants are broken.
    pub fn value_at_start_of_buffer(self) -> u32 {
        match self {
            EnumBufferState::Constant(v) => v,
            EnumBufferState::Varying(v) => v.points.into_iter().next().unwrap().value,
        }
    }
}

/// Represents the state of an switched value across a buffer
pub enum SwitchBufferState<I> {
    /// The value is constant across the buffer.
    Constant(bool),

    /// The value changes over the course of the buffer, represented by a
    /// [`TimedSwitchValues`].
    Varying(TimedSwitchValues<I>),
}

impl<I: IntoIterator<Item = TimedValue<bool>>> SwitchBufferState<I> {
    /// Get the value of the parameter at the start of the buffer.
    ///
    /// # Examples
    ///
    /// ```
    /// # use conformal_component::parameters::{SwitchBufferState, TimedSwitchValues, TimedValue};
    /// assert_eq!(SwitchBufferState::Varying(TimedSwitchValues::new(
    ///   vec![TimedValue { sample_offset: 0, value: true },
    ///        TimedValue { sample_offset: 100, value: false }],
    ///   128,
    /// ).unwrap()).value_at_start_of_buffer(), true);
    /// ```
    #[allow(clippy::missing_panics_doc)] // Only panics when invariants are broken.
    pub fn value_at_start_of_buffer(self) -> bool {
        match self {
            SwitchBufferState::Constant(v) => v,
            SwitchBufferState::Varying(v) => v.points.into_iter().next().unwrap().value,
        }
    }
}

/// Represents the value of a parameter as it varies across a buffer.
pub enum BufferState<N, E, S> {
    /// The value of a numeric parameter represented by a [`NumericBufferState`].
    Numeric(NumericBufferState<N>),

    /// The value of an enum parameter represented by a [`EnumBufferState`].
    Enum(EnumBufferState<E>),

    /// The value of a switch parameter represented by a [`SwitchBufferState`].
    Switch(SwitchBufferState<S>),
}

/// Represents the state of several parameters across a buffer.
///
/// Each parameter is represented by a [`BufferState`], which represents
/// a value for that parameter at each sample of the buffer.
///
/// To easily process parameters from this struct, you can use the
/// [`crate::pzip`] macro, which converts a [`BufferStates`] into a per-sample
/// iterator containing the values of each parameter you want to look at.
///
/// For more low-level usages, you can deal directly with the underlying [`BufferState`]
/// objects, which might yield higher performance in some cases than the [`crate::pzip`] macro.
///
/// Most of the time, this trait will be provided by the Conformal framework.
/// However, we provide simple implementations for this trait for testing or
/// in other scenarios where you need to call process functions outside of
/// Conformal.
///
///  - [`ConstantBufferStates`] - A simple implementation where all parameters are constant.
///  - [`RampedStatesMap`] - A simple implementation where the parameter can be different at
///    the start and end of the buffer.
pub trait BufferStates {
    /// Get the state of a parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If there is no parameter with the given ID, this will return `None`.
    fn get_by_hash(
        &self,
        id_hash: IdHash,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    >;

    /// Get the state of a parameter by it's unique ID.
    ///
    /// If there is no parameter with the given ID, this will return `None`.
    fn get(
        &self,
        unique_id: &str,
    ) -> Option<
        BufferState<
            impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone,
            impl Iterator<Item = TimedValue<u32>> + Clone,
            impl Iterator<Item = TimedValue<bool>> + Clone,
        >,
    > {
        self.get_by_hash(hash_id(unique_id))
    }

    /// Get the state of a numeric parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If there is no parameter with the given ID, or the parameter is not numeric,
    /// this will return `None`.
    fn numeric_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Numeric(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the state of a numeric parameter by it's unique ID.
    ///
    /// If there is no parameter with the given ID, or the parameter is not numeric,
    /// this will return `None`.
    fn get_numeric(
        &self,
        unique_id: &str,
    ) -> Option<NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>> {
        self.numeric_by_hash(hash_id(unique_id))
    }

    /// Get the state of an enum parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If there is no parameter with the given ID, or the parameter is not an enum,
    /// this will return `None`.
    fn enum_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<EnumBufferState<impl Iterator<Item = TimedValue<u32>> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Enum(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the state of an enum parameter by it's unique ID.
    ///
    /// If there is no parameter with the given ID, or the parameter is not an enum,
    /// this will return `None`.
    fn get_enum(
        &self,
        unique_id: &str,
    ) -> Option<EnumBufferState<impl Iterator<Item = TimedValue<u32>> + Clone>> {
        self.enum_by_hash(hash_id(unique_id))
    }

    /// Get the state of a switch parameter by it's hashed unique ID.
    ///
    /// You can get the hash of a unique ID using [`hash_id`].
    ///
    /// If there is no parameter with the given ID, or the parameter is not a switch,
    /// this will return `None`.
    fn switch_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Switch(v)) => Some(v),
            _ => None,
        }
    }

    /// Get the state of a switch parameter by it's unique ID.
    ///
    /// If there is no parameter with the given ID, or the parameter is not a switch,
    /// this will return `None`.
    fn get_switch(
        &self,
        unique_id: &str,
    ) -> Option<SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone>> {
        self.switch_by_hash(hash_id(unique_id))
    }
}
