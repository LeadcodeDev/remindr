use gpui_kit::{Context, EventEmitter};
use remindr_core::application::{Changes, Command, Session, SessionError};
use remindr_core::domain::Workspace;

use super::clock::SystemClock;
use super::store::AppStore;

pub struct WorkspaceModel {
    session: Session<AppStore, SystemClock>,
}

impl EventEmitter<Changes> for WorkspaceModel {}

impl WorkspaceModel {
    #[must_use]
    pub fn new(session: Session<AppStore, SystemClock>) -> Self {
        Self { session }
    }

    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        self.session.workspace()
    }

    pub fn execute(
        &mut self,
        command: Command,
        cx: &mut Context<Self>,
    ) -> Result<Changes, SessionError> {
        let changes = self.session.execute(command)?;
        cx.emit(changes.clone());
        cx.notify();
        Ok(changes)
    }

    pub fn undo(&mut self, cx: &mut Context<Self>) -> Result<Option<Changes>, SessionError> {
        let changes = self.session.undo()?;
        Self::publish(changes.as_ref(), cx);
        Ok(changes)
    }

    pub fn redo(&mut self, cx: &mut Context<Self>) -> Result<Option<Changes>, SessionError> {
        let changes = self.session.redo()?;
        Self::publish(changes.as_ref(), cx);
        Ok(changes)
    }

    fn publish(changes: Option<&Changes>, cx: &mut Context<Self>) {
        if let Some(changes) = changes {
            cx.emit(changes.clone());
            cx.notify();
        }
    }
}
