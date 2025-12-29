use serde_json::Value;
use sqlx::prelude::FromRow;

use crate::domain::database::document::Document;

#[derive(Debug, FromRow)]
pub struct DocumentEntity {
    pub id: i32,
    pub title: String,
    pub content: Value,
}

impl From<DocumentEntity> for Document {
    fn from(entity: DocumentEntity) -> Self {
        Document {
            id: entity.id,
            title: entity.title,
            content: entity.content,
        }
    }
}
