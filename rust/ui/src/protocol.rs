use std::{collections::HashMap, io::Write};

use base64::engine::general_purpose;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Value {
    #[serde(rename = "numeric")]
    Numeric(f32),
    #[serde(rename = "string")]
    String(String),
    #[serde(rename = "bytes")]
    #[serde(with = "serde_bytes")]
    Bytes(Vec<u8>),
    #[serde(rename = "bool")]
    Bool(bool),
}

impl From<f32> for Value {
    fn from(value: f32) -> Self {
        Value::Numeric(value)
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Value::String(value)
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Value::Bytes(value)
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}

impl From<conformal_component::parameters::Value> for Value {
    fn from(value: conformal_component::parameters::Value) -> Self {
        match value {
            conformal_component::parameters::Value::Numeric(value) => Value::Numeric(value),
            conformal_component::parameters::Value::Enum(value) => Value::String(value),
            conformal_component::parameters::Value::Switch(value) => Value::Bool(value),
        }
    }
}

impl TryFrom<Value> for conformal_component::parameters::Value {
    type Error = ();
    fn try_from(value: Value) -> Result<Self, Self::Error> {
        match value {
            Value::Numeric(value) => Ok(conformal_component::parameters::Value::Numeric(value)),
            Value::String(value) => Ok(conformal_component::parameters::Value::Enum(value)),
            Value::Bytes(_) => Err(()),
            Value::Bool(value) => Ok(conformal_component::parameters::Value::Switch(value)),
        }
    }
}

/// Requests are sent from the UI to the plugin.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "m")]
pub enum Request {
    /// Subscribe to a value at the given path.
    /// The plug-in will send a `Values` message soon with
    /// the current value, and again whenever the value changes.
    #[serde(rename = "subscribe")]
    Subscribe { path: String },

    /// Unsubscribe from a value at the given path.
    /// Note that this is just a hint for performance,
    /// it's still possible that the plug-in will send a `Values` message
    /// containing the value at this path in the future.
    #[serde(rename = "unsubscribe")]
    Unsubscribe { path: String },

    #[serde(rename = "set")]
    Set { path: String, value: Value },
}

/// Responses are sent from the plugin to the UI.
#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "m")]
pub enum Response {
    /// This provides the current state of a set of values.
    ///
    /// In general, this message will be sent whenever the UI subscribes
    /// to a new path or the value at a subscribed path has changed.
    ///
    /// However, it's valid for the plug-in to send spurious messages when the values
    /// haven't changed.
    ///
    /// Additionally, the plug-in is free to send values for paths that the UI hasn't subscribed to.
    #[serde(rename = "values")]
    Values { values: HashMap<String, Value> },

    /// This message is sent when `SubscribeValue` is called on a non-existent path.
    #[serde(rename = "subscribe_error")]
    SubscribeValueError { path: String },
}

pub mod parameter_info {
    //! These types are "extended" protocol types that are sent over the standard protocol
    //! as "bytes" params.

    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    #[serde(tag = "t")]
    pub enum TypeSpecific {
        #[serde(rename = "numeric")]
        Numeric {
            default: f32,
            valid_range: (f32, f32),
            units: String,
        },
        #[serde(rename = "enum")]
        Enum {
            default: String,
            values: Vec<String>,
        },
        #[serde(rename = "switch")]
        Switch { default: bool },
    }

    impl From<conformal_component::parameters::TypeSpecificInfo> for TypeSpecific {
        fn from(info: conformal_component::parameters::TypeSpecificInfo) -> Self {
            match info {
                conformal_component::parameters::TypeSpecificInfo::Numeric {
                    default,
                    valid_range,
                    units,
                } => Self::Numeric {
                    default,
                    valid_range: (*valid_range.start(), *valid_range.end()),
                    units: units.unwrap_or_else(String::new),
                },
                conformal_component::parameters::TypeSpecificInfo::Enum { default, values } => {
                    Self::Enum {
                        default: values[default as usize].clone(),
                        values,
                    }
                }
                conformal_component::parameters::TypeSpecificInfo::Switch { default } => {
                    Self::Switch { default }
                }
            }
        }
    }

    #[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
    pub struct Info {
        pub title: String,
        pub type_specific: TypeSpecific,
    }

    impl From<conformal_component::parameters::Info> for Info {
        fn from(info: conformal_component::parameters::Info) -> Self {
            Self {
                title: info.title,
                type_specific: info.type_specific.into(),
            }
        }
    }
}

pub fn make_serializer<Writer: Write>(
    write: &mut Writer,
) -> rmp_serde::Serializer<
    &mut Writer,
    rmp_serde::config::StructMapConfig<rmp_serde::config::DefaultConfig>,
> {
    rmp_serde::Serializer::new(write).with_struct_map()
}

pub fn serialize_as_bytes(message: &impl Serialize) -> Vec<u8> {
    let mut ret = Vec::with_capacity(128);
    message.serialize(&mut make_serializer(&mut ret)).unwrap();
    ret
}

#[cfg(test)]
pub fn deserialize_from_bytes<T: DeserializeOwned>(
    bytes: &[u8],
) -> Result<T, rmp_serde::decode::Error> {
    rmp_serde::from_read(bytes)
}

pub fn encode_message(message: &impl Serialize) -> String {
    let mut enc = base64::write::EncoderWriter::new(Vec::new(), &general_purpose::STANDARD);
    message.serialize(&mut make_serializer(&mut enc)).unwrap();
    String::from_utf8(enc.finish().unwrap()).unwrap()
}

pub fn decode_message<T: DeserializeOwned>(message: &str) -> Result<T, rmp_serde::decode::Error> {
    let mut dec = base64::read::DecoderReader::new(message.as_bytes(), &general_purpose::STANDARD);
    rmp_serde::from_read(&mut dec)
}

#[cfg(test)]
mod tests {
    use super::{Request, Response, Value, decode_message, encode_message};

    #[test]
    fn decode_defends_against_non_b64_chars() {
        assert!(decode_message::<Request>("***").is_err());
    }

    #[test]
    fn request_round_trip() {
        let request = Request::Subscribe {
            path: "foo".to_string(),
        };
        let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn request_set_bytes_round_trip() {
        let request = Request::Set {
            path: "foo".to_string(),
            value: Value::Bytes(vec![1, 2, 3]),
        };
        let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn request_set_string_round_trip() {
        let request = Request::Set {
            path: "foo".to_string(),
            value: Value::String("bar".to_string()),
        };
        let decoded = decode_message::<Request>(&encode_message(&request)).unwrap();
        assert_eq!(request, decoded);
    }

    #[test]
    fn response_round_trip() {
        let response = Response::Values {
            values: [("foo".to_string(), Value::Numeric(1.0))].into(),
        };
        let decoded = decode_message::<super::Response>(&encode_message(&response)).unwrap();
        assert_eq!(response, decoded);
    }
}
