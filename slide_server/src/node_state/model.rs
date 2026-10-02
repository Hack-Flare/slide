use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
pub struct NodeIdentity {
    pub node_id: String,
    pub node_name: String,
}
