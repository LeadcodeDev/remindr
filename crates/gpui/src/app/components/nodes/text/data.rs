use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNodeData {
    pub id: Uuid,

    #[serde(rename = "type")]
    pub node_type: String,

    pub metadata: TextMetadata,
}

impl TextNodeData {
    pub fn new(id: Uuid, node_type: String, metadata: TextMetadata) -> Self {
        Self {
            id,
            node_type,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextMetadata {
    pub content: SharedString,
}
