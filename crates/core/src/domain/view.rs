use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};

use super::{
    Block, BlockId, BlockKind, CalendarDate, Database, DatabaseId, DomainError, OptionId, Property,
    PropertyId, PropertyKind, PropertyValue, SortKey, ViewId, Workspace,
};

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

impl NumberCondition {
    fn target(&self) -> Option<f64> {
        match self {
            Self::Eq(target)
            | Self::Ne(target)
            | Self::Lt(target)
            | Self::Le(target)
            | Self::Gt(target)
            | Self::Ge(target) => Some(*target),
            Self::IsEmpty | Self::IsNotEmpty => None,
        }
    }
}

impl Condition {
    pub(crate) fn has_finite_numbers(&self) -> bool {
        match self {
            Self::Number(condition) => condition.target().is_none_or(f64::is_finite),
            _ => true,
        }
    }

    pub(crate) fn fits(&self, kind: &PropertyKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::Text(_),
                PropertyKind::Title | PropertyKind::Text | PropertyKind::Url
            ) | (Self::Number(_), PropertyKind::Number { .. })
                | (Self::Select(_), PropertyKind::Select { .. })
                | (Self::MultiSelect(_), PropertyKind::MultiSelect { .. })
                | (Self::Date(_), PropertyKind::Date)
                | (Self::Checkbox(_), PropertyKind::Checkbox)
        )
    }

    pub(crate) fn mentions_option(&self, option: OptionId) -> bool {
        match self {
            Self::Select(SelectCondition::Is(id) | SelectCondition::IsNot(id))
            | Self::MultiSelect(
                MultiSelectCondition::Contains(id) | MultiSelectCondition::DoesNotContain(id),
            ) => *id == option,
            _ => false,
        }
    }
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

impl View {
    pub(crate) fn references(&self, property: PropertyId) -> bool {
        self.filters
            .iter()
            .any(|filter| filter.property == property)
            || self.sorts.iter().any(|sort| sort.property == property)
            || self.visible.contains(&property)
            || self.column_widths.contains_key(&property)
            || matches!(self.kind, ViewKind::Board { group_by } if group_by == property)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct BoardGroup {
    pub option: Option<OptionId>,
    pub rows: Vec<BlockId>,
}

impl Workspace {
    pub(crate) fn view_database(&self, view: &View) -> Option<&Database> {
        let block = self.blocks.get(&view.block)?;
        let BlockKind::DatabaseView { database } = &block.kind else {
            return None;
        };
        self.databases.get(database)
    }

    pub(crate) fn database_views(&self, database: DatabaseId) -> Vec<&View> {
        let mut views: Vec<&View> = self
            .views
            .values()
            .filter(|view| {
                self.view_database(view)
                    .is_some_and(|owner| owner.id == database)
            })
            .collect();
        views.sort_by_key(|view| view.id);
        views
    }

    pub fn query(&self, view: ViewId) -> Result<Vec<BlockId>, DomainError> {
        let view = self
            .views
            .get(&view)
            .ok_or(DomainError::UnknownView(view))?;
        let Some(database) = self.view_database(view).filter(|database| database.alive) else {
            return Ok(Vec::new());
        };
        let schema = &database.schema;
        let sorts: Vec<(&Property, SortDirection)> = view
            .sorts
            .iter()
            .filter_map(|sort| {
                schema
                    .find(sort.property)
                    .map(|(_, property)| (property, sort.direction))
            })
            .collect();
        let positions = manual_positions(&view.manual_order);
        let mut ranked: Vec<RankedRow> = self
            .rows(database.id)
            .filter(|row| {
                view.filters
                    .iter()
                    .all(|filter| self.filter_matches(database, filter, row))
            })
            .map(|row| RankedRow {
                id: row.id,
                sort_values: sorts
                    .iter()
                    .map(|(property, _)| self.sort_value(property, row))
                    .collect(),
                position: positions.get(&row.id).copied().unwrap_or(usize::MAX),
            })
            .collect();
        ranked.sort_by(|left, right| {
            for (index, (_, direction)) in sorts.iter().enumerate() {
                let ordering = compare_cells(
                    left.sort_values[index].as_ref(),
                    right.sort_values[index].as_ref(),
                    *direction,
                );
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }
            left.position.cmp(&right.position)
        });
        Ok(ranked.into_iter().map(|row| row.id).collect())
    }

    pub fn board_groups(&self, view: ViewId) -> Result<Vec<BoardGroup>, DomainError> {
        let ordered = self.query(view)?;
        let view = self
            .views
            .get(&view)
            .ok_or(DomainError::UnknownView(view))?;
        let ViewKind::Board { group_by } = view.kind else {
            return Err(DomainError::ValueKindMismatch);
        };
        let Some(database) = self.view_database(view) else {
            return Ok(Vec::new());
        };
        let (_, property) = database
            .schema
            .find(group_by)
            .ok_or(DomainError::UnknownProperty(group_by))?;
        let PropertyKind::Select { options } = &property.kind else {
            return Err(DomainError::ValueKindMismatch);
        };
        let mut groups: Vec<BoardGroup> = std::iter::once(None)
            .chain(options.iter().map(|option| Some(option.id)))
            .map(|option| BoardGroup {
                option,
                rows: Vec::new(),
            })
            .collect();
        for row in ordered {
            let slot = match self.values.get(&(row, group_by)) {
                Some(PropertyValue::Select(chosen)) => options
                    .iter()
                    .position(|option| option.id == *chosen)
                    .map_or(0, |index| index + 1),
                _ => 0,
            };
            groups[slot].rows.push(row);
        }
        Ok(groups)
    }

    fn filter_matches(&self, database: &Database, filter: &Filter, row: &Block) -> bool {
        let Some((_, property)) = database.schema.find(filter.property) else {
            return true;
        };
        let value = self.values.get(&(row.id, property.id));
        match &filter.condition {
            Condition::Text(condition) => text_matches(condition, &text_of(property, row, value)),
            Condition::Number(condition) => number_matches(condition, number_of(value)),
            Condition::Select(condition) => select_matches(condition, value),
            Condition::MultiSelect(condition) => multi_select_matches(condition, value),
            Condition::Date(condition) => date_matches(condition, value),
            Condition::Checkbox(expected) => {
                matches!(value, Some(PropertyValue::Checkbox(true))) == *expected
            }
        }
    }

    fn sort_value(&self, property: &Property, row: &Block) -> Option<SortValue> {
        let value = self.values.get(&(row.id, property.id));
        match (&property.kind, value) {
            (PropertyKind::Title, _) => text_sort_value(&row.text.to_plain()),
            (PropertyKind::Text, Some(PropertyValue::Text(text)))
            | (PropertyKind::Url, Some(PropertyValue::Url(text))) => text_sort_value(text),
            (PropertyKind::Number { .. }, Some(PropertyValue::Number(number))) => {
                Some(SortValue::Number(*number))
            }
            (PropertyKind::Select { options }, Some(PropertyValue::Select(chosen))) => options
                .iter()
                .position(|option| option.id == *chosen)
                .map(SortValue::Index),
            (PropertyKind::MultiSelect { options }, Some(PropertyValue::MultiSelect(chosen))) => {
                let indices: Vec<usize> = chosen
                    .iter()
                    .filter_map(|id| options.iter().position(|option| option.id == *id))
                    .collect();
                (!indices.is_empty()).then_some(SortValue::Indices(indices))
            }
            (PropertyKind::Date, Some(PropertyValue::Date(date))) => Some(SortValue::Date(*date)),
            (PropertyKind::Checkbox, value) => Some(SortValue::Flag(matches!(
                value,
                Some(PropertyValue::Checkbox(true))
            ))),
            _ => None,
        }
    }
}

struct RankedRow {
    id: BlockId,
    sort_values: Vec<Option<SortValue>>,
    position: usize,
}

enum SortValue {
    Text(String),
    Number(f64),
    Index(usize),
    Indices(Vec<usize>),
    Date(CalendarDate),
    Flag(bool),
}

fn manual_positions(order: &[BlockId]) -> HashMap<BlockId, usize> {
    let mut positions = HashMap::with_capacity(order.len());
    for (position, id) in order.iter().enumerate() {
        positions.entry(*id).or_insert(position);
    }
    positions
}

fn text_sort_value(text: &str) -> Option<SortValue> {
    (!text.is_empty()).then(|| SortValue::Text(text.to_lowercase()))
}

fn compare_values(left: &SortValue, right: &SortValue) -> Ordering {
    match (left, right) {
        (SortValue::Text(left), SortValue::Text(right)) => left.cmp(right),
        (SortValue::Number(left), SortValue::Number(right)) => {
            left.partial_cmp(right).unwrap_or(Ordering::Equal)
        }
        (SortValue::Index(left), SortValue::Index(right)) => left.cmp(right),
        (SortValue::Indices(left), SortValue::Indices(right)) => left.cmp(right),
        (SortValue::Date(left), SortValue::Date(right)) => left.cmp(right),
        (SortValue::Flag(left), SortValue::Flag(right)) => left.cmp(right),
        _ => Ordering::Equal,
    }
}

fn compare_cells(
    left: Option<&SortValue>,
    right: Option<&SortValue>,
    direction: SortDirection,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => match direction {
            SortDirection::Ascending => compare_values(left, right),
            SortDirection::Descending => compare_values(right, left),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn text_of<'a>(property: &Property, row: &Block, value: Option<&'a PropertyValue>) -> Cow<'a, str> {
    match (&property.kind, value) {
        (PropertyKind::Title, _) => Cow::Owned(row.text.to_plain()),
        (_, Some(PropertyValue::Text(text) | PropertyValue::Url(text))) => Cow::Borrowed(text),
        _ => Cow::Borrowed(""),
    }
}

fn number_of(value: Option<&PropertyValue>) -> Option<f64> {
    match value {
        Some(PropertyValue::Number(number)) => Some(*number),
        _ => None,
    }
}

fn text_matches(condition: &TextCondition, text: &str) -> bool {
    let text = text.to_lowercase();
    match condition {
        TextCondition::Contains(needle) => text.contains(&needle.to_lowercase()),
        TextCondition::DoesNotContain(needle) => !text.contains(&needle.to_lowercase()),
        TextCondition::Is(expected) => text == expected.to_lowercase(),
        TextCondition::IsEmpty => text.is_empty(),
        TextCondition::IsNotEmpty => !text.is_empty(),
    }
}

fn number_matches(condition: &NumberCondition, value: Option<f64>) -> bool {
    match condition {
        NumberCondition::IsEmpty => value.is_none(),
        NumberCondition::IsNotEmpty => value.is_some(),
        NumberCondition::Eq(target) => value.is_some_and(|number| number == *target),
        NumberCondition::Ne(target) => value.is_some_and(|number| number != *target),
        NumberCondition::Lt(target) => value.is_some_and(|number| number < *target),
        NumberCondition::Le(target) => value.is_some_and(|number| number <= *target),
        NumberCondition::Gt(target) => value.is_some_and(|number| number > *target),
        NumberCondition::Ge(target) => value.is_some_and(|number| number >= *target),
    }
}

fn select_matches(condition: &SelectCondition, value: Option<&PropertyValue>) -> bool {
    let chosen = match value {
        Some(PropertyValue::Select(option)) => Some(*option),
        _ => None,
    };
    match condition {
        SelectCondition::Is(option) => chosen == Some(*option),
        SelectCondition::IsNot(option) => chosen != Some(*option),
        SelectCondition::IsEmpty => chosen.is_none(),
        SelectCondition::IsNotEmpty => chosen.is_some(),
    }
}

fn multi_select_matches(condition: &MultiSelectCondition, value: Option<&PropertyValue>) -> bool {
    let chosen: &[OptionId] = match value {
        Some(PropertyValue::MultiSelect(options)) => options,
        _ => &[],
    };
    match condition {
        MultiSelectCondition::Contains(option) => chosen.contains(option),
        MultiSelectCondition::DoesNotContain(option) => !chosen.contains(option),
        MultiSelectCondition::IsEmpty => chosen.is_empty(),
        MultiSelectCondition::IsNotEmpty => !chosen.is_empty(),
    }
}

fn date_matches(condition: &DateCondition, value: Option<&PropertyValue>) -> bool {
    let date = match value {
        Some(PropertyValue::Date(date)) => Some(*date),
        _ => None,
    };
    match condition {
        DateCondition::IsEmpty => date.is_none(),
        DateCondition::IsNotEmpty => date.is_some(),
        DateCondition::Is(target) => date == Some(*target),
        DateCondition::Before(target) => date.is_some_and(|date| date < *target),
        DateCondition::After(target) => date.is_some_and(|date| date > *target),
    }
}

#[cfg(test)]
mod tests;
