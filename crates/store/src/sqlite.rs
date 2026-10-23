use std::path::Path;

use remindr_core::domain::{Transaction, WorkspaceSnapshot};
use remindr_core::ports::{StoreError, WorkspaceStore};
use rusqlite::Connection;

pub struct SqliteStore {
    connection: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        let connection = Connection::open(path).map_err(write_error)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(write_error)?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(write_error)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(write_error)?;
        Ok(SqliteStore { connection })
    }
}

impl WorkspaceStore for SqliteStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError> {
        todo!()
    }

    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError> {
        todo!()
    }
}

fn write_error(error: rusqlite::Error) -> StoreError {
    StoreError::Write(Box::new(error))
}
