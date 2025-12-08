use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingNodeData {
    pub id: Uuid,

    #[serde(rename = "type")]
    pub node_type: String,

    pub metadata: HeadingMetadata,
}

impl HeadingNodeData {
    pub fn new(id: Uuid, node_type: String, metadata: HeadingMetadata) -> Self {
        Self {
            id,
            node_type,
            metadata,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingMetadata {
    pub content: SharedString,
    pub level: u32,
}

impl Default for HeadingMetadata {
    fn default() -> Self {
        Self {
            content: SharedString::new(""),
            level: 1,
        }
    }
}
