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
        let valid = (1..=12).contains(&month) && day >= 1 && day <= days_in_month(year, month);
        if valid {
            Ok(CalendarDate { year, month, day })
        } else {
            Err(DomainError::InvalidDate)
        }
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

fn days_in_month(year: i32, month: u8) -> u8 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
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

    pub(crate) fn validate_databases(&self) -> Result<(), DomainError> {
        Ok(())
    }

    pub fn value(&self, row: BlockId, property: PropertyId) -> Option<&PropertyValue> {
        self.values.get(&(row, property))
    }
}

#[cfg(test)]
mod tests {
    use super::CalendarDate;

    #[test]
    fn calendar_date_follows_the_gregorian_calendar() {
        assert!(CalendarDate::new(2024, 2, 29).is_ok());
        assert!(CalendarDate::new(2023, 2, 29).is_err());
        assert!(CalendarDate::new(1900, 2, 29).is_err());
        assert!(CalendarDate::new(2000, 2, 29).is_ok());
        assert!(CalendarDate::new(2026, 4, 31).is_err());
        assert!(CalendarDate::new(2026, 13, 1).is_err());
        assert!(CalendarDate::new(2026, 1, 0).is_err());
    }
}
