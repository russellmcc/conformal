use std::{
    collections::HashMap,
    ops::{Range, RangeBounds, RangeInclusive},
};

#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

pub mod serialization;
pub mod utils;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpecificInfoRef<'a, S> {
    Enum {
        default: u32,

        /// Note that values _must_ contain at least 2 elements.
        values: &'a [S],
    },
    Numeric {
        default: f32,
        valid_range: RangeInclusive<f32>,
        // Here an empty string indicates unitless values.
        units: &'a str,
    },
    Switch {
        default: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpecificInfo {
    Enum {
        default: u32,
        values: Vec<String>,
    },
    Numeric {
        default: f32,
        valid_range: std::ops::RangeInclusive<f32>,
        units: String,
    },
    Switch {
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
                units: (*units).to_string(),
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
                units,
            },
            TypeSpecificInfo::Switch { default } => {
                TypeSpecificInfoRef::Switch { default: *default }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flags {
    pub automatable: bool,
}

impl Default for Flags {
    fn default() -> Self {
        Flags { automatable: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct InfoRef<'a, S> {
    pub unique_id: &'a str,
    pub title: &'a str,
    pub short_title: &'a str,
    pub flags: Flags,
    pub type_specific: TypeSpecificInfoRef<'a, S>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Info {
    pub unique_id: String,
    pub title: String,
    pub short_title: String,
    pub flags: Flags,
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

pub type StaticInfoRef = InfoRef<'static, &'static str>;

pub fn to_infos(v: &[InfoRef<'_, &'_ str>]) -> Vec<Info> {
    v.iter().map(Into::into).collect()
}

pub type IdHash = u32;

/// Note that we use strings as the canonical ID for params,
/// however, in environments where we never want to allocate,
/// (such as audio processing code), we refer to parameters
/// by a hash of their ID. This is also what we use for plug-in
/// formats that require a numeric ID (such as VST3)
#[must_use]
pub fn hash_id(unique_id: &str) -> IdHash {
    fxhash::hash32(unique_id) & 0x7fff_ffff
}

/// This is used when performance is critical and we don't want to
/// refer to enums by their string values.
#[derive(Debug, Clone, PartialEq, Copy)]
pub enum InternalValue {
    Numeric(f32),
    Enum(u32),
    Switch(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Numeric(f32),
    Enum(String),
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

/// This represents the current state of all parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub values: HashMap<String, Value>,
}

/// Represents a snapshot of all valid parameters at a given point in time.
///
/// For convenience, we provide `get_numeric`, `get_enum`, and `get_switch` functions
/// which return the value of the parameter if it is of the correct type, or `None`
/// otherwise. Note that all parmeter types re-use the same `ID` space, so
/// only one of the specialized `get` methods will return a value for a given `ParameterID`.
pub trait States {
    fn get_by_hash(&self, id_hash: IdHash) -> Option<InternalValue>;

    fn get(&self, unique_id: &str) -> Option<InternalValue> {
        self.get_by_hash(hash_id(unique_id))
    }

    fn numeric_by_hash(&self, id_hash: IdHash) -> Option<f32> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Numeric(v)) => Some(v),
            _ => None,
        }
    }

    fn get_numeric(&self, unique_id: &str) -> Option<f32> {
        self.numeric_by_hash(hash_id(unique_id))
    }

    fn enum_by_hash(&self, id_hash: IdHash) -> Option<u32> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Enum(v)) => Some(v),
            _ => None,
        }
    }

    fn get_enum(&self, unique_id: &str) -> Option<u32> {
        self.enum_by_hash(hash_id(unique_id))
    }

    fn switch_by_hash(&self, id_hash: IdHash) -> Option<bool> {
        match self.get_by_hash(id_hash) {
            Some(InternalValue::Switch(v)) => Some(v),
            _ => None,
        }
    }

    fn get_switch(&self, unique_id: &str) -> Option<bool> {
        self.switch_by_hash(hash_id(unique_id))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PiecewiseLinearCurvePoint {
    pub sample_offset: usize,
    pub value: f32,
}

/// A Parameter piecewise linear curve represents a value that changes
/// over the course of the buffer, moving linearly from point to point.
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

#[derive(Debug, Clone, PartialEq)]
pub struct TimedValue<V> {
    pub sample_offset: usize,
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

/// This is equivalent to `PiecewiseLinearCurve` but for Enums. There
/// is no interpolation, since the value is enumerated.
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

/// This is equivalent to `PiecewiseLinearCurve` but for Switch. There
/// is no interpolation, since the value is switched
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
    Constant(f32),
    PiecewiseLinear(PiecewiseLinearCurve<I>),
}

impl<I: IntoIterator<Item = PiecewiseLinearCurvePoint>> NumericBufferState<I> {
    #[allow(clippy::missing_panics_doc)] // Only panics when invariants are broken.
    pub fn value_at_start_of_buffer(self) -> f32 {
        match self {
            NumericBufferState::Constant(v) => v,
            NumericBufferState::PiecewiseLinear(v) => v.points.into_iter().next().unwrap().value,
        }
    }
}

/// Represents the state of an enum value across a buffer
pub enum EnumBufferState<I> {
    Constant(u32),
    Varying(TimedEnumValues<I>),
}

impl<I: IntoIterator<Item = TimedValue<u32>>> EnumBufferState<I> {
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
    Constant(bool),
    Varying(TimedSwitchValues<I>),
}

impl<I: IntoIterator<Item = TimedValue<bool>>> SwitchBufferState<I> {
    #[allow(clippy::missing_panics_doc)] // Only panics when invariants are broken.
    pub fn value_at_start_of_buffer(self) -> bool {
        match self {
            SwitchBufferState::Constant(v) => v,
            SwitchBufferState::Varying(v) => v.points.into_iter().next().unwrap().value,
        }
    }
}

pub enum BufferState<N, E, S> {
    Numeric(NumericBufferState<N>),
    Enum(EnumBufferState<E>),
    Switch(SwitchBufferState<S>),
}

/// Represents the state of all parameters across a buffer.
///
/// Each parameter is marked as constant of varying to allow optimizations in
/// the constant case.
pub trait BufferStates {
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

    fn numeric_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Numeric(v)) => Some(v),
            _ => None,
        }
    }

    fn get_numeric(
        &self,
        unique_id: &str,
    ) -> Option<NumericBufferState<impl Iterator<Item = PiecewiseLinearCurvePoint> + Clone>> {
        self.numeric_by_hash(hash_id(unique_id))
    }

    fn enum_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<EnumBufferState<impl Iterator<Item = TimedValue<u32>> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Enum(v)) => Some(v),
            _ => None,
        }
    }

    fn get_enum(
        &self,
        unique_id: &str,
    ) -> Option<EnumBufferState<impl Iterator<Item = TimedValue<u32>> + Clone>> {
        self.enum_by_hash(hash_id(unique_id))
    }

    fn switch_by_hash(
        &self,
        param_id: IdHash,
    ) -> Option<SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone>> {
        match self.get_by_hash(param_id) {
            Some(BufferState::Switch(v)) => Some(v),
            _ => None,
        }
    }

    fn get_switch(
        &self,
        unique_id: &str,
    ) -> Option<SwitchBufferState<impl Iterator<Item = TimedValue<bool>> + Clone>> {
        self.switch_by_hash(hash_id(unique_id))
    }
}

pub mod store;
