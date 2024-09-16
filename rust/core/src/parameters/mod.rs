use std::collections::HashMap;

use conformal_component::parameters::Value;

pub mod serialization;

pub mod store;

/// This represents the current state of all parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub values: HashMap<String, Value>,
}
