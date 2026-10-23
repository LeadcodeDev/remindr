use crate::domain::{Transaction, WorkspaceSnapshot};
use crate::ports::{StoreError, WorkspaceStore};

#[derive(Debug)]
pub struct MemoryStore;

#[allow(clippy::new_without_default)]
impl MemoryStore {
    #[must_use]
    pub fn new() -> Self {
        todo!()
    }

    #[must_use]
    pub fn shared(&self) -> MemoryStore {
        todo!()
    }

    pub fn fail_next_commit(&self) {
        todo!()
    }
}

impl WorkspaceStore for MemoryStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError> {
        todo!()
    }

    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError> {
        todo!()
    }
}
