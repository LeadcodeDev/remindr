use remindr_core::application::Changes;
use remindr_core::domain::{Block, BlockId, BlockKind, Database, Emoji, Parent, Workspace};

use super::navigator::Route;

pub const UNTITLED: &str = "Untitled";

#[must_use]
pub fn page_title(block: &Block) -> String {
    let title = block.text.to_plain();
    if title.trim().is_empty() {
        UNTITLED.to_owned()
    } else {
        title
    }
}

#[must_use]
pub fn database_name(database: &Database) -> String {
    if database.name.trim().is_empty() {
        UNTITLED.to_owned()
    } else {
        database.name.clone()
    }
}

#[must_use]
pub fn page_icon(block: &Block) -> Option<&Emoji> {
    match &block.kind {
        BlockKind::Page { icon } => icon.as_ref(),
        _ => None,
    }
}

#[must_use]
pub fn alive_page(workspace: &Workspace, id: BlockId) -> Option<&Block> {
    workspace
        .block(id)
        .filter(|block| block.alive && matches!(block.kind, BlockKind::Page { .. }))
}

#[must_use]
pub fn route_alive(workspace: &Workspace, route: &Route) -> bool {
    match route {
        Route::Page(id) => alive_page(workspace, *id).is_some(),
        Route::Database(id) => workspace
            .database(*id)
            .is_some_and(|database| database.alive),
    }
}

#[must_use]
pub fn root_pages(workspace: &Workspace) -> Vec<BlockId> {
    workspace
        .children(Parent::Root)
        .filter(|block| matches!(block.kind, BlockKind::Page { .. }))
        .map(|block| block.id)
        .collect()
}

#[must_use]
pub fn first_root_page(workspace: &Workspace) -> Option<BlockId> {
    workspace
        .children(Parent::Root)
        .find(|block| matches!(block.kind, BlockKind::Page { .. }))
        .map(|block| block.id)
}

#[must_use]
pub fn last_root_page(workspace: &Workspace) -> Option<BlockId> {
    workspace
        .children(Parent::Root)
        .filter(|block| matches!(block.kind, BlockKind::Page { .. }))
        .last()
        .map(|block| block.id)
}

#[must_use]
pub fn last_child(workspace: &Workspace, parent: BlockId) -> Option<BlockId> {
    workspace
        .children(Parent::Block(parent))
        .last()
        .map(|block| block.id)
}

#[must_use]
pub fn child_pages(workspace: &Workspace, parent: BlockId) -> Vec<BlockId> {
    let mut found = Vec::new();
    collect_pages(workspace, Parent::Block(parent), &mut found);
    found
}

fn collect_pages(workspace: &Workspace, parent: Parent, found: &mut Vec<BlockId>) {
    for child in workspace.children(parent) {
        if matches!(child.kind, BlockKind::Page { .. }) {
            found.push(child.id);
        } else {
            collect_pages(workspace, Parent::Block(child.id), found);
        }
    }
}

#[must_use]
pub fn page_ancestors(workspace: &Workspace, page: BlockId) -> Vec<BlockId> {
    let mut found = Vec::new();
    let mut current = workspace.block(page).map(|block| block.parent);
    while let Some(Parent::Block(parent)) = current {
        let Some(block) = workspace.block(parent) else {
            break;
        };
        if matches!(block.kind, BlockKind::Page { .. }) {
            found.push(block.id);
        }
        current = Some(block.parent);
    }
    found
}

#[must_use]
pub fn trail(workspace: &Workspace, route: &Route) -> Vec<String> {
    match route {
        Route::Database(id) => workspace
            .database(*id)
            .map(database_name)
            .into_iter()
            .collect(),
        Route::Page(id) => page_trail(workspace, *id),
    }
}

fn page_trail(workspace: &Workspace, page: BlockId) -> Vec<String> {
    let mut titles = Vec::new();
    let mut current = workspace.block(page);
    while let Some(block) = current {
        if matches!(block.kind, BlockKind::Page { .. }) {
            titles.push(page_title(block));
        }
        current = match block.parent {
            Parent::Block(parent) => workspace.block(parent),
            Parent::Database(database) => {
                if let Some(database) = workspace.database(database) {
                    titles.push(database_name(database));
                }
                None
            }
            Parent::Root => None,
        };
    }
    titles.reverse();
    titles
}

#[must_use]
pub fn affects_navigation(workspace: &Workspace, changes: &Changes) -> bool {
    !changes.databases.is_empty()
        || changes.blocks.iter().any(|id| {
            workspace.block(*id).is_none_or(|block| {
                matches!(block.kind, BlockKind::Page { .. })
                    || workspace.children(Parent::Block(block.id)).next().is_some()
            })
        })
}

#[cfg(test)]
mod tests {
    use remindr_core::application::{Command, Session};
    use remindr_core::domain::{BlockId, BlockKind, Parent, RichText};

    use super::{
        affects_navigation, child_pages, first_root_page, last_root_page, page_ancestors, trail,
    };
    use crate::app::testing::{add_block, add_page, session};
    use crate::app::{AppStore, Route, SystemClock};

    fn setup() -> (Session<AppStore, SystemClock>, BlockId, BlockId, BlockId) {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let b = add_page(&mut session, Parent::Block(a), "B");
        let c = add_page(&mut session, Parent::Root, "C");
        (session, a, b, c)
    }

    #[test]
    fn trail_lists_ancestor_titles_root_first() {
        let (session, a, b, _) = setup();
        assert_eq!(
            trail(session.workspace(), &Route::Page(b)),
            vec!["A".to_owned(), "B".to_owned()]
        );
        assert_eq!(trail(session.workspace(), &Route::Page(a)), vec!["A"]);
    }

    #[test]
    fn trail_names_untitled_pages() {
        let mut session = session();
        let page = add_page(&mut session, Parent::Root, "");
        assert_eq!(
            trail(session.workspace(), &Route::Page(page)),
            vec!["Untitled"]
        );
    }

    #[test]
    fn trail_of_a_database_is_its_name() {
        let mut session = session();
        let changes = session.execute(Command::CreateDatabase).unwrap();
        let database = crate::app::commands::created_database(&changes).unwrap();
        assert_eq!(
            trail(session.workspace(), &Route::Database(database)),
            vec!["Untitled"]
        );
    }

    #[test]
    fn trail_of_a_row_starts_with_its_database() {
        let mut session = session();
        let changes = session.execute(Command::CreateDatabase).unwrap();
        let database = crate::app::commands::created_database(&changes).unwrap();
        session
            .execute(Command::RenameDatabase {
                database,
                name: "Tasks".to_owned(),
            })
            .unwrap();
        let changes = session
            .execute(Command::AddRow {
                database,
                values: Vec::new(),
            })
            .unwrap();
        let row = crate::app::commands::created_block(&changes).unwrap();
        session
            .execute(Command::EditText {
                block: row,
                text: RichText::plain("Write docs"),
            })
            .unwrap();
        assert_eq!(
            trail(session.workspace(), &Route::Page(row)),
            vec!["Tasks".to_owned(), "Write docs".to_owned()]
        );
    }

    #[test]
    fn child_pages_see_through_nested_blocks() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let toggle = add_block(&mut session, a, BlockKind::Toggle, "toggle");
        let nested = add_page(&mut session, Parent::Block(toggle), "Nested");
        let direct = add_page(&mut session, Parent::Block(a), "Direct");
        assert_eq!(child_pages(session.workspace(), a), vec![nested, direct]);
        assert_eq!(page_ancestors(session.workspace(), nested), vec![a]);
    }

    #[test]
    fn root_page_helpers_follow_sort_order() {
        let (session, a, _, c) = setup();
        assert_eq!(first_root_page(session.workspace()), Some(a));
        assert_eq!(last_root_page(session.workspace()), Some(c));
    }

    #[test]
    fn only_structural_changes_affect_navigation() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let text = add_block(&mut session, a, BlockKind::Text, "hello");
        let edit = session
            .execute(Command::EditText {
                block: text,
                text: RichText::plain("hello!"),
            })
            .unwrap();
        assert!(!affects_navigation(session.workspace(), &edit));
        let title = session
            .execute(Command::EditText {
                block: a,
                text: RichText::plain("Renamed"),
            })
            .unwrap();
        assert!(affects_navigation(session.workspace(), &title));
        let database = session.execute(Command::CreateDatabase).unwrap();
        assert!(affects_navigation(session.workspace(), &database));
    }

    #[test]
    fn undoing_a_creation_affects_navigation() {
        let mut session = session();
        add_page(&mut session, Parent::Root, "A");
        let created = session
            .execute(Command::CreatePage {
                parent: Parent::Root,
                after: None,
            })
            .unwrap();
        assert!(affects_navigation(session.workspace(), &created));
        let undone = session.undo().unwrap().unwrap();
        assert!(affects_navigation(session.workspace(), &undone));
    }
}
