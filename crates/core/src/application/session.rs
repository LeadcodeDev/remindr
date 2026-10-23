use crate::domain::{DomainError, Workspace};
use crate::ports::{Clock, StoreError, WorkspaceStore};

use super::history::History;
use super::{Changes, Command};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct Session<S: WorkspaceStore, C: Clock> {
    workspace: Workspace,
    store: S,
    clock: C,
    history: History,
}

impl<S: WorkspaceStore, C: Clock> Session<S, C> {
    pub fn open(store: S, clock: C) -> Result<Self, SessionError> {
        todo!()
    }

    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn execute(&mut self, command: Command) -> Result<Changes, SessionError> {
        todo!()
    }

    pub fn undo(&mut self) -> Result<Option<Changes>, SessionError> {
        todo!()
    }

    pub fn redo(&mut self) -> Result<Option<Changes>, SessionError> {
        todo!()
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        todo!()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        todo!()
    }
}
