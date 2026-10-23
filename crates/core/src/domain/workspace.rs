use std::collections::{BTreeSet, HashMap};

use super::{
    Block, BlockId, Database, DatabaseId, DomainError, Operation, Parent, PropertyId,
    PropertyValue, SortKey, Transaction, View, ViewId,
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
            workspace
                .children
                .entry(block.parent)
                .or_default()
                .insert((block.sort_key.clone(), block.id));
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
        todo!()
    }
}
