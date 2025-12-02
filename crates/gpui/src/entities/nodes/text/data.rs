use gpui::SharedString;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextNodeData {
    pub id: Uuid,
    pub metadata: TextMetadata,
}

impl TextNodeData {
    pub fn new(id: Uuid, metadata: TextMetadata) -> Self {
        Self { id, metadata }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TextMetadata {
    pub content: SharedString,
}
