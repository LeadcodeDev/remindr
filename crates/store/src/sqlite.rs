use std::path::Path;

use remindr_core::domain::{Transaction, WorkspaceSnapshot};
use remindr_core::ports::{StoreError, WorkspaceStore};
use rusqlite::Connection;

use crate::commit::commit;
use crate::error::write_error;
use crate::load::load;
use crate::migrate::migrate;

pub struct SqliteStore {
    pub(crate) connection: Connection,
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<SqliteStore, StoreError> {
        let mut connection = Connection::open(path).map_err(write_error)?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(write_error)?;
        connection
            .pragma_update(None, "synchronous", "NORMAL")
            .map_err(write_error)?;
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .map_err(write_error)?;
        migrate(&mut connection)?;
        Ok(SqliteStore { connection })
    }
}

impl WorkspaceStore for SqliteStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError> {
        load(&self.connection)
    }

    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError> {
        commit(&mut self.connection, transaction)
    }
}
