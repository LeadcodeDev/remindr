use super::{
    Block, BlockId, BlockKind, Database, DatabaseId, Emoji, Parent, Property, PropertyId,
    PropertyValue, RichText, SortKey, View, ViewId,
};

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    InsertBlock(Block),
    RemoveBlock {
        block: BlockId,
    },
    SetText {
        block: BlockId,
        text: RichText,
    },
    SetKind {
        block: BlockId,
        kind: BlockKind,
    },
    Move {
        block: BlockId,
        parent: Parent,
        sort_key: SortKey,
    },
    SetBlockAlive {
        block: BlockId,
        alive: bool,
    },
    InsertDatabase(Database),
    RemoveDatabase {
        database: DatabaseId,
    },
    SetDatabaseName {
        database: DatabaseId,
        name: String,
    },
    SetDatabaseIcon {
        database: DatabaseId,
        icon: Option<Emoji>,
    },
    SetDatabaseAlive {
        database: DatabaseId,
        alive: bool,
    },
    PutProperty {
        database: DatabaseId,
        property: Property,
        index: usize,
    },
    RemoveProperty {
        database: DatabaseId,
        property: PropertyId,
    },
    SetValue {
        row: BlockId,
        property: PropertyId,
        value: Option<PropertyValue>,
    },
    PutView(View),
    RemoveView {
        view: ViewId,
    },
}

impl Operation {
    #[must_use]
    pub fn is_block_operation(&self) -> bool {
        matches!(
            self,
            Self::InsertBlock(_)
                | Self::RemoveBlock { .. }
                | Self::SetText { .. }
                | Self::SetKind { .. }
                | Self::Move { .. }
                | Self::SetBlockAlive { .. }
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Transaction(Vec<Operation>);

impl Transaction {
    #[must_use]
    pub fn new(operations: Vec<Operation>) -> Transaction {
        Self(operations)
    }

    #[must_use]
    pub fn operations(&self) -> &[Operation] {
        &self.0
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
