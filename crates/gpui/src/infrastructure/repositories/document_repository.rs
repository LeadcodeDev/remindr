use anyhow::Result;
use sqlx::{SqlitePool, query, query_as};

use crate::{domain::database::document::Document, infrastructure::entities::DocumentEntity};

#[derive(Clone)]
pub struct DocumentRepository {
    pool: SqlitePool,
}

impl DocumentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_documents(&self) -> Result<Vec<Document>> {
        query_as::<_, DocumentEntity>("SELECT id, title, content FROM documents ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))
            .map(|documents| {
                documents
                    .into_iter()
                    .map(DocumentEntity::into)
                    .collect::<Vec<Document>>()
            })
    }

    pub async fn get_document_by_id(&self, id: i32) -> Result<Option<Document>> {
        query_as::<_, DocumentEntity>("SELECT id, title, content FROM documents WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|r| r.map(|r| r.into()))
            .map_err(|e| anyhow::Error::from(e))
    }

    pub async fn insert_document(&self, document: Document) -> Result<i32> {
        let res = query("INSERT INTO documents (title, content) VALUES (?, ?)")
            .bind(document.title)
            .bind(document.content)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))?;

        let last = res.last_insert_rowid();
        Ok(last as i32)
    }

    pub async fn update_document(&self, document: Document) -> Result<()> {
        query("UPDATE documents SET title = ?, content = ? WHERE id = ?")
            .bind(document.title)
            .bind(document.content)
            .bind(document.id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))?;

        Ok(())
    }
}
