use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DividerNodeData {
    pub id: Uuid,

    #[serde(rename = "type")]
    pub node_type: String,
}

impl DividerNodeData {
    pub fn new(id: Uuid, node_type: String) -> Self {
        Self { id, node_type }
    }
}
