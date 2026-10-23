use std::collections::BTreeSet;

use crate::domain::{
    BlockId, BlockKind, DatabaseId, DomainError, Emoji, Filter, MarkChange, OptionColor, OptionId,
    Parent, PropertyId, PropertyKind, PropertyValue, RichText, Sort, Timestamp, Transaction,
    ViewId, ViewKind, Workspace,
};

use super::{blocks, databases};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TextPoint {
    pub block: BlockId,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DropTarget {
    pub parent: Parent,
    pub after: Option<BlockId>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    CreatePage {
        parent: Parent,
        after: Option<BlockId>,
    },
    InsertBlock {
        after: BlockId,
        kind: BlockKind,
    },
    InsertBlockAt {
        target: DropTarget,
        kind: BlockKind,
    },
    EditText {
        block: BlockId,
        text: RichText,
    },
    SplitBlock {
        at: TextPoint,
    },
    MergeWithPrevious {
        block: BlockId,
    },
    DeleteRange {
        start: TextPoint,
        end: TextPoint,
    },
    SetMarks {
        start: TextPoint,
        end: TextPoint,
        change: MarkChange,
    },
    Indent {
        block: BlockId,
    },
    Outdent {
        block: BlockId,
    },
    ChangeKind {
        block: BlockId,
        kind: BlockKind,
    },
    MoveBlock {
        block: BlockId,
        target: DropTarget,
    },
    DeleteBlocks {
        blocks: Vec<BlockId>,
    },
    SetPageIcon {
        page: BlockId,
        icon: Option<Emoji>,
    },
    CreateDatabase,
    CreateDatabaseWithView {
        after: BlockId,
    },
    InsertDatabaseView {
        after: BlockId,
        database: DatabaseId,
    },
    RenameDatabase {
        database: DatabaseId,
        name: String,
    },
    SetDatabaseIcon {
        database: DatabaseId,
        icon: Option<Emoji>,
    },
    DeleteDatabase {
        database: DatabaseId,
    },
    AddProperty {
        database: DatabaseId,
        name: String,
        kind: PropertyKind,
    },
    RenameProperty {
        database: DatabaseId,
        property: PropertyId,
        name: String,
    },
    ChangePropertyKind {
        database: DatabaseId,
        property: PropertyId,
        kind: PropertyKind,
    },
    RemoveProperty {
        database: DatabaseId,
        property: PropertyId,
    },
    MoveProperty {
        database: DatabaseId,
        property: PropertyId,
        index: usize,
    },
    AddSelectOption {
        database: DatabaseId,
        property: PropertyId,
        name: String,
        color: OptionColor,
    },
    RenameSelectOption {
        database: DatabaseId,
        property: PropertyId,
        option: OptionId,
        name: String,
    },
    RemoveSelectOption {
        database: DatabaseId,
        property: PropertyId,
        option: OptionId,
    },
    AddRow {
        database: DatabaseId,
        values: Vec<(PropertyId, PropertyValue)>,
    },
    SetValue {
        row: BlockId,
        property: PropertyId,
        value: Option<PropertyValue>,
    },
    AddView {
        block: BlockId,
        name: String,
        kind: ViewKind,
    },
    RenameView {
        view: ViewId,
        name: String,
    },
    RemoveView {
        view: ViewId,
    },
    SetFilters {
        view: ViewId,
        filters: Vec<Filter>,
    },
    SetSorts {
        view: ViewId,
        sorts: Vec<Sort>,
    },
    SetVisibleProperties {
        view: ViewId,
        properties: Vec<PropertyId>,
    },
    SetColumnWidth {
        view: ViewId,
        property: PropertyId,
        width: u32,
    },
    SetViewKind {
        view: ViewId,
        kind: ViewKind,
    },
    MoveRow {
        view: ViewId,
        row: BlockId,
        before: Option<BlockId>,
    },
    MoveCard {
        view: ViewId,
        row: BlockId,
        group: Option<OptionId>,
        before: Option<BlockId>,
    },
}

impl Command {
    #[must_use]
    pub fn is_block_command(&self) -> bool {
        matches!(
            self,
            Self::CreatePage { .. }
                | Self::InsertBlock { .. }
                | Self::InsertBlockAt { .. }
                | Self::EditText { .. }
                | Self::SplitBlock { .. }
                | Self::MergeWithPrevious { .. }
                | Self::DeleteRange { .. }
                | Self::SetMarks { .. }
                | Self::Indent { .. }
                | Self::Outdent { .. }
                | Self::ChangeKind { .. }
                | Self::MoveBlock { .. }
                | Self::DeleteBlocks { .. }
                | Self::SetPageIcon { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Created {
    Block(BlockId),
    Database(DatabaseId),
    View(ViewId),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    pub blocks: BTreeSet<BlockId>,
    pub databases: BTreeSet<DatabaseId>,
    pub views: BTreeSet<ViewId>,
    pub created: Vec<Created>,
    pub caret: Option<TextPoint>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    pub transaction: Transaction,
    pub created: Vec<Created>,
    pub caret: Option<TextPoint>,
}

pub(crate) fn build(
    workspace: &Workspace,
    command: &Command,
    now: Timestamp,
) -> Result<Built, DomainError> {
    if command.is_block_command() {
        blocks::build(workspace, command, now)
    } else {
        databases::build(workspace, command, now)
    }
}
