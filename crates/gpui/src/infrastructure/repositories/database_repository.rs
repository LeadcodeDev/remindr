use anyhow::Error;
use sqlx::{SqlitePool, query, query_as};

use crate::{
    domain::database::{
        database_cell::DatabaseCellModel, database_column::DatabaseColumnModel,
        database_model::DatabaseModel, database_row::DatabaseRowModel,
        database_view::DatabaseViewModel,
    },
    infrastructure::entities::{
        DatabaseCellEntity, DatabaseColumnEntity, DatabaseEntity, DatabaseRowEntity,
        DatabaseViewEntity,
    },
};

#[derive(Clone)]
pub struct DatabaseRepository {
    pool: SqlitePool,
}

impl DatabaseRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // ── Databases ──

    pub async fn get_databases(&self) -> Result<Vec<DatabaseModel>, Error> {
        query_as::<_, DatabaseEntity>("SELECT id, name FROM databases ORDER BY id ASC")
            .fetch_all(&self.pool)
            .await
            .map_err(Error::from)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_database_by_id(&self, id: i32) -> Result<DatabaseModel, Error> {
        query_as::<_, DatabaseEntity>("SELECT id, name FROM databases WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
            .map(Into::into)
            .map_err(Error::from)
    }

    pub async fn insert_database(&self, name: String) -> Result<i32, Error> {
        let res = query("INSERT INTO databases (name) VALUES (?)")
            .bind(name)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(res.last_insert_rowid() as i32)
    }

    pub async fn update_database(&self, model: DatabaseModel) -> Result<(), Error> {
        query("UPDATE databases SET name = ? WHERE id = ?")
            .bind(model.name)
            .bind(model.id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    pub async fn delete_database(&self, id: i32) -> Result<(), Error> {
        query("DELETE FROM databases WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    // ── Columns ──

    pub async fn get_columns(&self, database_id: i32) -> Result<Vec<DatabaseColumnModel>, Error> {
        query_as::<_, DatabaseColumnEntity>(
            "SELECT id, database_id, label, column_type, config, position FROM database_columns WHERE database_id = ? ORDER BY position ASC"
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::from)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub async fn insert_column(
        &self,
        database_id: i32,
        label: String,
        column_type: &str,
        config: &str,
        position: i32,
    ) -> Result<i32, Error> {
        let res = query(
            "INSERT INTO database_columns (database_id, label, column_type, config, position) VALUES (?, ?, ?, ?, ?)"
        )
        .bind(database_id)
        .bind(label)
        .bind(column_type)
        .bind(config)
        .bind(position)
        .execute(&self.pool)
        .await
        .map_err(Error::from)?;
        Ok(res.last_insert_rowid() as i32)
    }

    pub async fn update_column(&self, model: DatabaseColumnModel) -> Result<(), Error> {
        let config_json = serde_json::to_string(&model.config).unwrap_or_else(|_| "{}".to_string());
        query(
            "UPDATE database_columns SET label = ?, column_type = ?, config = ?, position = ? WHERE id = ?"
        )
        .bind(model.label)
        .bind(model.column_type.as_str())
        .bind(config_json)
        .bind(model.position)
        .bind(model.id)
        .execute(&self.pool)
        .await
        .map_err(Error::from)?;
        Ok(())
    }

    pub async fn delete_column(&self, id: i32) -> Result<(), Error> {
        query("DELETE FROM database_columns WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    // ── Rows ──

    pub async fn get_rows(&self, database_id: i32) -> Result<Vec<DatabaseRowModel>, Error> {
        query_as::<_, DatabaseRowEntity>(
            "SELECT id, database_id FROM database_rows WHERE database_id = ? ORDER BY id ASC",
        )
        .bind(database_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::from)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub async fn insert_row(&self, database_id: i32) -> Result<i32, Error> {
        let res = query("INSERT INTO database_rows (database_id) VALUES (?)")
            .bind(database_id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(res.last_insert_rowid() as i32)
    }

    pub async fn delete_row(&self, id: i32) -> Result<(), Error> {
        query("DELETE FROM database_rows WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    // ── Cells ──

    pub async fn get_cells(&self, row_ids: &[i32]) -> Result<Vec<DatabaseCellModel>, Error> {
        if row_ids.is_empty() {
            return Ok(Vec::new());
        }
        let placeholders: Vec<String> = row_ids.iter().map(|_| "?".to_string()).collect();
        let sql = format!(
            "SELECT id, row_id, column_id, value FROM database_cells WHERE row_id IN ({})",
            placeholders.join(", ")
        );
        let mut q = sqlx::query_as::<_, DatabaseCellEntity>(&sql);
        for id in row_ids {
            q = q.bind(id);
        }
        q.fetch_all(&self.pool)
            .await
            .map_err(Error::from)
            .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub async fn upsert_cell(&self, row_id: i32, column_id: i32, value: &str) -> Result<(), Error> {
        query(
            "INSERT INTO database_cells (row_id, column_id, value) VALUES (?, ?, ?) ON CONFLICT(row_id, column_id) DO UPDATE SET value = excluded.value"
        )
        .bind(row_id)
        .bind(column_id)
        .bind(value)
        .execute(&self.pool)
        .await
        .map_err(Error::from)?;
        Ok(())
    }

    // ── Views ──

    pub async fn get_views(&self) -> Result<Vec<DatabaseViewModel>, Error> {
        query_as::<_, DatabaseViewEntity>(
            "SELECT id, database_id, name, view_type, folder_id FROM database_views ORDER BY id ASC"
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::from)
        .map(|rows| rows.into_iter().map(Into::into).collect())
    }

    pub async fn get_view_by_id(&self, id: i32) -> Result<DatabaseViewModel, Error> {
        query_as::<_, DatabaseViewEntity>(
            "SELECT id, database_id, name, view_type, folder_id FROM database_views WHERE id = ?",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map(Into::into)
        .map_err(Error::from)
    }

    pub async fn insert_view(
        &self,
        database_id: i32,
        name: String,
        view_type: &str,
        folder_id: Option<i32>,
    ) -> Result<i32, Error> {
        let res = query(
            "INSERT INTO database_views (database_id, name, view_type, folder_id) VALUES (?, ?, ?, ?)"
        )
        .bind(database_id)
        .bind(name)
        .bind(view_type)
        .bind(folder_id)
        .execute(&self.pool)
        .await
        .map_err(Error::from)?;
        Ok(res.last_insert_rowid() as i32)
    }

    pub async fn update_view(&self, model: DatabaseViewModel) -> Result<(), Error> {
        query("UPDATE database_views SET name = ?, folder_id = ? WHERE id = ?")
            .bind(model.name)
            .bind(model.folder_id)
            .bind(model.id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }

    pub async fn delete_view(&self, id: i32) -> Result<(), Error> {
        query("DELETE FROM database_views WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(Error::from)?;
        Ok(())
    }
}
