use std::cell::RefCell;
use std::rc::Rc;

use crate::domain::{Transaction, Workspace, WorkspaceSnapshot};
use crate::ports::{StoreError, WorkspaceStore};

#[derive(Debug, thiserror::Error)]
#[error("injected commit failure")]
struct InjectedFailure;

#[derive(Debug, Default)]
struct Storage {
    initial: WorkspaceSnapshot,
    committed: Vec<Transaction>,
    fail_next_commit: bool,
}

#[derive(Debug)]
pub struct MemoryStore {
    storage: Rc<RefCell<Storage>>,
}

#[allow(clippy::new_without_default)]
impl MemoryStore {
    #[must_use]
    pub fn new() -> Self {
        Self::with_snapshot(WorkspaceSnapshot::default())
    }

    #[must_use]
    pub fn with_snapshot(initial: WorkspaceSnapshot) -> Self {
        Self {
            storage: Rc::new(RefCell::new(Storage {
                initial,
                ..Storage::default()
            })),
        }
    }

    #[must_use]
    pub fn shared(&self) -> MemoryStore {
        Self {
            storage: Rc::clone(&self.storage),
        }
    }

    pub fn fail_next_commit(&self) {
        self.storage.borrow_mut().fail_next_commit = true;
    }

    #[must_use]
    pub fn committed(&self) -> usize {
        self.storage.borrow().committed.len()
    }
}

impl WorkspaceStore for MemoryStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError> {
        let storage = self.storage.borrow();
        let mut workspace = Workspace::from_snapshot(storage.initial.clone())
            .map_err(|error| StoreError::Corrupt(error.to_string()))?;
        for transaction in &storage.committed {
            workspace
                .apply(transaction)
                .map_err(|error| StoreError::Corrupt(error.to_string()))?;
        }
        Ok(workspace.to_snapshot())
    }

    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError> {
        let mut storage = self.storage.borrow_mut();
        if storage.fail_next_commit {
            storage.fail_next_commit = false;
            return Err(StoreError::Write(Box::new(InjectedFailure)));
        }
        storage.committed.push(transaction.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::{Transaction, WorkspaceSnapshot};
    use crate::ports::WorkspaceStore;

    use super::MemoryStore;

    #[test]
    fn injected_failure_hits_one_commit_only() {
        let mut store = MemoryStore::new();
        store.fail_next_commit();
        assert!(store.commit(&Transaction::default()).is_err());
        assert!(store.commit(&Transaction::default()).is_ok());
        assert_eq!(store.shared().committed(), 1);
    }

    #[test]
    fn empty_store_loads_an_empty_snapshot() {
        assert_eq!(
            MemoryStore::new().load().unwrap(),
            WorkspaceSnapshot::default()
        );
    }
}
