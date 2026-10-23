#[cfg(test)]
use remindr_core::testing::MemoryStore;

use remindr_core::domain::{Transaction, WorkspaceSnapshot};
use remindr_core::ports::{StoreError, WorkspaceStore};
use remindr_store::SqliteStore;

pub enum AppStore {
    Sqlite(SqliteStore),
    #[cfg(test)]
    Memory(MemoryStore),
}

impl WorkspaceStore for AppStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError> {
        match self {
            Self::Sqlite(store) => store.load(),
            #[cfg(test)]
            Self::Memory(store) => store.load(),
        }
    }

    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError> {
        match self {
            Self::Sqlite(store) => store.commit(transaction),
            #[cfg(test)]
            Self::Memory(store) => store.commit(transaction),
        }
    }
}
