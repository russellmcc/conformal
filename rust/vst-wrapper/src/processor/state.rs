use serde::{Deserialize, Serialize};

use conformal_core::parameters::serialization;

#[derive(Serialize, Deserialize)]
pub struct State {
    pub params: serialization::Snapshot,
}
