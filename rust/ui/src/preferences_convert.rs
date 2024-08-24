use super::protocol;
use preferences::Value;

impl From<Value> for protocol::Value {
    fn from(value: Value) -> Self {
        match value {
            Value::Switch(b) => protocol::Value::Bool(b),
        }
    }
}

pub enum ValueError {
    InvalidValue,
}

impl TryFrom<protocol::Value> for Value {
    type Error = ValueError;

    fn try_from(value: protocol::Value) -> Result<Self, Self::Error> {
        match value {
            protocol::Value::Bool(b) => Ok(Value::Switch(b)),
            _ => Err(ValueError::InvalidValue),
        }
    }
}
