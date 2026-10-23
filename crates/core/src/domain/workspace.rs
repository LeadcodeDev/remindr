use std::collections::{BTreeSet, HashMap};
use std::iter::successors;

use super::{
    Block, BlockId, BlockKind, Database, DatabaseId, DomainError, Operation, Parent, PropertyId,
    PropertyValue, RichText, SortKey, Transaction, View, ViewId,
};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceSnapshot {
    pub blocks: Vec<Block>,
    pub databases: Vec<Database>,
    pub values: Vec<(BlockId, PropertyId, PropertyValue)>,
    pub views: Vec<View>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Workspace {
    pub(crate) blocks: HashMap<BlockId, Block>,
    pub(crate) children: HashMap<Parent, BTreeSet<(SortKey, BlockId)>>,
    pub(crate) databases: HashMap<DatabaseId, Database>,
    pub(crate) values: HashMap<(BlockId, PropertyId), PropertyValue>,
    pub(crate) views: HashMap<ViewId, View>,
}

impl Workspace {
    pub fn from_snapshot(snapshot: WorkspaceSnapshot) -> Result<Workspace, DomainError> {
        let mut workspace = Workspace::default();
        for block in snapshot.blocks {
            if workspace.blocks.contains_key(&block.id) {
                return Err(DomainError::AlreadyExists);
            }
            workspace.index_insert(block.parent, &block.sort_key, block.id);
            workspace.blocks.insert(block.id, block);
        }
        for database in snapshot.databases {
            workspace.databases.insert(database.id, database);
        }
        for (row, property, value) in snapshot.values {
            workspace.values.insert((row, property), value);
        }
        for view in snapshot.views {
            workspace.views.insert(view.id, view);
        }
        workspace.validate_blocks()?;
        workspace.validate_databases()?;
        Ok(workspace)
    }

    #[must_use]
    pub fn to_snapshot(&self) -> WorkspaceSnapshot {
        let mut blocks: Vec<Block> = self.blocks.values().cloned().collect();
        blocks.sort_by_key(|block| block.id);
        let mut databases: Vec<Database> = self.databases.values().cloned().collect();
        databases.sort_by_key(|database| database.id);
        let mut values: Vec<(BlockId, PropertyId, PropertyValue)> = self
            .values
            .iter()
            .map(|((row, property), value)| (*row, *property, value.clone()))
            .collect();
        values.sort_by_key(|(row, property, _)| (*row, *property));
        let mut views: Vec<View> = self.views.values().cloned().collect();
        views.sort_by_key(|view| view.id);
        WorkspaceSnapshot {
            blocks,
            databases,
            values,
            views,
        }
    }

    #[must_use]
    pub fn block(&self, id: BlockId) -> Option<&Block> {
        self.blocks.get(&id)
    }

    pub fn children(&self, parent: Parent) -> impl Iterator<Item = &Block> {
        self.children
            .get(&parent)
            .into_iter()
            .flatten()
            .filter_map(|(_, id)| self.blocks.get(id))
            .filter(|block| block.alive)
    }

    #[must_use]
    pub fn database(&self, id: DatabaseId) -> Option<&Database> {
        self.databases.get(&id)
    }

    pub fn databases(&self) -> impl Iterator<Item = &Database> {
        let mut alive: Vec<&Database> = self
            .databases
            .values()
            .filter(|database| database.alive)
            .collect();
        alive.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));
        alive.into_iter()
    }

    pub fn rows(&self, database: DatabaseId) -> impl Iterator<Item = &Block> {
        self.children(Parent::Database(database))
    }

    pub fn views(&self, block: BlockId) -> impl Iterator<Item = &View> {
        let mut owned: Vec<&View> = self
            .views
            .values()
            .filter(|view| view.block == block)
            .collect();
        owned.sort_by(|left, right| left.sort_key.cmp(&right.sort_key));
        owned.into_iter()
    }

    #[must_use]
    pub fn view(&self, id: ViewId) -> Option<&View> {
        self.views.get(&id)
    }

    pub fn apply(&mut self, transaction: &Transaction) -> Result<Transaction, DomainError> {
        let mut applied = Vec::with_capacity(transaction.operations().len());
        for operation in transaction.operations() {
            match self.apply_operation(operation) {
                Ok(inverse) => applied.push(inverse),
                Err(error) => {
                    self.roll_back(applied);
                    return Err(error);
                }
            }
        }
        applied.reverse();
        Ok(Transaction::new(applied))
    }

    fn apply_operation(&mut self, operation: &Operation) -> Result<Operation, DomainError> {
        if operation.is_block_operation() {
            self.apply_block_operation(operation)
        } else {
            self.apply_database_operation(operation)
        }
    }

    fn roll_back(&mut self, mut applied: Vec<Operation>) {
        while let Some(inverse) = applied.pop() {
            let restored = self.apply_operation(&inverse);
            debug_assert!(restored.is_ok());
        }
    }

    pub(crate) fn apply_block_operation(
        &mut self,
        operation: &Operation,
    ) -> Result<Operation, DomainError> {
        match operation {
            Operation::InsertBlock(block) => self.insert_block(block),
            Operation::RemoveBlock { block } => self.remove_block(*block),
            Operation::SetText { block, text } => self.set_text(*block, text),
            Operation::SetKind { block, kind } => self.set_kind(*block, kind),
            Operation::Move {
                block,
                parent,
                sort_key,
            } => self.move_block(*block, *parent, sort_key),
            Operation::SetBlockAlive { block, alive } => self.set_block_alive(*block, *alive),
            _ => Err(DomainError::InvalidParent),
        }
    }

    fn insert_block(&mut self, block: &Block) -> Result<Operation, DomainError> {
        if self.blocks.contains_key(&block.id) {
            return Err(DomainError::AlreadyExists);
        }
        self.check_placement(&block.kind, block.parent, block.alive)?;
        self.index_insert(block.parent, &block.sort_key, block.id);
        self.blocks.insert(block.id, block.clone());
        Ok(Operation::RemoveBlock { block: block.id })
    }

    fn remove_block(&mut self, id: BlockId) -> Result<Operation, DomainError> {
        if !self.blocks.contains_key(&id) {
            return Err(DomainError::UnknownBlock(id));
        }
        let has_children = self
            .children
            .get(&Parent::Block(id))
            .is_some_and(|siblings| !siblings.is_empty());
        let has_values = self.values.keys().any(|(row, _)| *row == id);
        let owns_views = self.views.values().any(|view| view.block == id);
        if has_children || has_values || owns_views {
            return Err(DomainError::StillReferenced);
        }
        let old = self
            .blocks
            .remove(&id)
            .ok_or(DomainError::UnknownBlock(id))?;
        self.index_remove(old.parent, &old.sort_key, id);
        Ok(Operation::InsertBlock(old))
    }

    fn set_text(&mut self, id: BlockId, text: &RichText) -> Result<Operation, DomainError> {
        let block = self
            .blocks
            .get_mut(&id)
            .ok_or(DomainError::UnknownBlock(id))?;
        let old = std::mem::replace(&mut block.text, text.clone());
        Ok(Operation::SetText {
            block: id,
            text: old,
        })
    }

    fn set_kind(&mut self, id: BlockId, kind: &BlockKind) -> Result<Operation, DomainError> {
        let block = self.blocks.get(&id).ok_or(DomainError::UnknownBlock(id))?;
        if !kind.accepts_children() && self.children(Parent::Block(id)).next().is_some() {
            return Err(DomainError::ChildrenNotAllowed);
        }
        self.check_kind_at(kind, block.parent)?;
        let block = self
            .blocks
            .get_mut(&id)
            .ok_or(DomainError::UnknownBlock(id))?;
        let old = std::mem::replace(&mut block.kind, kind.clone());
        Ok(Operation::SetKind {
            block: id,
            kind: old,
        })
    }

    fn move_block(
        &mut self,
        id: BlockId,
        parent: Parent,
        sort_key: &SortKey,
    ) -> Result<Operation, DomainError> {
        let block = self.blocks.get(&id).ok_or(DomainError::UnknownBlock(id))?;
        if let Parent::Block(target) = parent
            && self
                .ancestors_or_self(target)
                .take(self.blocks.len() + 1)
                .any(|ancestor| ancestor == id)
        {
            return Err(DomainError::CycleDetected);
        }
        self.check_placement(&block.kind, parent, block.alive)?;
        let old_parent = block.parent;
        let old_key = block.sort_key.clone();
        self.index_remove(old_parent, &old_key, id);
        self.index_insert(parent, sort_key, id);
        let block = self
            .blocks
            .get_mut(&id)
            .ok_or(DomainError::UnknownBlock(id))?;
        block.parent = parent;
        block.sort_key = sort_key.clone();
        Ok(Operation::Move {
            block: id,
            parent: old_parent,
            sort_key: old_key,
        })
    }

    fn set_block_alive(&mut self, id: BlockId, alive: bool) -> Result<Operation, DomainError> {
        let block = self.blocks.get(&id).ok_or(DomainError::UnknownBlock(id))?;
        if alive && !block.alive {
            self.check_parent(block.parent, true)?;
        }
        let block = self
            .blocks
            .get_mut(&id)
            .ok_or(DomainError::UnknownBlock(id))?;
        let old = std::mem::replace(&mut block.alive, alive);
        Ok(Operation::SetBlockAlive {
            block: id,
            alive: old,
        })
    }

    fn check_parent(&self, parent: Parent, must_accept: bool) -> Result<(), DomainError> {
        match parent {
            Parent::Root => Ok(()),
            Parent::Database(database) if self.databases.contains_key(&database) => Ok(()),
            Parent::Database(database) => Err(DomainError::UnknownDatabase(database)),
            Parent::Block(owner) => {
                let owner = self
                    .blocks
                    .get(&owner)
                    .ok_or(DomainError::UnknownBlock(owner))?;
                if owner.kind.accepts_children() || !must_accept {
                    Ok(())
                } else {
                    Err(DomainError::ChildrenNotAllowed)
                }
            }
        }
    }

    fn check_kind_at(&self, kind: &BlockKind, parent: Parent) -> Result<(), DomainError> {
        let needs_page = matches!(parent, Parent::Root | Parent::Database(_));
        if needs_page && !matches!(kind, BlockKind::Page { .. }) {
            return Err(DomainError::InvalidParent);
        }
        match kind {
            BlockKind::DatabaseView { database } if !self.databases.contains_key(database) => {
                Err(DomainError::UnknownDatabase(*database))
            }
            _ => Ok(()),
        }
    }

    fn check_placement(
        &self,
        kind: &BlockKind,
        parent: Parent,
        alive: bool,
    ) -> Result<(), DomainError> {
        self.check_parent(parent, alive)?;
        self.check_kind_at(kind, parent)
    }

    fn ancestors_or_self(&self, start: BlockId) -> impl Iterator<Item = BlockId> + '_ {
        successors(Some(start), |id| match self.blocks.get(id)?.parent {
            Parent::Block(parent) => Some(parent),
            Parent::Root | Parent::Database(_) => None,
        })
    }

    fn has_cycle(&self, start: BlockId) -> bool {
        self.ancestors_or_self(start)
            .skip(1)
            .take(self.blocks.len())
            .any(|ancestor| ancestor == start)
    }

    fn validate_blocks(&self) -> Result<(), DomainError> {
        let mut blocks: Vec<&Block> = self.blocks.values().collect();
        blocks.sort_by_key(|block| block.id);
        for block in blocks {
            self.check_placement(&block.kind, block.parent, block.alive)?;
            if self.has_cycle(block.id) {
                return Err(DomainError::CycleDetected);
            }
        }
        Ok(())
    }

    fn index_insert(&mut self, parent: Parent, sort_key: &SortKey, id: BlockId) {
        self.children
            .entry(parent)
            .or_default()
            .insert((sort_key.clone(), id));
    }

    fn index_remove(&mut self, parent: Parent, sort_key: &SortKey, id: BlockId) {
        if let Some(siblings) = self.children.get_mut(&parent) {
            siblings.remove(&(sort_key.clone(), id));
            if siblings.is_empty() {
                self.children.remove(&parent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use proptest::prelude::*;

    use super::{Workspace, WorkspaceSnapshot};
    use crate::domain::{
        Block, BlockId, BlockKind, Database, DatabaseId, DomainError, HeadingLevel, Operation,
        Parent, PropertyId, PropertyValue, RichText, Schema, SortKey, Timestamp, Transaction, Uuid,
        View, ViewId, ViewKind,
    };

    fn id(n: u128) -> BlockId {
        BlockId::from_uuid(Uuid::from_u128(n))
    }

    fn key(raw: &str) -> SortKey {
        SortKey::parse(raw).unwrap()
    }

    fn block(n: u128, kind: BlockKind, parent: Parent, sort_key: &str) -> Block {
        Block {
            id: id(n),
            kind,
            parent,
            sort_key: key(sort_key),
            text: RichText::default(),
            alive: true,
            created_at: Timestamp::from_millis(0),
        }
    }

    fn page() -> BlockKind {
        BlockKind::Page { icon: None }
    }

    fn database(sort_key: &str) -> Database {
        Database {
            id: DatabaseId::new(),
            name: "Tasks".to_owned(),
            icon: None,
            schema: Schema::default(),
            sort_key: key(sort_key),
            alive: true,
            created_at: Timestamp::from_millis(0),
        }
    }

    fn view_of(block: BlockId) -> View {
        View {
            id: ViewId::new(),
            block,
            name: "Table".to_owned(),
            sort_key: key("V"),
            kind: ViewKind::Table,
            filters: Vec::new(),
            sorts: Vec::new(),
            visible: Vec::new(),
            column_widths: BTreeMap::new(),
            manual_order: Vec::new(),
        }
    }

    fn snapshot(blocks: Vec<Block>) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            blocks,
            ..WorkspaceSnapshot::default()
        }
    }

    fn base() -> Workspace {
        Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Text, Parent::Block(id(1)), "V"),
            block(
                3,
                BlockKind::Heading(HeadingLevel::One),
                Parent::Block(id(1)),
                "k",
            ),
            block(4, BlockKind::Divider, Parent::Block(id(1)), "t"),
            block(5, BlockKind::Text, Parent::Block(id(2)), "V"),
        ]))
        .unwrap()
    }

    fn assert_rejected(workspace: &mut Workspace, operation: &Operation, expected: DomainError) {
        let before = workspace.clone();
        assert_eq!(workspace.apply_block_operation(operation), Err(expected));
        assert_eq!(*workspace, before);
    }

    fn child_ids(workspace: &Workspace, parent: Parent) -> Vec<BlockId> {
        workspace.children(parent).map(|block| block.id).collect()
    }

    #[test]
    fn insert_existing_id_is_already_exists() {
        let mut workspace = base();
        let again = block(2, BlockKind::Text, Parent::Block(id(1)), "x");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(again),
            DomainError::AlreadyExists,
        );
    }

    #[test]
    fn insert_rejects_a_missing_parent() {
        let mut workspace = base();
        let orphan = block(9, BlockKind::Text, Parent::Block(id(77)), "V");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(orphan),
            DomainError::UnknownBlock(id(77)),
        );
        let missing = DatabaseId::new();
        let row = block(9, page(), Parent::Database(missing), "V");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(row),
            DomainError::UnknownDatabase(missing),
        );
    }

    #[test]
    fn insert_rejects_a_non_page_under_root_or_database() {
        let mut workspace = base();
        let stray = block(9, BlockKind::Text, Parent::Root, "V");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(stray),
            DomainError::InvalidParent,
        );
        let owner = database("V");
        let owner_id = owner.id;
        workspace.databases.insert(owner_id, owner);
        let stray_row = block(9, BlockKind::Text, Parent::Database(owner_id), "V");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(stray_row),
            DomainError::InvalidParent,
        );
    }

    #[test]
    fn insert_rejects_a_child_under_a_parent_that_refuses_children() {
        let mut workspace = base();
        let under_heading = block(9, BlockKind::Text, Parent::Block(id(3)), "V");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(under_heading),
            DomainError::ChildrenNotAllowed,
        );
    }

    #[test]
    fn insert_rejects_a_database_view_of_a_missing_database() {
        let mut workspace = base();
        let missing = DatabaseId::new();
        let embed = block(
            9,
            BlockKind::DatabaseView { database: missing },
            Parent::Block(id(1)),
            "V",
        );
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(embed),
            DomainError::UnknownDatabase(missing),
        );
    }

    #[test]
    fn insert_adds_the_block_and_returns_a_remove_inverse() {
        let mut workspace = base();
        let before = workspace.clone();
        let added = block(9, BlockKind::Text, Parent::Block(id(1)), "x");
        let inverse = workspace
            .apply_block_operation(&Operation::InsertBlock(added))
            .unwrap();
        assert_eq!(inverse, Operation::RemoveBlock { block: id(9) });
        assert_eq!(
            child_ids(&workspace, Parent::Block(id(1))),
            [id(2), id(3), id(4), id(9)]
        );
        workspace.apply_block_operation(&inverse).unwrap();
        assert_eq!(workspace, before);
    }

    #[test]
    fn insert_accepts_a_page_under_a_database() {
        let mut workspace = base();
        let owner = database("V");
        let owner_id = owner.id;
        workspace.databases.insert(owner_id, owner);
        let row = block(9, page(), Parent::Database(owner_id), "V");
        workspace
            .apply_block_operation(&Operation::InsertBlock(row))
            .unwrap();
        assert_eq!(workspace.rows(owner_id).count(), 1);
    }

    #[test]
    fn remove_rejects_an_unknown_block() {
        let mut workspace = base();
        assert_rejected(
            &mut workspace,
            &Operation::RemoveBlock { block: id(77) },
            DomainError::UnknownBlock(id(77)),
        );
    }

    #[test]
    fn remove_block_with_children_is_still_referenced() {
        let mut workspace = base();
        assert_rejected(
            &mut workspace,
            &Operation::RemoveBlock { block: id(2) },
            DomainError::StillReferenced,
        );
    }

    #[test]
    fn remove_rejects_a_block_with_children_even_dead_ones() {
        let mut workspace = base();
        workspace
            .apply_block_operation(&Operation::SetBlockAlive {
                block: id(5),
                alive: false,
            })
            .unwrap();
        assert_rejected(
            &mut workspace,
            &Operation::RemoveBlock { block: id(2) },
            DomainError::StillReferenced,
        );
    }

    #[test]
    fn remove_succeeds_once_the_children_are_removed() {
        let mut workspace = base();
        workspace
            .apply_block_operation(&Operation::RemoveBlock { block: id(5) })
            .unwrap();
        workspace
            .apply_block_operation(&Operation::RemoveBlock { block: id(2) })
            .unwrap();
        assert!(workspace.block(id(2)).is_none());
    }

    #[test]
    fn remove_rejects_a_block_with_property_values() {
        let mut workspace = base();
        workspace
            .values
            .insert((id(3), PropertyId::new()), PropertyValue::Checkbox(true));
        assert_rejected(
            &mut workspace,
            &Operation::RemoveBlock { block: id(3) },
            DomainError::StillReferenced,
        );
    }

    #[test]
    fn remove_rejects_a_block_that_owns_views() {
        let mut workspace = base();
        let view = view_of(id(4));
        workspace.views.insert(view.id, view);
        assert_rejected(
            &mut workspace,
            &Operation::RemoveBlock { block: id(4) },
            DomainError::StillReferenced,
        );
    }

    #[test]
    fn remove_deletes_the_block_and_prunes_the_child_index() {
        let mut workspace = base();
        let before = workspace.clone();
        let inverse = workspace
            .apply_block_operation(&Operation::RemoveBlock { block: id(5) })
            .unwrap();
        assert!(workspace.block(id(5)).is_none());
        assert!(!workspace.children.contains_key(&Parent::Block(id(2))));
        assert!(matches!(inverse, Operation::InsertBlock(ref old) if old.id == id(5)));
        workspace.apply_block_operation(&inverse).unwrap();
        assert_eq!(workspace, before);
    }

    #[test]
    fn set_text_rejects_an_unknown_block() {
        let mut workspace = base();
        let operation = Operation::SetText {
            block: id(77),
            text: RichText::plain("x"),
        };
        assert_rejected(
            &mut workspace,
            &operation,
            DomainError::UnknownBlock(id(77)),
        );
    }

    #[test]
    fn set_text_returns_the_old_text_as_inverse() {
        let mut workspace = base();
        let inverse = workspace
            .apply_block_operation(&Operation::SetText {
                block: id(2),
                text: RichText::plain("new"),
            })
            .unwrap();
        assert_eq!(workspace.block(id(2)).unwrap().text, RichText::plain("new"));
        assert_eq!(
            inverse,
            Operation::SetText {
                block: id(2),
                text: RichText::default()
            }
        );
    }

    #[test]
    fn set_kind_rejects_an_unknown_block() {
        let mut workspace = base();
        let operation = Operation::SetKind {
            block: id(77),
            kind: BlockKind::Text,
        };
        assert_rejected(
            &mut workspace,
            &operation,
            DomainError::UnknownBlock(id(77)),
        );
    }

    #[test]
    fn set_kind_rejects_a_kind_that_refuses_children_when_alive_children_exist() {
        let mut workspace = base();
        let operation = Operation::SetKind {
            block: id(2),
            kind: BlockKind::Heading(HeadingLevel::Two),
        };
        assert_rejected(&mut workspace, &operation, DomainError::ChildrenNotAllowed);
    }

    #[test]
    fn set_kind_allows_a_kind_that_refuses_children_when_only_dead_children_exist() {
        let mut workspace = base();
        workspace
            .apply_block_operation(&Operation::SetBlockAlive {
                block: id(5),
                alive: false,
            })
            .unwrap();
        workspace
            .apply_block_operation(&Operation::SetKind {
                block: id(2),
                kind: BlockKind::Divider,
            })
            .unwrap();
        assert_eq!(workspace.block(id(2)).unwrap().kind, BlockKind::Divider);
    }

    #[test]
    fn set_kind_rejects_a_non_page_kind_under_root_or_database() {
        let mut workspace = base();
        let operation = Operation::SetKind {
            block: id(1),
            kind: BlockKind::Text,
        };
        assert_rejected(&mut workspace, &operation, DomainError::InvalidParent);
        let owner = database("V");
        let owner_id = owner.id;
        workspace.databases.insert(owner_id, owner);
        workspace
            .apply_block_operation(&Operation::Move {
                block: id(1),
                parent: Parent::Database(owner_id),
                sort_key: key("V"),
            })
            .unwrap();
        assert_rejected(&mut workspace, &operation, DomainError::InvalidParent);
    }

    #[test]
    fn set_kind_rejects_a_database_view_of_a_missing_database() {
        let mut workspace = base();
        let missing = DatabaseId::new();
        let operation = Operation::SetKind {
            block: id(3),
            kind: BlockKind::DatabaseView { database: missing },
        };
        assert_rejected(
            &mut workspace,
            &operation,
            DomainError::UnknownDatabase(missing),
        );
    }

    #[test]
    fn set_kind_returns_the_old_kind_as_inverse() {
        let mut workspace = base();
        let inverse = workspace
            .apply_block_operation(&Operation::SetKind {
                block: id(2),
                kind: BlockKind::ToDo { checked: true },
            })
            .unwrap();
        assert_eq!(
            inverse,
            Operation::SetKind {
                block: id(2),
                kind: BlockKind::Text
            }
        );
    }

    #[test]
    fn move_rejects_an_unknown_block() {
        let mut workspace = base();
        let operation = Operation::Move {
            block: id(77),
            parent: Parent::Root,
            sort_key: key("V"),
        };
        assert_rejected(
            &mut workspace,
            &operation,
            DomainError::UnknownBlock(id(77)),
        );
    }

    #[test]
    fn move_rejects_invalid_parents() {
        let mut workspace = base();
        let to_root = Operation::Move {
            block: id(2),
            parent: Parent::Root,
            sort_key: key("V"),
        };
        assert_rejected(&mut workspace, &to_root, DomainError::InvalidParent);
        let to_missing = Operation::Move {
            block: id(2),
            parent: Parent::Block(id(77)),
            sort_key: key("V"),
        };
        assert_rejected(
            &mut workspace,
            &to_missing,
            DomainError::UnknownBlock(id(77)),
        );
        let to_heading = Operation::Move {
            block: id(2),
            parent: Parent::Block(id(3)),
            sort_key: key("V"),
        };
        assert_rejected(&mut workspace, &to_heading, DomainError::ChildrenNotAllowed);
        let missing = DatabaseId::new();
        let to_database = Operation::Move {
            block: id(1),
            parent: Parent::Database(missing),
            sort_key: key("V"),
        };
        assert_rejected(
            &mut workspace,
            &to_database,
            DomainError::UnknownDatabase(missing),
        );
    }

    #[test]
    fn move_rejects_a_cycle() {
        let mut workspace = base();
        let under_itself = Operation::Move {
            block: id(2),
            parent: Parent::Block(id(2)),
            sort_key: key("V"),
        };
        assert_rejected(&mut workspace, &under_itself, DomainError::CycleDetected);
        let under_child = Operation::Move {
            block: id(1),
            parent: Parent::Block(id(5)),
            sort_key: key("V"),
        };
        assert_rejected(&mut workspace, &under_child, DomainError::CycleDetected);
    }

    #[test]
    fn move_updates_the_child_index_and_returns_the_old_position() {
        let mut workspace = base();
        let before = workspace.clone();
        let inverse = workspace
            .apply_block_operation(&Operation::Move {
                block: id(5),
                parent: Parent::Block(id(1)),
                sort_key: key("m"),
            })
            .unwrap();
        assert_eq!(
            child_ids(&workspace, Parent::Block(id(1))),
            [id(2), id(3), id(5), id(4)]
        );
        assert!(!workspace.children.contains_key(&Parent::Block(id(2))));
        assert_eq!(
            inverse,
            Operation::Move {
                block: id(5),
                parent: Parent::Block(id(2)),
                sort_key: key("V")
            }
        );
        workspace.apply_block_operation(&inverse).unwrap();
        assert_eq!(workspace, before);
    }

    #[test]
    fn set_block_alive_rejects_an_unknown_block() {
        let mut workspace = base();
        let operation = Operation::SetBlockAlive {
            block: id(77),
            alive: false,
        };
        assert_rejected(
            &mut workspace,
            &operation,
            DomainError::UnknownBlock(id(77)),
        );
    }

    #[test]
    fn set_block_alive_returns_the_old_flag_as_inverse() {
        let mut workspace = base();
        let inverse = workspace
            .apply_block_operation(&Operation::SetBlockAlive {
                block: id(3),
                alive: false,
            })
            .unwrap();
        assert!(!workspace.block(id(3)).unwrap().alive);
        assert_eq!(
            inverse,
            Operation::SetBlockAlive {
                block: id(3),
                alive: true
            }
        );
    }

    #[test]
    fn children_are_in_sort_key_order_and_alive_only() {
        let mut workspace = base();
        workspace
            .apply_block_operation(&Operation::SetBlockAlive {
                block: id(3),
                alive: false,
            })
            .unwrap();
        assert_eq!(child_ids(&workspace, Parent::Block(id(1))), [id(2), id(4)]);
        let last = block(9, BlockKind::Text, Parent::Block(id(1)), "A");
        workspace
            .apply_block_operation(&Operation::InsertBlock(last))
            .unwrap();
        assert_eq!(
            child_ids(&workspace, Parent::Block(id(1))),
            [id(9), id(2), id(4)]
        );
    }

    #[test]
    fn failed_operation_leaves_workspace_unchanged() {
        let mut workspace = base();
        let before = workspace.clone();
        let transaction = Transaction::new(vec![
            Operation::InsertBlock(block(9, BlockKind::Text, Parent::Block(id(1)), "x")),
            Operation::Move {
                block: id(1),
                parent: Parent::Block(id(5)),
                sort_key: key("V"),
            },
        ]);
        assert_eq!(
            workspace.apply(&transaction),
            Err(DomainError::CycleDetected)
        );
        assert_eq!(workspace, before);
    }

    #[test]
    fn apply_returns_inverses_in_reverse_order() {
        let mut workspace = base();
        let before = workspace.clone();
        let transaction = Transaction::new(vec![
            Operation::InsertBlock(block(9, BlockKind::Text, Parent::Block(id(1)), "x")),
            Operation::SetText {
                block: id(9),
                text: RichText::plain("hi"),
            },
        ]);
        let inverse = workspace.apply(&transaction).unwrap();
        workspace.apply(&inverse).unwrap();
        assert_eq!(workspace, before);
    }

    #[test]
    fn snapshot_with_cycle_is_rejected() {
        let result = Workspace::from_snapshot(snapshot(vec![
            block(1, BlockKind::Text, Parent::Block(id(2)), "V"),
            block(2, BlockKind::Text, Parent::Block(id(1)), "V"),
        ]));
        assert_eq!(result, Err(DomainError::CycleDetected));
        let self_parent = Workspace::from_snapshot(snapshot(vec![block(
            1,
            BlockKind::Text,
            Parent::Block(id(1)),
            "V",
        )]));
        assert_eq!(self_parent, Err(DomainError::CycleDetected));
    }

    #[test]
    fn snapshot_with_a_cycle_hanging_below_a_valid_chain_is_rejected() {
        let result = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Text, Parent::Block(id(3)), "V"),
            block(3, BlockKind::Text, Parent::Block(id(2)), "V"),
            block(4, BlockKind::Text, Parent::Block(id(3)), "k"),
        ]));
        assert_eq!(result, Err(DomainError::CycleDetected));
    }

    #[test]
    fn snapshot_with_a_missing_parent_is_rejected() {
        let result = Workspace::from_snapshot(snapshot(vec![block(
            1,
            BlockKind::Text,
            Parent::Block(id(9)),
            "V",
        )]));
        assert_eq!(result, Err(DomainError::UnknownBlock(id(9))));
        let missing = DatabaseId::new();
        let row = Workspace::from_snapshot(snapshot(vec![block(
            1,
            page(),
            Parent::Database(missing),
            "V",
        )]));
        assert_eq!(row, Err(DomainError::UnknownDatabase(missing)));
    }

    #[test]
    fn snapshot_with_a_parent_rule_violation_is_rejected() {
        let stray =
            Workspace::from_snapshot(snapshot(vec![block(1, BlockKind::Text, Parent::Root, "V")]));
        assert_eq!(stray, Err(DomainError::InvalidParent));
        let heading_at_root = Workspace::from_snapshot(snapshot(vec![block(
            1,
            BlockKind::Heading(HeadingLevel::One),
            Parent::Root,
            "V",
        )]));
        assert_eq!(heading_at_root, Err(DomainError::InvalidParent));
        let child_of_divider = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Divider, Parent::Block(id(1)), "V"),
            block(3, BlockKind::Text, Parent::Block(id(2)), "V"),
        ]));
        assert_eq!(child_of_divider, Err(DomainError::ChildrenNotAllowed));
    }

    #[test]
    fn snapshot_accepts_a_dead_child_under_a_parent_that_refuses_children() {
        let mut dead = block(3, BlockKind::Text, Parent::Block(id(2)), "V");
        dead.alive = false;
        let result = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Divider, Parent::Block(id(1)), "V"),
            dead,
        ]));
        assert!(result.is_ok());
    }

    #[test]
    fn a_dead_child_can_be_inserted_or_moved_under_a_parent_that_refuses_children() {
        let mut dead = block(3, BlockKind::Text, Parent::Block(id(2)), "V");
        dead.alive = false;
        let mut workspace = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Divider, Parent::Block(id(1)), "V"),
            block(4, BlockKind::Text, Parent::Block(id(1)), "k"),
            dead.clone(),
        ]))
        .unwrap();
        let before = workspace.clone();
        let out = workspace
            .apply_block_operation(&Operation::Move {
                block: id(3),
                parent: Parent::Block(id(4)),
                sort_key: key("V"),
            })
            .unwrap();
        workspace.apply_block_operation(&out).unwrap();
        assert_eq!(workspace, before);
        let removed = workspace
            .apply_block_operation(&Operation::RemoveBlock { block: id(3) })
            .unwrap();
        workspace.apply_block_operation(&removed).unwrap();
        assert_eq!(workspace, before);
        let alive_child = block(5, BlockKind::Text, Parent::Block(id(2)), "k");
        assert_rejected(
            &mut workspace,
            &Operation::InsertBlock(alive_child),
            DomainError::ChildrenNotAllowed,
        );
    }

    #[test]
    fn snapshot_with_duplicate_block_ids_is_rejected() {
        let result = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(1, page(), Parent::Root, "k"),
        ]));
        assert_eq!(result, Err(DomainError::AlreadyExists));
    }

    #[test]
    fn reviving_a_block_under_a_parent_that_refuses_children_is_rejected() {
        let mut dead = block(3, BlockKind::Text, Parent::Block(id(2)), "V");
        dead.alive = false;
        let mut workspace = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Divider, Parent::Block(id(1)), "V"),
            dead,
        ]))
        .unwrap();
        let revive = Operation::SetBlockAlive {
            block: id(3),
            alive: true,
        };
        assert_rejected(&mut workspace, &revive, DomainError::ChildrenNotAllowed);
    }

    #[test]
    fn snapshot_with_a_database_view_of_a_missing_database_is_rejected() {
        let missing = DatabaseId::new();
        let result = Workspace::from_snapshot(snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(
                2,
                BlockKind::DatabaseView { database: missing },
                Parent::Block(id(1)),
                "V",
            ),
        ]));
        assert_eq!(result, Err(DomainError::UnknownDatabase(missing)));
    }

    #[test]
    fn snapshot_roundtrip_keeps_dead_blocks_and_order() {
        let mut dead = block(3, BlockKind::Text, Parent::Block(id(1)), "k");
        dead.alive = false;
        let snapshot = snapshot(vec![
            block(1, page(), Parent::Root, "V"),
            block(2, BlockKind::Text, Parent::Block(id(1)), "V"),
            dead,
        ]);
        let workspace = Workspace::from_snapshot(snapshot.clone()).unwrap();
        let mut expected = snapshot;
        expected.blocks.sort_by_key(|block| block.id);
        assert_eq!(workspace.to_snapshot(), expected);
        assert_eq!(child_ids(&workspace, Parent::Block(id(1))), [id(2)]);
    }

    const POOL: u128 = 5;

    fn pool_id() -> impl Strategy<Value = BlockId> {
        (1..=POOL).prop_map(id)
    }

    fn any_kind() -> impl Strategy<Value = BlockKind> {
        prop_oneof![
            Just(BlockKind::Text),
            Just(BlockKind::Heading(HeadingLevel::One)),
            Just(BlockKind::Divider),
            Just(BlockKind::Toggle),
            Just(page()),
        ]
    }

    fn any_parent() -> impl Strategy<Value = Parent> {
        prop_oneof![Just(Parent::Root), pool_id().prop_map(Parent::Block)]
    }

    fn any_key() -> impl Strategy<Value = SortKey> {
        prop_oneof![Just(key("V")), Just(key("G")), Just(key("k"))]
    }

    fn any_operation() -> impl Strategy<Value = Operation> {
        prop_oneof![
            (pool_id(), any_kind(), any_parent(), any_key()).prop_map(|(id, kind, parent, key)| {
                Operation::InsertBlock(Block {
                    id,
                    kind,
                    parent,
                    sort_key: key,
                    text: RichText::default(),
                    alive: true,
                    created_at: Timestamp::from_millis(0),
                })
            }),
            pool_id().prop_map(|block| Operation::RemoveBlock { block }),
            (pool_id(), "[a-z]{0,3}").prop_map(|(block, text)| Operation::SetText {
                block,
                text: RichText::plain(&text)
            }),
            (pool_id(), any_kind()).prop_map(|(block, kind)| Operation::SetKind { block, kind }),
            (pool_id(), any_parent(), any_key()).prop_map(|(block, parent, sort_key)| {
                Operation::Move {
                    block,
                    parent,
                    sort_key,
                }
            }),
            (pool_id(), any::<bool>())
                .prop_map(|(block, alive)| Operation::SetBlockAlive { block, alive }),
        ]
    }

    proptest! {
        #[test]
        fn apply_then_inverse_restores_workspace(
            operations in proptest::collection::vec(any_operation(), 0..40)
        ) {
            let mut workspace = Workspace::default();
            let mut inverses = Vec::new();
            for operation in &operations {
                let before = workspace.clone();
                match workspace.apply_block_operation(operation) {
                    Ok(inverse) => {
                        let mut replayed = workspace.clone();
                        prop_assert!(replayed.apply_block_operation(&inverse).is_ok());
                        prop_assert_eq!(&replayed, &before);
                        inverses.push(inverse);
                    }
                    Err(_) => prop_assert_eq!(&workspace, &before),
                }
            }
            while let Some(inverse) = inverses.pop() {
                prop_assert!(workspace.apply_block_operation(&inverse).is_ok());
            }
            prop_assert_eq!(workspace, Workspace::default());
        }

        #[test]
        fn applied_workspaces_round_trip_through_a_snapshot(
            operations in proptest::collection::vec(any_operation(), 0..40)
        ) {
            let mut workspace = Workspace::default();
            for operation in &operations {
                let _ = workspace.apply_block_operation(operation);
            }
            let rebuilt = Workspace::from_snapshot(workspace.to_snapshot());
            prop_assert_eq!(rebuilt, Ok(workspace));
        }
    }
}
