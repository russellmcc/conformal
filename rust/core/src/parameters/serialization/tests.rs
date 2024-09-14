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
