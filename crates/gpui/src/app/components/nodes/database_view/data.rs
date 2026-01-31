use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseViewNodeData {
    pub id: Uuid,

    #[serde(rename = "type")]
    pub node_type: String,

    pub metadata: DatabaseViewMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DatabaseViewMetadata {
    pub database_id: i32,
    pub view_ids: Vec<i32>,
}

impl DatabaseViewNodeData {
    pub fn new(id: Uuid, node_type: String, metadata: DatabaseViewMetadata) -> Self {
        Self {
            id,
            node_type,
            metadata,
        }
    }
}
