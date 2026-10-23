use std::borrow::Cow;
use std::collections::BTreeMap;

use remindr_core::domain::{
    BlockId, Condition, DateCondition, Filter, MultiSelectCondition, NumberCondition, OptionId,
    PropertyId, SelectCondition, Sort, SortDirection, SortKey, TextCondition, View, ViewId,
    ViewKind,
};
use remindr_core::ports::StoreError;
use rusqlite::Row;
use serde::{Deserialize, Serialize};

use super::value::{date_text, parse_date};
use super::{Invalid, Named, StoredId, corrupt, decode, named, to_json};

const KIND: &str = "view.kind";
const CONFIG: &str = "view.config";

named!(SortDirection {
    SortDirection::Ascending => "ascending",
    SortDirection::Descending => "descending",
});

#[derive(Serialize, Deserialize)]
struct Config<'a> {
    filters: Vec<FilterData<'a>>,
    sorts: Vec<SortData<'a>>,
    visible: Vec<String>,
    column_widths: BTreeMap<String, u32>,
    manual_order: Vec<String>,
    group_by: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct FilterData<'a> {
    property: String,
    condition: ConditionData<'a>,
}

#[derive(Serialize, Deserialize)]
struct SortData<'a> {
    property: String,
    direction: Cow<'a, str>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TextOp {
    Contains,
    DoesNotContain,
    Is,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum NumberOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SelectOp {
    Is,
    IsNot,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MultiSelectOp {
    Contains,
    DoesNotContain,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DateOp {
    Is,
    Before,
    After,
    IsEmpty,
    IsNotEmpty,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CheckboxOp {
    Is,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ConditionData<'a> {
    Text {
        op: TextOp,
        value: Option<Cow<'a, str>>,
    },
    Number {
        op: NumberOp,
        #[serde(serialize_with = "super::finite_option")]
        value: Option<f64>,
    },
    Select {
        op: SelectOp,
        value: Option<String>,
    },
    MultiSelect {
        op: MultiSelectOp,
        value: Option<String>,
    },
    Date {
        op: DateOp,
        value: Option<String>,
    },
    Checkbox {
        op: CheckboxOp,
        value: bool,
    },
}

pub(crate) struct ViewRow {
    id: String,
    block_id: String,
    name: String,
    kind: String,
    sort_key: String,
    config: String,
}

impl ViewRow {
    pub(crate) const SELECT: &str =
        "SELECT id, block_id, name, kind, sort_key, config FROM view ORDER BY id";

    pub(crate) fn from_row(row: &Row<'_>) -> rusqlite::Result<ViewRow> {
        Ok(ViewRow {
            id: row.get(0)?,
            block_id: row.get(1)?,
            name: row.get(2)?,
            kind: row.get(3)?,
            sort_key: row.get(4)?,
            config: row.get(5)?,
        })
    }

    pub(crate) fn into_view(self) -> Result<View, StoreError> {
        let config: Config<'_> = decode(CONFIG, &self.config)?;
        let kind = view_kind(&self.kind, config.group_by.as_deref())?;
        let (filters, sorts, visible, column_widths, manual_order) =
            config_parts(config).map_err(corrupt(CONFIG))?;
        Ok(View {
            id: ViewId::parse(&self.id).map_err(corrupt("view.id"))?,
            block: BlockId::parse(&self.block_id).map_err(corrupt("view.block_id"))?,
            name: self.name,
            sort_key: SortKey::parse(&self.sort_key).map_err(corrupt("view.sort_key"))?,
            kind,
            filters,
            sorts,
            visible,
            column_widths,
            manual_order,
        })
    }
}

type ConfigParts = (
    Vec<Filter>,
    Vec<Sort>,
    Vec<PropertyId>,
    BTreeMap<PropertyId, u32>,
    Vec<BlockId>,
);

fn config_parts(config: Config<'_>) -> Result<ConfigParts, Invalid> {
    let filters = config
        .filters
        .into_iter()
        .map(filter)
        .collect::<Result<_, Invalid>>()?;
    let sorts = config
        .sorts
        .iter()
        .map(sort)
        .collect::<Result<_, Invalid>>()?;
    let visible = config
        .visible
        .iter()
        .map(|property| PropertyId::parse(property))
        .collect::<Result<_, Invalid>>()?;
    let column_widths = config
        .column_widths
        .iter()
        .map(|(property, width)| Ok((PropertyId::parse(property)?, *width)))
        .collect::<Result<_, Invalid>>()?;
    let manual_order = config
        .manual_order
        .iter()
        .map(|row| BlockId::parse(row))
        .collect::<Result<_, Invalid>>()?;
    Ok((filters, sorts, visible, column_widths, manual_order))
}

fn view_kind(name: &str, group_by: Option<&str>) -> Result<ViewKind, StoreError> {
    match name {
        "table" => Ok(ViewKind::Table),
        "board" => group_by
            .ok_or_else(|| Invalid::new("board view without group_by"))
            .and_then(PropertyId::parse)
            .map(|group_by| ViewKind::Board { group_by })
            .map_err(corrupt(CONFIG)),
        other => Err(corrupt(KIND)(Invalid::new(format!(
            "unknown kind `{other}`"
        )))),
    }
}

fn filter(data: FilterData<'_>) -> Result<Filter, Invalid> {
    Ok(Filter {
        property: PropertyId::parse(&data.property)?,
        condition: data.condition.try_into()?,
    })
}

fn sort(data: &SortData<'_>) -> Result<Sort, Invalid> {
    Ok(Sort {
        property: PropertyId::parse(&data.property)?,
        direction: named(&data.direction, "sort direction")?,
    })
}

pub(crate) fn encode_view(view: &View) -> Result<(&'static str, String), StoreError> {
    let (kind, group_by) = match view.kind {
        ViewKind::Table => ("table", None),
        ViewKind::Board { group_by } => ("board", Some(group_by.to_text())),
    };
    let config = Config {
        filters: view
            .filters
            .iter()
            .map(|filter| FilterData {
                property: filter.property.to_text(),
                condition: ConditionData::from(&filter.condition),
            })
            .collect(),
        sorts: view
            .sorts
            .iter()
            .map(|sort| SortData {
                property: sort.property.to_text(),
                direction: Cow::Borrowed(sort.direction.name()),
            })
            .collect(),
        visible: view.visible.iter().map(StoredId::to_text).collect(),
        column_widths: view
            .column_widths
            .iter()
            .map(|(property, width)| (property.to_text(), *width))
            .collect(),
        manual_order: view.manual_order.iter().map(StoredId::to_text).collect(),
        group_by,
    };
    Ok((kind, to_json(&config)?))
}

impl<'a> From<&'a Condition> for ConditionData<'a> {
    fn from(condition: &'a Condition) -> Self {
        match condition {
            Condition::Text(text) => text_data(text),
            Condition::Number(number) => number_data(number),
            Condition::Select(select) => select_data(select),
            Condition::MultiSelect(multi) => multi_select_data(multi),
            Condition::Date(date) => date_data(date),
            Condition::Checkbox(checked) => ConditionData::Checkbox {
                op: CheckboxOp::Is,
                value: *checked,
            },
        }
    }
}

fn text_data(condition: &TextCondition) -> ConditionData<'_> {
    let (op, value) = match condition {
        TextCondition::Contains(value) => (TextOp::Contains, Some(value)),
        TextCondition::DoesNotContain(value) => (TextOp::DoesNotContain, Some(value)),
        TextCondition::Is(value) => (TextOp::Is, Some(value)),
        TextCondition::IsEmpty => (TextOp::IsEmpty, None),
        TextCondition::IsNotEmpty => (TextOp::IsNotEmpty, None),
    };
    ConditionData::Text {
        op,
        value: value.map(|value| Cow::Borrowed(value.as_str())),
    }
}

fn number_data(condition: &NumberCondition) -> ConditionData<'_> {
    let (op, value) = match condition {
        NumberCondition::Eq(value) => (NumberOp::Eq, Some(*value)),
        NumberCondition::Ne(value) => (NumberOp::Ne, Some(*value)),
        NumberCondition::Lt(value) => (NumberOp::Lt, Some(*value)),
        NumberCondition::Le(value) => (NumberOp::Le, Some(*value)),
        NumberCondition::Gt(value) => (NumberOp::Gt, Some(*value)),
        NumberCondition::Ge(value) => (NumberOp::Ge, Some(*value)),
        NumberCondition::IsEmpty => (NumberOp::IsEmpty, None),
        NumberCondition::IsNotEmpty => (NumberOp::IsNotEmpty, None),
    };
    ConditionData::Number { op, value }
}

fn select_data(condition: &SelectCondition) -> ConditionData<'_> {
    let (op, value) = match condition {
        SelectCondition::Is(option) => (SelectOp::Is, Some(option.to_text())),
        SelectCondition::IsNot(option) => (SelectOp::IsNot, Some(option.to_text())),
        SelectCondition::IsEmpty => (SelectOp::IsEmpty, None),
        SelectCondition::IsNotEmpty => (SelectOp::IsNotEmpty, None),
    };
    ConditionData::Select { op, value }
}

fn multi_select_data(condition: &MultiSelectCondition) -> ConditionData<'_> {
    let (op, value) = match condition {
        MultiSelectCondition::Contains(option) => (MultiSelectOp::Contains, Some(option.to_text())),
        MultiSelectCondition::DoesNotContain(option) => {
            (MultiSelectOp::DoesNotContain, Some(option.to_text()))
        }
        MultiSelectCondition::IsEmpty => (MultiSelectOp::IsEmpty, None),
        MultiSelectCondition::IsNotEmpty => (MultiSelectOp::IsNotEmpty, None),
    };
    ConditionData::MultiSelect { op, value }
}

fn date_data(condition: &DateCondition) -> ConditionData<'_> {
    let (op, value) = match condition {
        DateCondition::Is(date) => (DateOp::Is, Some(date_text(date))),
        DateCondition::Before(date) => (DateOp::Before, Some(date_text(date))),
        DateCondition::After(date) => (DateOp::After, Some(date_text(date))),
        DateCondition::IsEmpty => (DateOp::IsEmpty, None),
        DateCondition::IsNotEmpty => (DateOp::IsNotEmpty, None),
    };
    ConditionData::Date { op, value }
}

impl TryFrom<ConditionData<'_>> for Condition {
    type Error = Invalid;

    fn try_from(data: ConditionData<'_>) -> Result<Condition, Invalid> {
        match data {
            ConditionData::Text { op, value } => text_condition(op, value).map(Condition::Text),
            ConditionData::Number { op, value } => {
                number_condition(op, value).map(Condition::Number)
            }
            ConditionData::Select { op, value } => {
                select_condition(op, value).map(Condition::Select)
            }
            ConditionData::MultiSelect { op, value } => {
                multi_select_condition(op, value).map(Condition::MultiSelect)
            }
            ConditionData::Date { op, value } => date_condition(op, value).map(Condition::Date),
            ConditionData::Checkbox { value, .. } => Ok(Condition::Checkbox(value)),
        }
    }
}

fn operand<T>(value: Option<T>) -> Result<T, Invalid> {
    value.ok_or_else(|| Invalid::new("condition without a value"))
}

fn text_condition(op: TextOp, value: Option<Cow<'_, str>>) -> Result<TextCondition, Invalid> {
    Ok(match op {
        TextOp::Contains => TextCondition::Contains(operand(value)?.into_owned()),
        TextOp::DoesNotContain => TextCondition::DoesNotContain(operand(value)?.into_owned()),
        TextOp::Is => TextCondition::Is(operand(value)?.into_owned()),
        TextOp::IsEmpty => TextCondition::IsEmpty,
        TextOp::IsNotEmpty => TextCondition::IsNotEmpty,
    })
}

fn number_condition(op: NumberOp, value: Option<f64>) -> Result<NumberCondition, Invalid> {
    Ok(match op {
        NumberOp::Eq => NumberCondition::Eq(operand(value)?),
        NumberOp::Ne => NumberCondition::Ne(operand(value)?),
        NumberOp::Lt => NumberCondition::Lt(operand(value)?),
        NumberOp::Le => NumberCondition::Le(operand(value)?),
        NumberOp::Gt => NumberCondition::Gt(operand(value)?),
        NumberOp::Ge => NumberCondition::Ge(operand(value)?),
        NumberOp::IsEmpty => NumberCondition::IsEmpty,
        NumberOp::IsNotEmpty => NumberCondition::IsNotEmpty,
    })
}

fn select_condition(op: SelectOp, value: Option<String>) -> Result<SelectCondition, Invalid> {
    Ok(match op {
        SelectOp::Is => SelectCondition::Is(OptionId::parse(&operand(value)?)?),
        SelectOp::IsNot => SelectCondition::IsNot(OptionId::parse(&operand(value)?)?),
        SelectOp::IsEmpty => SelectCondition::IsEmpty,
        SelectOp::IsNotEmpty => SelectCondition::IsNotEmpty,
    })
}

fn multi_select_condition(
    op: MultiSelectOp,
    value: Option<String>,
) -> Result<MultiSelectCondition, Invalid> {
    Ok(match op {
        MultiSelectOp::Contains => {
            MultiSelectCondition::Contains(OptionId::parse(&operand(value)?)?)
        }
        MultiSelectOp::DoesNotContain => {
            MultiSelectCondition::DoesNotContain(OptionId::parse(&operand(value)?)?)
        }
        MultiSelectOp::IsEmpty => MultiSelectCondition::IsEmpty,
        MultiSelectOp::IsNotEmpty => MultiSelectCondition::IsNotEmpty,
    })
}

fn date_condition(op: DateOp, value: Option<String>) -> Result<DateCondition, Invalid> {
    Ok(match op {
        DateOp::Is => DateCondition::Is(parse_date(&operand(value)?)?),
        DateOp::Before => DateCondition::Before(parse_date(&operand(value)?)?),
        DateOp::After => DateCondition::After(parse_date(&operand(value)?)?),
        DateOp::IsEmpty => DateCondition::IsEmpty,
        DateOp::IsNotEmpty => DateCondition::IsNotEmpty,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use remindr_core::domain::{
        BlockId, CalendarDate, Condition, DateCondition, Filter, MultiSelectCondition,
        NumberCondition, OptionId, PropertyId, SelectCondition, Sort, SortDirection, SortKey,
        TextCondition, View, ViewId, ViewKind,
    };
    use remindr_core::ports::StoreError;

    use super::{ViewRow, encode_view};
    use crate::codec::{AWKWARD_FLOATS, StoredId};

    fn corrupt_message(error: StoreError) -> String {
        match error {
            StoreError::Corrupt(message) => message,
            other => panic!("{other:?}"),
        }
    }

    fn view(kind: ViewKind) -> View {
        View {
            id: ViewId::new(),
            block: BlockId::new(),
            name: "Main \"table\"".to_owned(),
            sort_key: SortKey::parse("V").unwrap(),
            kind,
            filters: vec![],
            sorts: vec![],
            visible: vec![],
            column_widths: BTreeMap::new(),
            manual_order: vec![],
        }
    }

    fn row_of(view: &View) -> ViewRow {
        let (kind, config) = encode_view(view).unwrap();
        ViewRow {
            id: view.id.to_text(),
            block_id: view.block.to_text(),
            name: view.name.clone(),
            kind: kind.to_owned(),
            sort_key: view.sort_key.as_str().to_owned(),
            config,
        }
    }

    fn all_conditions() -> Vec<Condition> {
        let option = OptionId::new();
        let date = CalendarDate::new(2026, 10, 23).unwrap();
        vec![
            Condition::Text(TextCondition::Contains("a".to_owned())),
            Condition::Text(TextCondition::DoesNotContain("b".to_owned())),
            Condition::Text(TextCondition::Is("c \"q\"".to_owned())),
            Condition::Text(TextCondition::IsEmpty),
            Condition::Text(TextCondition::IsNotEmpty),
            Condition::Number(NumberCondition::Eq(1.5)),
            Condition::Number(NumberCondition::Ne(-2.0)),
            Condition::Number(NumberCondition::Lt(3.0)),
            Condition::Number(NumberCondition::Le(4.0)),
            Condition::Number(NumberCondition::Gt(5.0)),
            Condition::Number(NumberCondition::Ge(6.0)),
            Condition::Number(NumberCondition::IsEmpty),
            Condition::Number(NumberCondition::IsNotEmpty),
            Condition::Select(SelectCondition::Is(option)),
            Condition::Select(SelectCondition::IsNot(option)),
            Condition::Select(SelectCondition::IsEmpty),
            Condition::Select(SelectCondition::IsNotEmpty),
            Condition::MultiSelect(MultiSelectCondition::Contains(option)),
            Condition::MultiSelect(MultiSelectCondition::DoesNotContain(option)),
            Condition::MultiSelect(MultiSelectCondition::IsEmpty),
            Condition::MultiSelect(MultiSelectCondition::IsNotEmpty),
            Condition::Date(DateCondition::Is(date)),
            Condition::Date(DateCondition::Before(date)),
            Condition::Date(DateCondition::After(date)),
            Condition::Date(DateCondition::IsEmpty),
            Condition::Date(DateCondition::IsNotEmpty),
            Condition::Checkbox(true),
            Condition::Checkbox(false),
        ]
    }

    #[test]
    fn table_view_with_every_condition_round_trips() {
        let first = PropertyId::new();
        let second = PropertyId::new();
        let mut table = view(ViewKind::Table);
        table.filters = all_conditions()
            .into_iter()
            .map(|condition| Filter {
                property: first,
                condition,
            })
            .collect();
        table.sorts = vec![
            Sort {
                property: first,
                direction: SortDirection::Ascending,
            },
            Sort {
                property: second,
                direction: SortDirection::Descending,
            },
        ];
        table.visible = vec![second, first];
        table.column_widths = BTreeMap::from([(first, 240), (second, 96)]);
        table.manual_order = vec![BlockId::new(), BlockId::new()];
        assert_eq!(row_of(&table).into_view().unwrap(), table);
    }

    #[test]
    fn board_view_round_trips_with_its_group() {
        let group_by = PropertyId::new();
        let board = view(ViewKind::Board { group_by });
        let row = row_of(&board);
        assert_eq!(row.kind, "board");
        assert_eq!(row.into_view().unwrap(), board);
    }

    #[test]
    fn config_has_the_documented_shape() {
        let property = PropertyId::new();
        let id = property.to_text();
        let mut table = view(ViewKind::Table);
        table.filters = vec![Filter {
            property,
            condition: Condition::Text(TextCondition::Contains("x".to_owned())),
        }];
        table.sorts = vec![Sort {
            property,
            direction: SortDirection::Ascending,
        }];
        table.visible = vec![property];
        table.column_widths = BTreeMap::from([(property, 240)]);
        let (kind, config) = encode_view(&table).unwrap();
        assert_eq!(kind, "table");
        assert_eq!(
            config,
            format!(
                r#"{{"filters":[{{"property":"{id}","condition":{{"type":"text","op":"contains","value":"x"}}}}],"sorts":[{{"property":"{id}","direction":"ascending"}}],"visible":["{id}"],"column_widths":{{"{id}":240}},"manual_order":[],"group_by":null}}"#
            )
        );
    }

    #[test]
    fn empty_and_checkbox_conditions_use_null_and_bool_values() {
        let property = PropertyId::new();
        let mut table = view(ViewKind::Table);
        table.filters = vec![
            Filter {
                property,
                condition: Condition::Select(SelectCondition::IsEmpty),
            },
            Filter {
                property,
                condition: Condition::Checkbox(true),
            },
        ];
        let (_, config) = encode_view(&table).unwrap();
        assert!(
            config.contains(r#"{"type":"select","op":"is_empty","value":null}"#),
            "{config}"
        );
        assert!(
            config.contains(r#"{"type":"checkbox","op":"is","value":true}"#),
            "{config}"
        );
    }

    fn number_of(condition: &Condition) -> f64 {
        match condition {
            Condition::Number(
                NumberCondition::Eq(number)
                | NumberCondition::Ne(number)
                | NumberCondition::Lt(number)
                | NumberCondition::Le(number)
                | NumberCondition::Gt(number)
                | NumberCondition::Ge(number),
            ) => *number,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn filter_numbers_round_trip_bit_for_bit() {
        let property = PropertyId::new();
        let numbers: Vec<f64> = AWKWARD_FLOATS
            .into_iter()
            .flat_map(|number| [number, -number])
            .collect();
        let constructors: [fn(f64) -> NumberCondition; 6] = [
            NumberCondition::Eq,
            NumberCondition::Ne,
            NumberCondition::Lt,
            NumberCondition::Le,
            NumberCondition::Gt,
            NumberCondition::Ge,
        ];
        let mut table = view(ViewKind::Table);
        table.filters = numbers
            .iter()
            .flat_map(|number| {
                constructors.iter().map(|constructor| Filter {
                    property,
                    condition: Condition::Number(constructor(*number)),
                })
            })
            .collect();
        let decoded = row_of(&table).into_view().unwrap();
        assert_eq!(decoded.filters.len(), table.filters.len());
        for (sent, loaded) in table.filters.iter().zip(&decoded.filters) {
            let (sent, loaded) = (number_of(&sent.condition), number_of(&loaded.condition));
            assert_eq!(loaded.to_bits(), sent.to_bits(), "{sent:e}");
        }
    }

    #[test]
    fn non_finite_filter_numbers_are_refused() {
        let mut table = view(ViewKind::Table);
        table.filters = vec![Filter {
            property: PropertyId::new(),
            condition: Condition::Number(NumberCondition::Gt(f64::NAN)),
        }];
        assert!(matches!(encode_view(&table), Err(StoreError::Write(_))));
    }

    #[test]
    fn unknown_view_kind_is_corrupt() {
        let mut row = row_of(&view(ViewKind::Table));
        row.kind = "gallery".to_owned();
        let message = corrupt_message(row.into_view().unwrap_err());
        assert!(message.starts_with("view.kind: "), "{message}");
    }

    #[test]
    fn board_without_group_is_corrupt() {
        let mut row = row_of(&view(ViewKind::Table));
        row.kind = "board".to_owned();
        let message = corrupt_message(row.into_view().unwrap_err());
        assert!(message.starts_with("view.config: "), "{message}");
    }

    #[test]
    fn bad_config_is_corrupt() {
        let property = PropertyId::new().to_text();
        for config in [
            "nope".to_owned(),
            r#"{"filters":[]}"#.to_owned(),
            format!(
                r#"{{"filters":[{{"property":"{property}","condition":{{"type":"text","op":"is_empty_ish","value":null}}}}],"sorts":[],"visible":[],"column_widths":{{}},"manual_order":[],"group_by":null}}"#
            ),
            format!(
                r#"{{"filters":[{{"property":"{property}","condition":{{"type":"text","op":"contains","value":null}}}}],"sorts":[],"visible":[],"column_widths":{{}},"manual_order":[],"group_by":null}}"#
            ),
            r#"{"filters":[],"sorts":[{"property":"x","direction":"ascending"}],"visible":[],"column_widths":{},"manual_order":[],"group_by":null}"#.to_owned(),
            format!(
                r#"{{"filters":[],"sorts":[{{"property":"{property}","direction":"sideways"}}],"visible":[],"column_widths":{{}},"manual_order":[],"group_by":null}}"#
            ),
        ] {
            let mut row = row_of(&view(ViewKind::Table));
            row.config = config.clone();
            let message = corrupt_message(row.into_view().unwrap_err());
            assert!(message.starts_with("view.config: "), "{config}: {message}");
        }
    }

    #[test]
    fn bad_ids_are_corrupt() {
        let mut row = row_of(&view(ViewKind::Table));
        row.block_id = "nope".to_owned();
        let message = corrupt_message(row.into_view().unwrap_err());
        assert!(message.starts_with("view.block_id: "), "{message}");
    }
}
