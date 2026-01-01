use anyhow::Error;
use sqlx::{SqlitePool, query, query_as};

use crate::{domain::database::document::DocumentModel, infrastructure::entities::DocumentEntity};

#[derive(Clone)]
pub struct DocumentRepository {
    pool: SqlitePool,
}

impl DocumentRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn get_documents(&self) -> Result<Vec<DocumentModel>, Error> {
        query_as::<_, DocumentEntity>("SELECT id, title, content FROM documents ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))
            .map(|documents| {
                documents
                    .into_iter()
                    .map(DocumentEntity::into)
                    .collect::<Vec<DocumentModel>>()
            })
    }

    pub async fn get_document_by_id(&self, id: i32) -> Result<DocumentModel, Error> {
        query_as::<_, DocumentEntity>("SELECT id, title, content FROM documents WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map(|r| r.into())
            .map_err(|e| anyhow::Error::from(e))
    }

    pub async fn insert_document(&self, document: DocumentModel) -> Result<i32, Error> {
        let res = query("INSERT INTO documents (title, content) VALUES (?, ?)")
            .bind(document.title)
            .bind(document.content)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))?;

        let last = res.last_insert_rowid();
        Ok(last as i32)
    }

    pub async fn update_document(&self, document: DocumentModel) -> Result<(), Error> {
        query("UPDATE documents SET title = $1, content = $2 WHERE id = $3")
            .bind(document.title)
            .bind(document.content)
            .bind(document.id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))?;

        Ok(())
    }

    pub async fn delete_document(&self, id: i32) -> Result<(), Error> {
        query("DELETE FROM documents WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| anyhow::Error::from(e))?;

        Ok(())
    }
}
