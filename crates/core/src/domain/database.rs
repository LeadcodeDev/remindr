use std::collections::HashSet;

use super::{
    BlockId, BlockKind, DatabaseId, DomainError, Emoji, Filter, Operation, OptionId, Parent,
    PropertyId, SortKey, Timestamp, View, ViewKind, Workspace,
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

impl PropertyKind {
    pub(crate) fn options(&self) -> Option<&[SelectOption]> {
        match self {
            Self::Select { options } | Self::MultiSelect { options } => Some(options),
            _ => None,
        }
    }

    fn is_title(&self) -> bool {
        matches!(self, Self::Title)
    }
}

impl Schema {
    pub(crate) fn find(&self, id: PropertyId) -> Option<(usize, &Property)> {
        self.properties
            .iter()
            .enumerate()
            .find(|(_, property)| property.id == id)
    }

    fn has_single_title(&self) -> bool {
        self.properties
            .iter()
            .filter(|property| property.kind.is_title())
            .count()
            == 1
    }

    fn has_duplicate_ids(&self) -> bool {
        self.properties.iter().enumerate().any(|(index, property)| {
            self.properties[..index]
                .iter()
                .any(|earlier| earlier.id == property.id)
        })
    }
}

fn check_option(options: &[SelectOption], option: OptionId) -> Result<(), DomainError> {
    if options.iter().any(|candidate| candidate.id == option) {
        Ok(())
    } else {
        Err(DomainError::UnknownOption(option))
    }
}

fn check_value(kind: &PropertyKind, value: &PropertyValue) -> Result<(), DomainError> {
    match (kind, value) {
        (PropertyKind::Text, PropertyValue::Text(_))
        | (PropertyKind::Url, PropertyValue::Url(_))
        | (PropertyKind::Date, PropertyValue::Date(_))
        | (PropertyKind::Checkbox, PropertyValue::Checkbox(_)) => Ok(()),
        (PropertyKind::Number { .. }, PropertyValue::Number(number)) if number.is_finite() => {
            Ok(())
        }
        (PropertyKind::Select { options }, PropertyValue::Select(option)) => {
            check_option(options, *option)
        }
        (PropertyKind::MultiSelect { options }, PropertyValue::MultiSelect(chosen)) => chosen
            .iter()
            .try_for_each(|option| check_option(options, *option)),
        _ => Err(DomainError::ValueKindMismatch),
    }
}

fn check_filter(filter: &Filter, kind: &PropertyKind) -> Result<(), DomainError> {
    if filter.condition.fits(kind) {
        Ok(())
    } else {
        Err(DomainError::ConditionKindMismatch)
    }
}

fn check_group_by(kind: &PropertyKind) -> Result<(), DomainError> {
    if matches!(kind, PropertyKind::Select { .. }) {
        Ok(())
    } else {
        Err(DomainError::ValueKindMismatch)
    }
}

impl Workspace {
    pub(crate) fn apply_database_operation(
        &mut self,
        operation: &Operation,
    ) -> Result<Operation, DomainError> {
        match operation {
            Operation::InsertDatabase(database) => self.insert_database(database),
            Operation::RemoveDatabase { database } => self.remove_database(*database),
            Operation::SetDatabaseName { database, name } => {
                let record = self.database_mut(*database)?;
                let old = std::mem::replace(&mut record.name, name.clone());
                Ok(Operation::SetDatabaseName {
                    database: *database,
                    name: old,
                })
            }
            Operation::SetDatabaseIcon { database, icon } => {
                let record = self.database_mut(*database)?;
                let old = std::mem::replace(&mut record.icon, icon.clone());
                Ok(Operation::SetDatabaseIcon {
                    database: *database,
                    icon: old,
                })
            }
            Operation::SetDatabaseAlive { database, alive } => {
                let record = self.database_mut(*database)?;
                let old = std::mem::replace(&mut record.alive, *alive);
                Ok(Operation::SetDatabaseAlive {
                    database: *database,
                    alive: old,
                })
            }
            Operation::PutProperty {
                database,
                property,
                index,
            } => self.put_property(*database, property, *index),
            Operation::RemoveProperty { database, property } => {
                self.remove_property(*database, *property)
            }
            Operation::SetValue {
                row,
                property,
                value,
            } => self.set_value(*row, *property, value.as_ref()),
            Operation::PutView(view) => self.put_view(view),
            Operation::RemoveView { view } => self
                .views
                .remove(view)
                .map(Operation::PutView)
                .ok_or(DomainError::UnknownView(*view)),
            Operation::InsertBlock(_)
            | Operation::RemoveBlock { .. }
            | Operation::SetText { .. }
            | Operation::SetKind { .. }
            | Operation::Move { .. }
            | Operation::SetBlockAlive { .. } => self.apply_block_operation(operation),
        }
    }

    pub(crate) fn validate_databases(&self) -> Result<(), DomainError> {
        let mut seen = HashSet::new();
        for database in self.databases.values() {
            if !database.schema.has_single_title() {
                return Err(DomainError::TitleIsBlockText);
            }
            for property in &database.schema.properties {
                if !seen.insert(property.id) {
                    return Err(DomainError::AlreadyExists);
                }
            }
        }
        for ((row, property), value) in &self.values {
            let definition = self.value_target(*row, *property)?;
            check_value(&definition.kind, value)?;
        }
        self.views
            .values()
            .try_for_each(|view| self.validate_view(view))
    }

    pub fn value(&self, row: BlockId, property: PropertyId) -> Option<&PropertyValue> {
        self.values.get(&(row, property))
    }

    fn database_mut(&mut self, id: DatabaseId) -> Result<&mut Database, DomainError> {
        self.databases
            .get_mut(&id)
            .ok_or(DomainError::UnknownDatabase(id))
    }

    fn property_id_taken_elsewhere(&self, database: DatabaseId, property: PropertyId) -> bool {
        self.databases
            .values()
            .filter(|other| other.id != database)
            .any(|other| other.schema.find(property).is_some())
    }

    fn insert_database(&mut self, database: &Database) -> Result<Operation, DomainError> {
        if self.databases.contains_key(&database.id) || database.schema.has_duplicate_ids() {
            return Err(DomainError::AlreadyExists);
        }
        if !database.schema.has_single_title() {
            return Err(DomainError::TitleIsBlockText);
        }
        let taken = database
            .schema
            .properties
            .iter()
            .any(|property| self.property_id_taken_elsewhere(database.id, property.id));
        if taken {
            return Err(DomainError::AlreadyExists);
        }
        self.databases.insert(database.id, database.clone());
        Ok(Operation::RemoveDatabase {
            database: database.id,
        })
    }

    fn remove_database(&mut self, id: DatabaseId) -> Result<Operation, DomainError> {
        let Some(record) = self.databases.get(&id) else {
            return Err(DomainError::UnknownDatabase(id));
        };
        let has_rows = self
            .children
            .get(&Parent::Database(id))
            .is_some_and(|rows| !rows.is_empty());
        let is_shown = self.blocks.values().any(
            |block| matches!(block.kind, BlockKind::DatabaseView { database } if database == id),
        );
        let has_values = self
            .values
            .keys()
            .any(|(_, property)| record.schema.find(*property).is_some());
        if has_rows || is_shown || has_values {
            return Err(DomainError::StillReferenced);
        }
        self.databases
            .remove(&id)
            .map(Operation::InsertDatabase)
            .ok_or(DomainError::UnknownDatabase(id))
    }

    fn put_property(
        &mut self,
        database: DatabaseId,
        property: &Property,
        index: usize,
    ) -> Result<Operation, DomainError> {
        let record = self
            .databases
            .get(&database)
            .ok_or(DomainError::UnknownDatabase(database))?;
        if self.property_id_taken_elsewhere(database, property.id) {
            return Err(DomainError::AlreadyExists);
        }
        let titles_kept = record
            .schema
            .properties
            .iter()
            .filter(|other| other.id != property.id && other.kind.is_title())
            .count();
        if titles_kept + usize::from(property.kind.is_title()) != 1 {
            return Err(DomainError::TitleIsBlockText);
        }
        if record.schema.find(property.id).is_some() {
            self.values
                .iter()
                .filter(|((_, id), _)| *id == property.id)
                .try_for_each(|(_, value)| check_value(&property.kind, value))?;
            for view in self.database_views(database) {
                for filter in view.filters.iter().filter(|f| f.property == property.id) {
                    check_filter(filter, &property.kind)?;
                }
                if matches!(view.kind, ViewKind::Board { group_by } if group_by == property.id) {
                    check_group_by(&property.kind)?;
                }
            }
        }
        let record = self.database_mut(database)?;
        let previous = record
            .schema
            .find(property.id)
            .map(|(old_index, _)| old_index)
            .map(|old_index| (old_index, record.schema.properties.remove(old_index)));
        let at = index.min(record.schema.properties.len());
        record.schema.properties.insert(at, property.clone());
        Ok(match previous {
            Some((old_index, old)) => Operation::PutProperty {
                database,
                property: old,
                index: old_index,
            },
            None => Operation::RemoveProperty {
                database,
                property: property.id,
            },
        })
    }

    fn remove_property(
        &mut self,
        database: DatabaseId,
        property: PropertyId,
    ) -> Result<Operation, DomainError> {
        let record = self
            .databases
            .get(&database)
            .ok_or(DomainError::UnknownDatabase(database))?;
        let (index, definition) = record
            .schema
            .find(property)
            .ok_or(DomainError::UnknownProperty(property))?;
        if definition.kind.is_title() {
            return Err(DomainError::TitleIsBlockText);
        }
        let has_values = self.values.keys().any(|(_, id)| *id == property);
        let is_referenced = self
            .database_views(database)
            .iter()
            .any(|view| view.references(property));
        if has_values || is_referenced {
            return Err(DomainError::StillReferenced);
        }
        let record = self.database_mut(database)?;
        let removed = record.schema.properties.remove(index);
        Ok(Operation::PutProperty {
            database,
            property: removed,
            index,
        })
    }

    fn value_target(&self, row: BlockId, property: PropertyId) -> Result<&Property, DomainError> {
        let block = self
            .blocks
            .get(&row)
            .ok_or(DomainError::UnknownBlock(row))?;
        let (Parent::Database(database), BlockKind::Page { .. }) = (block.parent, &block.kind)
        else {
            return Err(DomainError::InvalidParent);
        };
        let record = self
            .databases
            .get(&database)
            .ok_or(DomainError::UnknownDatabase(database))?;
        let (_, definition) = record
            .schema
            .find(property)
            .ok_or(DomainError::UnknownProperty(property))?;
        if definition.kind.is_title() {
            return Err(DomainError::TitleIsBlockText);
        }
        Ok(definition)
    }

    fn set_value(
        &mut self,
        row: BlockId,
        property: PropertyId,
        value: Option<&PropertyValue>,
    ) -> Result<Operation, DomainError> {
        let definition = self.value_target(row, property)?;
        if let Some(value) = value {
            check_value(&definition.kind, value)?;
        }
        let old = match value {
            Some(value) => self.values.insert((row, property), value.clone()),
            None => self.values.remove(&(row, property)),
        };
        Ok(Operation::SetValue {
            row,
            property,
            value: old,
        })
    }

    fn put_view(&mut self, view: &View) -> Result<Operation, DomainError> {
        self.validate_view(view)?;
        Ok(match self.views.insert(view.id, view.clone()) {
            Some(old) => Operation::PutView(old),
            None => Operation::RemoveView { view: view.id },
        })
    }

    fn validate_view(&self, view: &View) -> Result<(), DomainError> {
        let block = self
            .blocks
            .get(&view.block)
            .ok_or(DomainError::UnknownBlock(view.block))?;
        let BlockKind::DatabaseView { database } = block.kind else {
            return Err(DomainError::InvalidParent);
        };
        let schema = &self
            .databases
            .get(&database)
            .ok_or(DomainError::UnknownDatabase(database))?
            .schema;
        let known = |id: PropertyId| {
            schema
                .find(id)
                .map(|(_, property)| property)
                .ok_or(DomainError::UnknownProperty(id))
        };
        for filter in &view.filters {
            check_filter(filter, &known(filter.property)?.kind)?;
        }
        for sort in &view.sorts {
            known(sort.property)?;
        }
        for id in view.visible.iter().chain(view.column_widths.keys()) {
            known(*id)?;
        }
        if let ViewKind::Board { group_by } = view.kind {
            check_group_by(&known(group_by)?.kind)?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) mod fixture;
#[cfg(test)]
mod tests;
