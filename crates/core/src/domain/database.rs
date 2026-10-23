use super::{
    BlockId, DatabaseId, DomainError, Emoji, Operation, OptionId, PropertyId, SortKey, Timestamp,
    Workspace,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OptionColor {
    Default,
    Gray,
    Brown,
    Orange,
    Yellow,
    Green,
    Blue,
    Purple,
    Pink,
    Red,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SelectOption {
    pub id: OptionId,
    pub name: String,
    pub color: OptionColor,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NumberFormat {
    Plain,
    Percent,
    Euro,
    Dollar,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PropertyKind {
    Title,
    Text,
    Number { format: NumberFormat },
    Select { options: Vec<SelectOption> },
    MultiSelect { options: Vec<SelectOption> },
    Date,
    Checkbox,
    Url,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Property {
    pub id: PropertyId,
    pub name: String,
    pub kind: PropertyKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Schema {
    pub properties: Vec<Property>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CalendarDate {
    year: i32,
    month: u8,
    day: u8,
}

impl CalendarDate {
    pub fn new(year: i32, month: u8, day: u8) -> Result<CalendarDate, DomainError> {
        todo!()
    }

    #[must_use]
    pub fn year(&self) -> i32 {
        self.year
    }

    #[must_use]
    pub fn month(&self) -> u8 {
        self.month
    }

    #[must_use]
    pub fn day(&self) -> u8 {
        self.day
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
    Text(String),
    Number(f64),
    Select(OptionId),
    MultiSelect(Vec<OptionId>),
    Date(CalendarDate),
    Checkbox(bool),
    Url(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Database {
    pub id: DatabaseId,
    pub name: String,
    pub icon: Option<Emoji>,
    pub schema: Schema,
    pub sort_key: SortKey,
    pub alive: bool,
    pub created_at: Timestamp,
}

impl Workspace {
    pub(crate) fn apply_database_operation(
        &mut self,
        operation: &Operation,
    ) -> Result<Operation, DomainError> {
        todo!()
    }

    pub fn value(&self, row: BlockId, property: PropertyId) -> Option<&PropertyValue> {
        self.values.get(&(row, property))
    }
}
