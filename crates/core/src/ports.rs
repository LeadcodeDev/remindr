use crate::domain::{Timestamp, Transaction, WorkspaceSnapshot};

pub trait WorkspaceStore {
    fn load(&self) -> Result<WorkspaceSnapshot, StoreError>;
    fn commit(&mut self, transaction: &Transaction) -> Result<(), StoreError>;
}

pub trait Clock {
    fn now(&self) -> Timestamp;
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("storage read failed")]
    Read(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("storage write failed")]
    Write(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("stored data is corrupt: {0}")]
    Corrupt(String),
}
