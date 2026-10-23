use remindr_core::application::Changes;
use remindr_core::domain::{BlockId, DatabaseId, Parent, Workspace};

#[must_use]
pub fn touches(
    workspace: &Workspace,
    changes: &Changes,
    database: DatabaseId,
    block: Option<BlockId>,
) -> bool {
    changes.databases.contains(&database)
        || !changes.views.is_empty()
        || changes.blocks.iter().any(|id| {
            Some(*id) == block
                || workspace
                    .block(*id)
                    .is_none_or(|found| found.parent == Parent::Database(database))
        })
}
