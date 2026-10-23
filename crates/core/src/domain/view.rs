use std::collections::BTreeMap;

use super::{BlockId, CalendarDate, DomainError, OptionId, PropertyId, SortKey, ViewId, Workspace};

#[derive(Clone, Debug, PartialEq)]
pub enum TextCondition {
    Contains(String),
    DoesNotContain(String),
    Is(String),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NumberCondition {
    Eq(f64),
    Ne(f64),
    Lt(f64),
    Le(f64),
    Gt(f64),
    Ge(f64),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SelectCondition {
    Is(OptionId),
    IsNot(OptionId),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum MultiSelectCondition {
    Contains(OptionId),
    DoesNotContain(OptionId),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DateCondition {
    Is(CalendarDate),
    Before(CalendarDate),
    After(CalendarDate),
    IsEmpty,
    IsNotEmpty,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Condition {
    Text(TextCondition),
    Number(NumberCondition),
    Select(SelectCondition),
    MultiSelect(MultiSelectCondition),
    Date(DateCondition),
    Checkbox(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Filter {
    pub property: PropertyId,
    pub condition: Condition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Sort {
    pub property: PropertyId,
    pub direction: SortDirection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ViewKind {
    Table,
    Board { group_by: PropertyId },
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub id: ViewId,
    pub block: BlockId,
    pub name: String,
    pub sort_key: SortKey,
    pub kind: ViewKind,
    pub filters: Vec<Filter>,
    pub sorts: Vec<Sort>,
    pub visible: Vec<PropertyId>,
    pub column_widths: BTreeMap<PropertyId, u32>,
    pub manual_order: Vec<BlockId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoardGroup {
    pub option: Option<OptionId>,
    pub rows: Vec<BlockId>,
}

impl Workspace {
    pub fn query(&self, view: ViewId) -> Result<Vec<BlockId>, DomainError> {
        todo!()
    }

    pub fn board_groups(&self, view: ViewId) -> Result<Vec<BoardGroup>, DomainError> {
        todo!()
    }
}
