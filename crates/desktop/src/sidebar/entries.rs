use std::collections::HashSet;

use gpui_kit::{ElementId, SharedString};
use remindr_core::domain::{BlockId, DatabaseId, Uuid, Workspace};

use crate::app::Route;
use crate::app::navigation::{
    alive_page, child_pages, database_name, page_icon, page_title, root_pages,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Page(BlockId),
    Database(DatabaseId),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub target: Target,
    pub label: SharedString,
    pub emoji: Option<SharedString>,
    pub active: bool,
    pub expanded: bool,
    pub children: Vec<Entry>,
}

impl Entry {
    #[must_use]
    pub fn key(&self) -> u64 {
        match self.target {
            Target::Page(page) => key(page.as_uuid()),
            Target::Database(database) => key(database.as_uuid()),
        }
    }
}

fn key(uuid: Uuid) -> u64 {
    uuid.as_u64_pair().1
}

#[must_use]
pub fn page_open_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-open".into(), key(page.as_uuid()))
}

#[must_use]
pub fn page_toggle_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-toggle".into(), key(page.as_uuid()))
}

#[must_use]
pub fn page_add_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-add".into(), key(page.as_uuid()))
}

#[must_use]
pub fn page_menu_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-menu".into(), key(page.as_uuid()))
}

#[must_use]
pub fn page_icon_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-icon".into(), key(page.as_uuid()))
}

#[must_use]
pub fn page_entry_id(page: BlockId) -> ElementId {
    ElementId::NamedInteger("sidebar-page-entry".into(), key(page.as_uuid()))
}

#[must_use]
pub fn database_open_id(database: DatabaseId) -> ElementId {
    ElementId::NamedInteger("sidebar-database-open".into(), key(database.as_uuid()))
}

#[must_use]
pub fn database_menu_id(database: DatabaseId) -> ElementId {
    ElementId::NamedInteger("sidebar-database-menu".into(), key(database.as_uuid()))
}

#[must_use]
pub fn database_entry_id(database: DatabaseId) -> ElementId {
    ElementId::NamedInteger("sidebar-database-entry".into(), key(database.as_uuid()))
}

#[must_use]
pub fn page_entries(
    workspace: &Workspace,
    route: Option<&Route>,
    expanded: &HashSet<BlockId>,
) -> Vec<Entry> {
    root_pages(workspace)
        .into_iter()
        .filter_map(|page| page_entry(workspace, page, route, expanded))
        .collect()
}

fn page_entry(
    workspace: &Workspace,
    page: BlockId,
    route: Option<&Route>,
    expanded: &HashSet<BlockId>,
) -> Option<Entry> {
    let block = alive_page(workspace, page)?;
    let children: Vec<Entry> = child_pages(workspace, page)
        .into_iter()
        .filter_map(|child| page_entry(workspace, child, route, expanded))
        .collect();
    Some(Entry {
        target: Target::Page(page),
        label: page_title(block).into(),
        emoji: page_icon(block).map(|emoji| emoji.as_str().to_owned().into()),
        active: route == Some(&Route::Page(page)),
        expanded: !children.is_empty() && expanded.contains(&page),
        children,
    })
}

#[must_use]
pub fn database_entries(workspace: &Workspace, route: Option<&Route>) -> Vec<Entry> {
    workspace
        .databases()
        .map(|database| Entry {
            target: Target::Database(database.id),
            label: database_name(database).into(),
            emoji: database
                .icon
                .as_ref()
                .map(|emoji| emoji.as_str().to_owned().into()),
            active: route == Some(&Route::Database(database.id)),
            expanded: false,
            children: Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use remindr_core::application::Command;
    use remindr_core::domain::{BlockKind, Emoji, Parent};

    use super::{Target, database_entries, page_entries};
    use crate::app::Route;
    use crate::app::commands::created_database;
    use crate::app::testing::{add_block, add_page, session};

    #[test]
    fn pages_are_listed_in_order_with_nested_children() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let b = add_page(&mut session, Parent::Block(a), "B");
        let c = add_page(&mut session, Parent::Block(b), "C");
        let d = add_page(&mut session, Parent::Root, "D");
        let expanded = HashSet::from([a]);
        let entries = page_entries(session.workspace(), None, &expanded);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].target, Target::Page(a));
        assert!(entries[0].expanded);
        assert_eq!(entries[0].children[0].target, Target::Page(b));
        assert!(!entries[0].children[0].expanded);
        assert_eq!(entries[0].children[0].children[0].target, Target::Page(c));
        assert_eq!(entries[1].target, Target::Page(d));
        assert!(!entries[1].expanded);
    }

    #[test]
    fn untitled_pages_get_a_placeholder_and_icons_become_emoji() {
        let mut session = session();
        let page = add_page(&mut session, Parent::Root, "");
        session
            .execute(Command::SetPageIcon {
                page,
                icon: Some(Emoji::try_from("📄").unwrap()),
            })
            .unwrap();
        let entries = page_entries(session.workspace(), None, &HashSet::new());
        assert_eq!(entries[0].label.as_ref(), "Untitled");
        assert_eq!(entries[0].emoji.as_deref(), Some("📄"));
    }

    #[test]
    fn deleted_pages_leave_the_entries() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let b = add_page(&mut session, Parent::Block(a), "B");
        session
            .execute(Command::DeleteBlocks { blocks: vec![b] })
            .unwrap();
        let entries = page_entries(session.workspace(), None, &HashSet::from([a]));
        assert!(entries[0].children.is_empty());
        assert!(!entries[0].expanded);
        session
            .execute(Command::DeleteBlocks { blocks: vec![a] })
            .unwrap();
        assert!(page_entries(session.workspace(), None, &HashSet::new()).is_empty());
    }

    #[test]
    fn pages_nested_in_other_blocks_are_children_of_the_page() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let toggle = add_block(&mut session, a, BlockKind::Toggle, "t");
        let nested = add_page(&mut session, Parent::Block(toggle), "Nested");
        let entries = page_entries(session.workspace(), None, &HashSet::new());
        assert_eq!(entries[0].children[0].target, Target::Page(nested));
    }

    #[test]
    fn the_route_marks_the_active_entry() {
        let mut session = session();
        let a = add_page(&mut session, Parent::Root, "A");
        let b = add_page(&mut session, Parent::Root, "B");
        let entries = page_entries(session.workspace(), Some(&Route::Page(b)), &HashSet::new());
        assert!(!entries[0].active);
        assert!(entries[1].active);
        assert_eq!(entries[0].target, Target::Page(a));
    }

    #[test]
    fn databases_are_listed_apart_from_pages() {
        let mut session = session();
        let page = add_page(&mut session, Parent::Root, "A");
        let changes = session.execute(Command::CreateDatabase).unwrap();
        let database = created_database(&changes).unwrap();
        let pages = page_entries(session.workspace(), None, &HashSet::new());
        let databases = database_entries(session.workspace(), Some(&Route::Database(database)));
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].target, Target::Page(page));
        assert_eq!(databases.len(), 1);
        assert_eq!(databases[0].target, Target::Database(database));
        assert_eq!(databases[0].label.as_ref(), "Untitled");
        assert!(databases[0].active);
    }
}
