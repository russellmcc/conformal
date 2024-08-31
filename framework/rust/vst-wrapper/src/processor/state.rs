use serde::{Deserialize, Serialize};

use conformal_component::parameters;

#[derive(Serialize, Deserialize)]
pub struct State {
    pub params: parameters::serialization::Snapshot,
}
