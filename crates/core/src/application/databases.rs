use crate::domain::{DomainError, Timestamp, Workspace};

use super::{Built, Command};

pub(crate) fn build(
    workspace: &Workspace,
    command: &Command,
    now: Timestamp,
) -> Result<Built, DomainError> {
    todo!()
}
