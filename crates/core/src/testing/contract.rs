use std::collections::BTreeMap;
use std::fmt::Debug;

use crate::domain::{
    Block, BlockId, BlockKind, CalendarDate, CodeLanguage, Condition, Database, DatabaseId,
    DateCondition, Emoji, Filter, HeadingLevel, Link, Marks, MultiSelectCondition, NumberCondition,
    NumberFormat, Operation, OptionColor, OptionId, Parent, Property, PropertyId, PropertyKind,
    PropertyValue, RichText, Schema, SelectCondition, SelectOption, Sort, SortDirection, SortKey,
    Span, TextColor, TextCondition, Timestamp, Transaction, Uuid, View, ViewId, ViewKind,
    WorkspaceSnapshot,
};
use crate::ports::WorkspaceStore;

const PAGE: u32 = 0x70;
const TEXT: u32 = 0x10;
const TODO: u32 = 0x20;
const CALLOUT: u32 = 0x30;
const CODE: u32 = 0x40;
const HEADING: u32 = 0x50;
const DIVIDER: u32 = 0x60;
const ROW_ONE: u32 = 0x82;
const ROW_TWO: u32 = 0x81;
const DATABASE_VIEW: u32 = 0x90;
const DATABASE: u32 = 0xD1;
const TITLE_PROPERTY: u32 = 0xA3;
const URL_PROPERTY: u32 = 0xA8;
const TEXT_PROPERTY: u32 = 0xA1;
const NUMBER_PROPERTY: u32 = 0xA6;
const SELECT_PROPERTY: u32 = 0xA2;
const MULTI_PROPERTY: u32 = 0xA7;
const DATE_PROPERTY: u32 = 0xA4;
const CHECKBOX_PROPERTY: u32 = 0xA5;
const OPTION_TODO: u32 = 0xC3;
const OPTION_DOING: u32 = 0xC1;
const OPTION_DONE: u32 = 0xC2;
const OPTION_URGENT: u32 = 0xC5;
const OPTION_LATER: u32 = 0xC4;
const OPTION_BLOCKED: u32 = 0xC6;
const TABLE_VIEW: u32 = 0xB2;
const BOARD_VIEW: u32 = 0xB1;

pub fn store_contract<S: WorkspaceStore>(open: impl Fn() -> S) {
    let mut expected = WorkspaceSnapshot::default();
    assert_loads(&open, "fresh storage", &expected);
    insert_blocks(&open, &mut expected);
    insert_database(&open, &mut expected);
    mutate_blocks(&open, &mut expected);
    mutate_database(&open, &mut expected);
    mutate_properties(&open, &mut expected);
    mutate_views(&open, &mut expected);
    remove_everything(&open, &mut expected);
}

fn insert_blocks<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let blocks = page_blocks();
    let operations = blocks.iter().cloned().map(Operation::InsertBlock).collect();
    expected.blocks.extend(blocks);
    canonical(expected);
    check(open, "insert blocks", operations, expected);
}

fn insert_database<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let database = tasks_database();
    let rows = row_blocks();
    let values = row_values();
    let database_view = database_view_block();
    let views = [table_view(), board_view()];
    let mut operations = vec![Operation::InsertDatabase(database.clone())];
    operations.extend(rows.iter().cloned().map(Operation::InsertBlock));
    operations.extend(
        values
            .iter()
            .cloned()
            .map(|(row, property, value)| Operation::SetValue {
                row,
                property,
                value: Some(value),
            }),
    );
    operations.push(Operation::InsertBlock(database_view.clone()));
    operations.extend(views.iter().cloned().map(Operation::PutView));
    expected.databases.push(database);
    expected.blocks.extend(rows);
    expected.blocks.push(database_view);
    expected.values.extend(values);
    expected.views.extend(views);
    canonical(expected);
    check(open, "insert database", operations, expected);
}

fn mutate_blocks<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let text = RichText::plain("Edited \"text\"");
    block_mut(expected, TEXT).text = text.clone();
    let operations = vec![Operation::SetText {
        block: block_id(TEXT),
        text,
    }];
    check(open, "SetText", operations, expected);

    block_mut(expected, TODO).kind = BlockKind::Toggle;
    let operations = vec![Operation::SetKind {
        block: block_id(TODO),
        kind: BlockKind::Toggle,
    }];
    check(open, "SetKind", operations, expected);

    let parent = Parent::Block(block_id(CALLOUT));
    let sort_key = key("V");
    let divider = block_mut(expected, DIVIDER);
    divider.parent = parent;
    divider.sort_key = sort_key.clone();
    let operations = vec![Operation::Move {
        block: block_id(DIVIDER),
        parent,
        sort_key,
    }];
    check(open, "Move", operations, expected);

    block_mut(expected, HEADING).alive = false;
    let operations = vec![Operation::SetBlockAlive {
        block: block_id(HEADING),
        alive: false,
    }];
    check(open, "SetBlockAlive", operations, expected);
}

fn mutate_database<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    database_mut(expected).name = "Projects \"2026\"".to_owned();
    let operations = vec![Operation::SetDatabaseName {
        database: database_id(DATABASE),
        name: "Projects \"2026\"".to_owned(),
    }];
    check(open, "SetDatabaseName", operations, expected);

    database_mut(expected).icon = None;
    let operations = vec![Operation::SetDatabaseIcon {
        database: database_id(DATABASE),
        icon: None,
    }];
    check(open, "SetDatabaseIcon", operations, expected);

    database_mut(expected).alive = false;
    let operations = vec![Operation::SetDatabaseAlive {
        database: database_id(DATABASE),
        alive: false,
    }];
    check(open, "SetDatabaseAlive", operations, expected);
}

fn mutate_properties<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let mut options = status_options();
    options.push(option(OPTION_BLOCKED, "Blocked", OptionColor::Red));
    let status = property(SELECT_PROPERTY, "State", PropertyKind::Select { options });
    let index = schema_index(expected, SELECT_PROPERTY);
    schema_mut(expected)[index] = status.clone();
    let operations = vec![Operation::PutProperty {
        database: database_id(DATABASE),
        property: status,
        index,
    }];
    check(open, "PutProperty replace", operations, expected);

    let index = schema_index(expected, CHECKBOX_PROPERTY);
    let checkbox = schema_mut(expected).remove(index);
    schema_mut(expected).insert(0, checkbox.clone());
    let operations = vec![Operation::PutProperty {
        database: database_id(DATABASE),
        property: checkbox,
        index: 0,
    }];
    check(open, "PutProperty move to index 0", operations, expected);

    expected
        .values
        .retain(|(_, property, _)| *property != property_id(URL_PROPERTY));
    let operations = vec![Operation::SetValue {
        row: block_id(ROW_ONE),
        property: property_id(URL_PROPERTY),
        value: None,
    }];
    check(open, "SetValue none", operations, expected);

    let index = schema_index(expected, URL_PROPERTY);
    schema_mut(expected).remove(index);
    let operations = vec![Operation::RemoveProperty {
        database: database_id(DATABASE),
        property: property_id(URL_PROPERTY),
    }];
    check(open, "RemoveProperty", operations, expected);
}

fn mutate_views<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let updated = View {
        name: "All tasks".to_owned(),
        kind: ViewKind::Board {
            group_by: property_id(SELECT_PROPERTY),
        },
        filters: vec![Filter {
            property: property_id(CHECKBOX_PROPERTY),
            condition: Condition::Checkbox(true),
        }],
        sorts: vec![],
        visible: vec![property_id(TITLE_PROPERTY)],
        column_widths: BTreeMap::from([(property_id(TITLE_PROPERTY), 300)]),
        manual_order: vec![block_id(ROW_ONE)],
        ..table_view()
    };
    *view_mut(expected, TABLE_VIEW) = updated.clone();
    check(
        open,
        "PutView update",
        vec![Operation::PutView(updated)],
        expected,
    );

    expected.views.retain(|view| view.id != view_id(BOARD_VIEW));
    let operations = vec![Operation::RemoveView {
        view: view_id(BOARD_VIEW),
    }];
    check(open, "RemoveView", operations, expected);
}

fn remove_everything<S: WorkspaceStore>(open: &impl Fn() -> S, expected: &mut WorkspaceSnapshot) {
    let operations = expected
        .values
        .drain(..)
        .map(|(row, property, _)| Operation::SetValue {
            row,
            property,
            value: None,
        })
        .collect();
    check(open, "clear remaining values", operations, expected);

    expected.views.clear();
    let operations = vec![Operation::RemoveView {
        view: view_id(TABLE_VIEW),
    }];
    check(open, "RemoveView last", operations, expected);

    let removed = [ROW_ONE, ROW_TWO, DATABASE_VIEW, DIVIDER];
    expected
        .blocks
        .retain(|block| !removed.iter().any(|id| block.id == block_id(*id)));
    let operations = removed
        .iter()
        .map(|id| Operation::RemoveBlock {
            block: block_id(*id),
        })
        .collect();
    check(open, "RemoveBlock", operations, expected);

    expected.databases.clear();
    let operations = vec![Operation::RemoveDatabase {
        database: database_id(DATABASE),
    }];
    check(open, "RemoveDatabase", operations, expected);
}

fn check<S: WorkspaceStore>(
    open: &impl Fn() -> S,
    step: &str,
    operations: Vec<Operation>,
    expected: &WorkspaceSnapshot,
) {
    let mut store = open();
    must(store.commit(&Transaction::new(operations)), step);
    drop(store);
    assert_loads(open, step, expected);
}

fn assert_loads<S: WorkspaceStore>(
    open: &impl Fn() -> S,
    step: &str,
    expected: &WorkspaceSnapshot,
) {
    let loaded = must(open().load(), step);
    assert_eq!(&loaded, expected, "{step}");
}

fn must<T, E: Debug>(result: Result<T, E>, step: &str) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("{step}: {error:?}"),
    }
}

fn found<T>(item: Option<T>, what: &str) -> T {
    match item {
        Some(item) => item,
        None => panic!("missing {what}"),
    }
}

fn canonical(snapshot: &mut WorkspaceSnapshot) {
    snapshot.blocks.sort_by_key(|block| block.id);
    snapshot.databases.sort_by_key(|database| database.id);
    snapshot
        .values
        .sort_by_key(|(row, property, _)| (*row, *property));
    snapshot.views.sort_by_key(|view| view.id);
}

fn block_mut(snapshot: &mut WorkspaceSnapshot, id: u32) -> &mut Block {
    found(
        snapshot
            .blocks
            .iter_mut()
            .find(|block| block.id == block_id(id)),
        "block",
    )
}

fn database_mut(snapshot: &mut WorkspaceSnapshot) -> &mut Database {
    found(snapshot.databases.first_mut(), "database")
}

fn schema_mut(snapshot: &mut WorkspaceSnapshot) -> &mut Vec<Property> {
    &mut database_mut(snapshot).schema.properties
}

fn schema_index(snapshot: &mut WorkspaceSnapshot, id: u32) -> usize {
    found(
        schema_mut(snapshot)
            .iter()
            .position(|property| property.id == property_id(id)),
        "property",
    )
}

fn view_mut(snapshot: &mut WorkspaceSnapshot, id: u32) -> &mut View {
    found(
        snapshot
            .views
            .iter_mut()
            .find(|view| view.id == view_id(id)),
        "view",
    )
}

fn uuid(n: u32) -> Uuid {
    Uuid::from_u128(u128::from(n))
}

fn block_id(n: u32) -> BlockId {
    BlockId::from_uuid(uuid(n))
}

fn database_id(n: u32) -> DatabaseId {
    DatabaseId::from_uuid(uuid(n))
}

fn property_id(n: u32) -> PropertyId {
    PropertyId::from_uuid(uuid(n))
}

fn option_id(n: u32) -> OptionId {
    OptionId::from_uuid(uuid(n))
}

fn view_id(n: u32) -> ViewId {
    ViewId::from_uuid(uuid(n))
}

fn created(n: u32) -> Timestamp {
    Timestamp::from_millis(1_760_000_000_000 + i64::from(n))
}

fn key(raw: &str) -> SortKey {
    must(SortKey::parse(raw), "sort key")
}

fn emoji(raw: &str) -> Emoji {
    must(Emoji::try_from(raw), "emoji")
}

fn date(year: i32, month: u8, day: u8) -> CalendarDate {
    must(CalendarDate::new(year, month, day), "date")
}

fn block(id: u32, kind: BlockKind, parent: Parent, sort_key: &str, text: RichText) -> Block {
    Block {
        id: block_id(id),
        kind,
        parent,
        sort_key: key(sort_key),
        text,
        alive: true,
        created_at: created(id),
    }
}

fn span(text: &str, marks: Marks) -> Span {
    Span {
        text: text.to_owned(),
        marks,
    }
}

fn marked_text() -> RichText {
    let link = must(Link::try_from("https://example.com/a?b=1"), "link");
    RichText::new([
        span("Hello ", Marks::default()),
        span(
            "bold link",
            Marks {
                bold: true,
                link: Some(link),
                color: Some(TextColor::Blue),
                ..Marks::default()
            },
        ),
        span(" and ", Marks::default()),
        span(
            "everything \"quoted\"",
            Marks {
                bold: true,
                italic: true,
                underline: true,
                strikethrough: true,
                code: true,
                link: None,
                color: Some(TextColor::RedBackground),
            },
        ),
    ])
}

fn page_blocks() -> Vec<Block> {
    let page = Parent::Block(block_id(PAGE));
    vec![
        block(
            PAGE,
            BlockKind::Page {
                icon: Some(emoji("📄")),
            },
            Parent::Root,
            "V",
            RichText::plain("Home"),
        ),
        block(TEXT, BlockKind::Text, page, "V", marked_text()),
        block(
            TODO,
            BlockKind::ToDo { checked: true },
            page,
            "a",
            RichText::plain("Ship it"),
        ),
        block(
            CALLOUT,
            BlockKind::Callout {
                icon: emoji("💡")
            },
            page,
            "k",
            RichText::plain("Remember"),
        ),
        block(
            CODE,
            BlockKind::Code {
                language: CodeLanguage::new("rust"),
            },
            page,
            "t",
            RichText::plain("fn main() {}"),
        ),
        block(
            HEADING,
            BlockKind::Heading(HeadingLevel::Two),
            page,
            "w",
            RichText::plain("Plan"),
        ),
        block(DIVIDER, BlockKind::Divider, page, "y", RichText::default()),
    ]
}

fn row_blocks() -> Vec<Block> {
    let rows = Parent::Database(database_id(DATABASE));
    vec![
        block(
            ROW_ONE,
            BlockKind::Page { icon: None },
            rows,
            "V",
            RichText::plain("Write the spec"),
        ),
        block(
            ROW_TWO,
            BlockKind::Page {
                icon: Some(emoji("🔥")),
            },
            rows,
            "a",
            RichText::plain("Review"),
        ),
    ]
}

fn database_view_block() -> Block {
    block(
        DATABASE_VIEW,
        BlockKind::DatabaseView {
            database: database_id(DATABASE),
        },
        Parent::Block(block_id(PAGE)),
        "z",
        RichText::default(),
    )
}

fn property(id: u32, name: &str, kind: PropertyKind) -> Property {
    Property {
        id: property_id(id),
        name: name.to_owned(),
        kind,
    }
}

fn option(id: u32, name: &str, color: OptionColor) -> SelectOption {
    SelectOption {
        id: option_id(id),
        name: name.to_owned(),
        color,
    }
}

fn status_options() -> Vec<SelectOption> {
    vec![
        option(OPTION_TODO, "Todo", OptionColor::Gray),
        option(OPTION_DOING, "Doing", OptionColor::Yellow),
        option(OPTION_DONE, "Done", OptionColor::Green),
    ]
}

fn tag_options() -> Vec<SelectOption> {
    vec![
        option(OPTION_URGENT, "Urgent", OptionColor::Red),
        option(OPTION_LATER, "Later", OptionColor::Default),
    ]
}

fn tasks_database() -> Database {
    Database {
        id: database_id(DATABASE),
        name: "Tasks".to_owned(),
        icon: Some(emoji("✅")),
        schema: Schema {
            properties: vec![
                property(TITLE_PROPERTY, "Name", PropertyKind::Title),
                property(URL_PROPERTY, "Link", PropertyKind::Url),
                property(TEXT_PROPERTY, "Notes", PropertyKind::Text),
                property(
                    NUMBER_PROPERTY,
                    "Cost",
                    PropertyKind::Number {
                        format: NumberFormat::Euro,
                    },
                ),
                property(
                    SELECT_PROPERTY,
                    "Status",
                    PropertyKind::Select {
                        options: status_options(),
                    },
                ),
                property(
                    MULTI_PROPERTY,
                    "Tags",
                    PropertyKind::MultiSelect {
                        options: tag_options(),
                    },
                ),
                property(DATE_PROPERTY, "Due", PropertyKind::Date),
                property(CHECKBOX_PROPERTY, "Done", PropertyKind::Checkbox),
            ],
        },
        sort_key: key("V"),
        alive: true,
        created_at: created(DATABASE),
    }
}

fn row_values() -> Vec<(BlockId, PropertyId, PropertyValue)> {
    let row = block_id(ROW_ONE);
    vec![
        (
            row,
            property_id(URL_PROPERTY),
            PropertyValue::Url("https://example.com/spec".to_owned()),
        ),
        (
            row,
            property_id(TEXT_PROPERTY),
            PropertyValue::Text("Draft \"v1\" é".to_owned()),
        ),
        (
            row,
            property_id(NUMBER_PROPERTY),
            PropertyValue::Number(12.5),
        ),
        (
            row,
            property_id(SELECT_PROPERTY),
            PropertyValue::Select(option_id(OPTION_DOING)),
        ),
        (
            row,
            property_id(MULTI_PROPERTY),
            PropertyValue::MultiSelect(vec![option_id(OPTION_URGENT), option_id(OPTION_LATER)]),
        ),
        (
            row,
            property_id(DATE_PROPERTY),
            PropertyValue::Date(date(2026, 10, 23)),
        ),
        (
            row,
            property_id(CHECKBOX_PROPERTY),
            PropertyValue::Checkbox(true),
        ),
    ]
}

fn filter(property: u32, condition: Condition) -> Filter {
    Filter {
        property: property_id(property),
        condition,
    }
}

fn table_view() -> View {
    View {
        id: view_id(TABLE_VIEW),
        block: block_id(DATABASE_VIEW),
        name: "Table \"main\"".to_owned(),
        sort_key: key("V"),
        kind: ViewKind::Table,
        filters: vec![
            filter(TITLE_PROPERTY, Condition::Text(TextCondition::IsNotEmpty)),
            filter(
                TEXT_PROPERTY,
                Condition::Text(TextCondition::Contains("spec".to_owned())),
            ),
            filter(NUMBER_PROPERTY, Condition::Number(NumberCondition::Gt(3.0))),
            filter(
                SELECT_PROPERTY,
                Condition::Select(SelectCondition::Is(option_id(OPTION_DOING))),
            ),
            filter(
                MULTI_PROPERTY,
                Condition::MultiSelect(MultiSelectCondition::Contains(option_id(OPTION_URGENT))),
            ),
            filter(
                DATE_PROPERTY,
                Condition::Date(DateCondition::Before(date(2026, 12, 31))),
            ),
            filter(CHECKBOX_PROPERTY, Condition::Checkbox(true)),
        ],
        sorts: vec![
            Sort {
                property: property_id(NUMBER_PROPERTY),
                direction: SortDirection::Descending,
            },
            Sort {
                property: property_id(TITLE_PROPERTY),
                direction: SortDirection::Ascending,
            },
        ],
        visible: vec![
            property_id(TITLE_PROPERTY),
            property_id(NUMBER_PROPERTY),
            property_id(SELECT_PROPERTY),
        ],
        column_widths: BTreeMap::from([
            (property_id(TITLE_PROPERTY), 280),
            (property_id(NUMBER_PROPERTY), 96),
        ]),
        manual_order: vec![block_id(ROW_TWO), block_id(ROW_ONE)],
    }
}

fn board_view() -> View {
    View {
        id: view_id(BOARD_VIEW),
        block: block_id(DATABASE_VIEW),
        name: "Board".to_owned(),
        sort_key: key("a"),
        kind: ViewKind::Board {
            group_by: property_id(SELECT_PROPERTY),
        },
        filters: vec![],
        sorts: vec![],
        visible: vec![property_id(TITLE_PROPERTY)],
        column_widths: BTreeMap::new(),
        manual_order: vec![],
    }
}

#[cfg(test)]
mod tests {
    use crate::testing::MemoryStore;

    use super::store_contract;

    #[test]
    fn memory_store_honours_the_contract() {
        let store = MemoryStore::new();
        store_contract(|| store.shared());
    }
}
