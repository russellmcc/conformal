use serde::{Deserialize, Serialize};

use component::parameters;

#[derive(Serialize, Deserialize)]
pub struct State {
    pub params: parameters::serialization::Snapshot,
}
