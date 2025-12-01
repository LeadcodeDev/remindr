use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingNodeData {
    pub id: Uuid,
    pub metadata: HeadingMetadata,
}

impl HeadingNodeData {
    pub fn new(id: Uuid, metadata: HeadingMetadata) -> Self {
        Self { id, metadata }
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
