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

use conformal_component::parameters::{TypeSpecificInfoRef, Value as ParameterValue};
use serde::{Deserialize, Serialize};

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

impl<'a, S: AsRef<str>> From<TypeSpecificInfoRef<'a, S>> for WriteInfoRef<&'a [S]> {
    fn from(info: TypeSpecificInfoRef<'a, S>) -> Self {
        match info {
            TypeSpecificInfoRef::Numeric { .. } => Self::Numeric {},
            TypeSpecificInfoRef::Enum { values, .. } => Self::Enum { values },
            TypeSpecificInfoRef::Switch { .. } => Self::Switch {},
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

impl<'a, S: AsRef<str>> From<TypeSpecificInfoRef<'a, S>> for ReadInfoRef<std::slice::Iter<'a, S>> {
    fn from(info: TypeSpecificInfoRef<'a, S>) -> Self {
        match info {
            TypeSpecificInfoRef::Numeric {
                default,
                valid_range,
                ..
            } => Self::Numeric {
                default,
                valid_range,
            },
            TypeSpecificInfoRef::Enum {
                default, values, ..
            } => Self::Enum {
                default,
                values: values.iter(),
            },
            TypeSpecificInfoRef::Switch { default, .. } => Self::Switch { default },
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
                (WriteInfoRef::Numeric {}, ParameterValue::Numeric(value)) => {
                    Some(Value::Numeric(value))
                }
                (WriteInfoRef::Enum { .. }, ParameterValue::Enum(value)) => {
                    Some(Value::Enum(value))
                }
                (WriteInfoRef::Switch {}, ParameterValue::Switch(value)) => {
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
                        Ok(ParameterValue::Numeric(*value))
                    } else {
                        Err(DeserializationError::VersionTooNew())
                    }
                }
                (ReadInfoRef::Numeric { default, .. }, None) => {
                    Ok(ParameterValue::Numeric(default))
                }
                (ReadInfoRef::Enum { values, .. }, Some(Value::Enum(value))) => {
                    if values.into_iter().any(|v| v == value.as_str()) {
                        Ok(ParameterValue::Enum(std::mem::take(value)))
                    } else {
                        Err(DeserializationError::VersionTooNew())
                    }
                }
                (ReadInfoRef::Enum { default, values }, None) => Ok(ParameterValue::Enum(
                    values
                        .clone()
                        .into_iter()
                        .nth(default as usize)
                        .unwrap()
                        .to_string(),
                )),
                (ReadInfoRef::Switch { .. }, Some(Value::Switch(value))) => {
                    Ok(ParameterValue::Switch(*value))
                }
                (ReadInfoRef::Switch { default, .. }, None) => Ok(ParameterValue::Switch(default)),
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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::parameters::Snapshot;
    use conformal_component::parameters::Value;

    fn to_hash<'a, T, I: IntoIterator<Item = (&'a str, T)>>(i: I) -> HashMap<String, T> {
        i.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
    }

    #[test]
    fn defends_against_missing_info_in_serialize() {
        let snapshot = Snapshot {
            values: to_hash([("numeric", Value::Numeric(0.0))]),
        };
        let lookup = |_: &_| None;
        assert!(snapshot.into_serialize_no_enum(lookup).is_none());
    }

    #[test]
    fn defends_against_wrong_type_serialize() {
        let snapshot = Snapshot {
            values: to_hash([("numeric", Value::Numeric(0.0))]),
        };
        let lookup = |id: &_| match id {
            "0" => Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            }),
            _ => None,
        };
        let serialized = snapshot.into_serialize(lookup);
        assert!(serialized.is_none());
    }

    #[test]
    fn roundtrip() {
        let snapshot = Snapshot {
            values: to_hash([
                ("numeric", Value::Numeric(0.0)),
                ("enum", Value::Enum("b".to_string())),
                ("switch", Value::Switch(true)),
            ]),
        };
        let lookup = |id: &_| match id {
            "numeric" => Some(super::WriteInfoRef::Numeric {}),
            "enum" => Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            }),
            "switch" => Some(super::WriteInfoRef::Switch {}),
            _ => None,
        };
        let serialized = snapshot.clone().into_serialize(lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([
            (
                "numeric",
                super::ReadInfoRef::Numeric {
                    default: 0.0,
                    valid_range: 0.0..=1.0,
                },
            ),
            (
                "enum",
                super::ReadInfoRef::Enum {
                    default: 0,
                    values: ["a", "b", "c"],
                },
            ),
            ("switch", super::ReadInfoRef::Switch { default: true }),
        ]);
        assert!(deserialized.is_ok());

        assert_eq!(snapshot, deserialized.unwrap());
    }

    #[test]
    fn adding_parameters_without_migration() {
        let snapshot = Snapshot { values: [].into() };
        let lookup = |_: &_| None;
        let serialized = snapshot.into_serialize_no_enum(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([
            (
                "numeric",
                super::ReadInfoRef::Numeric {
                    default: 0.0,
                    valid_range: 0.0..=1.0,
                },
            ),
            (
                "enum",
                super::ReadInfoRef::Enum {
                    default: 0,
                    values: ["a", "b", "c"],
                },
            ),
            ("switch", super::ReadInfoRef::Switch { default: true }),
        ]);
        assert!(deserialized.is_ok());

        assert_eq!(
            deserialized.unwrap(),
            Snapshot {
                values: to_hash([
                    ("numeric", Value::Numeric(0.0)),
                    ("enum", Value::Enum("a".to_string())),
                    ("switch", Value::Switch(true)),
                ])
            }
        );
    }

    #[test]
    fn changing_parameter_type_without_migration_fails() {
        let snapshot = Snapshot {
            values: to_hash([("numeric", Value::Numeric(0.0))]),
        };
        let lookup = |id: &_| match id {
            "numeric" => Some(super::WriteInfoRef::Numeric {}),
            _ => None,
        };
        let serialized = snapshot.into_serialize_no_enum(lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([(
            "numeric",
            super::ReadInfoRef::Enum {
                default: 0,
                values: ["a", "b", "c"],
            },
        )]);
        assert!(deserialized.is_err());
    }

    #[test]
    fn add_new_enum_parameter_without_migration() {
        let snapshot = Snapshot {
            values: to_hash([("enum", Value::Enum("b".to_string()))]),
        };
        let lookup = |_: &_| {
            Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            })
        };
        let serialized = snapshot.into_serialize(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([(
            "enum",
            super::ReadInfoRef::Enum {
                default: 0,
                values: ["a", "b", "c", "d"],
            },
        )]);
        assert!(deserialized.is_ok());

        assert_eq!(
            deserialized.unwrap(),
            Snapshot {
                values: to_hash([("enum", Value::Enum("b".to_string())),])
            }
        );
    }

    #[test]
    fn removing_parameter_without_migration() {
        let snapshot = Snapshot {
            values: to_hash([("enum", Value::Enum("b".to_string()))]),
        };
        let lookup = |_: &_| {
            Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            })
        };
        let serialized = snapshot.into_serialize(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized
            .unwrap()
            .into_snapshot::<std::iter::Empty<&'_ str>>([]);
        assert!(deserialized.is_ok());

        assert_eq!(deserialized.unwrap(), Snapshot { values: [].into() });
    }

    #[test]
    fn re_ordering_enum_without_migration() {
        let snapshot = Snapshot {
            values: to_hash([("enum", Value::Enum("b".to_string()))]),
        };
        let lookup = |_: &_| {
            Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            })
        };
        let serialized = snapshot.into_serialize(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([(
            "enum",
            super::ReadInfoRef::Enum {
                default: 0,
                values: ["b", "c", "a"],
            },
        )]);
        assert!(deserialized.is_ok());

        assert_eq!(
            deserialized.unwrap(),
            Snapshot {
                values: to_hash([("enum", Value::Enum("b".to_string()))])
            }
        );
    }

    #[test]
    fn narrowing_range_causes_too_new() {
        let snapshot = Snapshot {
            values: to_hash([("numeric", Value::Numeric(0.7))]),
        };
        let lookup = |_: &_| Some(super::WriteInfoRef::Numeric {});
        let serialized = snapshot.into_serialize_no_enum(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot_no_enums([(
            "numeric",
            super::ReadInfoRef::Numeric {
                default: 0.0,
                valid_range: 0.0..=0.5,
            },
        )]);
        assert_eq!(
            deserialized,
            Err(super::DeserializationError::VersionTooNew())
        );
    }

    #[test]
    fn removing_enum_value_causes_too_new() {
        let snapshot = Snapshot {
            values: to_hash([("enum", Value::Enum("b".to_string()))]),
        };
        let lookup = |_: &_| {
            Some(super::WriteInfoRef::Enum {
                values: ["a", "b", "c"],
            })
        };
        let serialized = snapshot.into_serialize(&lookup);
        assert!(serialized.is_some());

        let deserialized = serialized.unwrap().into_snapshot([(
            "enum",
            super::ReadInfoRef::Enum {
                default: 0,
                values: ["a", "c"],
            },
        )]);
        assert_eq!(
            deserialized,
            Err(super::DeserializationError::VersionTooNew())
        );
    }
}
