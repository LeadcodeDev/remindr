use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingNodeData {
    pub id: Uuid,
    pub metadata: Metadata,
}

impl HeadingNodeData {
    pub fn new(id: Uuid, metadata: Metadata) -> Self {
        Self { id, metadata }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub content: SharedString,
    pub level: u32,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            content: SharedString::new(""),
            level: 1,
        }
    }
}
