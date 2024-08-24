//! This holds utilities for serializing the state of a set of parameters.
//!
//! Our serialization model allows backwards compatibility, but _not_ forwards compatibility.
//! This means that a new version of the plug-in will be able to load the state of an old version,
//! but an old version will not be able to load the state of a new version.
//!
//! # Data model changes
//!
//! The data format is designed to allow certain changes without explicit migrations
//!
//! - Adding a parameter (given the parameter's default setting matches the old behavior).
//!   - Note that the parameter's unique id must have never been used before
//! - Removing a parameter.
//! - Changing the default value of a parameter.
//!
//! If your parameter is _not_ automatable, some additional changes are allowed without
//! explicit migrations:
//!
//! - Re-ordering enum values.
//! - Increasing the allowed range of a numeric parameter.
//! - Adding a new enum values to the end of the list.
//!
//! Other changes will need explicit migrations (to be supported after #19).
//!
//! ## Automatable parameter restrictions
//!
//! If your parameter is automatable, the following changes are _not_ allowed,
//! even with explicit migrations. These restrictions are due to the vst3 format's
//! data model for automation.
//!
//! - Re-ordering or removing existing enum values.
//! - Changing the range of a numeric parameter.
//! - Adding a value to an enum parameter.
//! - Changing the type of a parameter.
//!
//! If you need to make one of these changes, you should remove the parameter and
//! create a new parameter with a new ID instead.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Value {
    Numeric(f32),
    Enum(String),
    Switch(bool),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    values: HashMap<String, Value>,
}

/// This contains metadata needed to serialize a parameter.
///
/// Note in particular this does not contain the range or default
/// of the parameter - this ensures that increasing the range or
/// changing the default value of a parameter does not require
/// a migration!
pub enum WriteInfoRef<I> {
    Numeric {},
    Enum { values: I },
    Switch {},
}

impl<'a, S: AsRef<str>> From<super::TypeSpecificInfoRef<'a, S>> for WriteInfoRef<&'a [S]> {
    fn from(info: super::TypeSpecificInfoRef<'a, S>) -> Self {
        match info {
            super::TypeSpecificInfoRef::Numeric { .. } => Self::Numeric {},
            super::TypeSpecificInfoRef::Enum { values, .. } => Self::Enum { values },
            super::TypeSpecificInfoRef::Switch { .. } => Self::Switch {},
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReadInfoRef<I> {
    Numeric {
        default: f32,
        valid_range: std::ops::RangeInclusive<f32>,
    },
    Enum {
        default: u32,
        values: I,
    },
    Switch {
        default: bool,
    },
}

impl<'a, S: AsRef<str>> From<super::TypeSpecificInfoRef<'a, S>>
    for ReadInfoRef<std::slice::Iter<'a, S>>
{
    fn from(info: super::TypeSpecificInfoRef<'a, S>) -> Self {
        match info {
            super::TypeSpecificInfoRef::Numeric {
                default,
                valid_range,
                ..
            } => Self::Numeric {
                default,
                valid_range,
            },
            super::TypeSpecificInfoRef::Enum {
                default, values, ..
            } => Self::Enum {
                default,
                values: values.iter(),
            },
            super::TypeSpecificInfoRef::Switch { default, .. } => Self::Switch { default },
        }
    }
}

impl super::Snapshot {
    /// Convert a snapshot to a serialized snapshot.
    ///
    /// This will allocate.
    ///
    /// Note that this will only fail if there is an inconsistency between the snapshot and the
    /// provided info.
    pub fn into_serialize<'a, I: IntoIterator<Item = &'a str>>(
        self,
        lookup: impl Fn(&str) -> Option<WriteInfoRef<I>>,
    ) -> Option<Snapshot> {
        let mut values = HashMap::new();
        values.reserve(self.values.len());
        for (id, value) in self.values {
            let info = lookup(id.as_str())?;
            let serialized_value = match (info, value) {
                (WriteInfoRef::Numeric {}, super::Value::Numeric(value)) => {
                    Some(Value::Numeric(value))
                }
                (WriteInfoRef::Enum { .. }, super::Value::Enum(value)) => Some(Value::Enum(value)),
                (WriteInfoRef::Switch {}, super::Value::Switch(value)) => {
                    Some(Value::Switch(value))
                }
                _ => None,
            }?;

            values.insert(id.clone(), serialized_value);
        }

        Some(Snapshot { values })
    }

    pub fn into_serialize_no_enum(
        self,
        lookup: impl Fn(&str) -> Option<WriteInfoRef<std::iter::Empty<&'static str>>>,
    ) -> Option<Snapshot> {
        self.into_serialize(lookup)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SnapshotCorruptionError {
    /// Changing the type of a parameter requires a migration, so it's an error
    /// if we try to load a snapshot that has a different type for a parameter.
    IncompatibleType(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeserializationError {
    /// The snapshot was saved with a newer version of the plug-in, so it's not
    /// compatible with this version.
    VersionTooNew(),

    Corrupted(SnapshotCorruptionError),
}

impl Snapshot {
    /// Convert a serialized snapshot to a snapshot.
    ///
    /// # Panics
    ///
    /// Panics if `all_params` is corrupted (e.g., has a default value for the enum that
    /// is outside the values range)
    ///
    /// # Errors
    ///
    /// Will return `DeserializationError::VersionTooNew` error if the serialized snapshot has
    /// parameters that are out of range,, or `DeserializationError::Corrupted` if any parameters
    /// were the wrong type.
    pub fn into_snapshot<'a, I: IntoIterator<Item = &'a str> + Clone>(
        mut self,
        all_params: impl IntoIterator<Item = (&'a str, ReadInfoRef<I>)>,
    ) -> Result<super::Snapshot, DeserializationError> {
        let mut values = HashMap::new();
        for (id, info) in all_params {
            let serialized_value = self.values.get_mut(id);
            let value = match (info, serialized_value) {
                (ReadInfoRef::Numeric { valid_range, .. }, Some(Value::Numeric(value))) => {
                    if valid_range.contains(value) {
                        Ok(super::Value::Numeric(*value))
                    } else {
                        Err(DeserializationError::VersionTooNew())
                    }
                }
                (ReadInfoRef::Numeric { default, .. }, None) => Ok(super::Value::Numeric(default)),
                (ReadInfoRef::Enum { values, .. }, Some(Value::Enum(value))) => {
                    if values.into_iter().any(|v| v == value.as_str()) {
                        Ok(super::Value::Enum(std::mem::take(value)))
                    } else {
                        Err(DeserializationError::VersionTooNew())
                    }
                }
                (ReadInfoRef::Enum { default, values }, None) => Ok(super::Value::Enum(
                    values
                        .clone()
                        .into_iter()
                        .nth(default as usize)
                        .unwrap()
                        .to_string(),
                )),
                (ReadInfoRef::Switch { .. }, Some(Value::Switch(value))) => {
                    Ok(super::Value::Switch(*value))
                }
                (ReadInfoRef::Switch { default, .. }, None) => Ok(super::Value::Switch(default)),
                // Note that changing parameter types requires a migration, so
                // if the type in the snapshot doesn't match the type in the info,
                // it's invalid.
                _ => Err(DeserializationError::Corrupted(
                    SnapshotCorruptionError::IncompatibleType(id.to_string()),
                )),
            }?;

            values.insert(id.to_owned(), value);
        }

        Ok(super::Snapshot { values })
    }

    #[cfg(test)]
    fn into_snapshot_no_enums<'a>(
        self,
        all_params: impl IntoIterator<Item = (&'a str, ReadInfoRef<std::iter::Empty<&'a str>>)>,
    ) -> Result<super::Snapshot, DeserializationError> {
        self.into_snapshot(all_params)
    }
}
