use std::collections::BTreeMap;

use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use remindr_core::application::{Command, Created, DropTarget};
use remindr_core::domain::{
    BlockId, BlockKind, CalendarDate, Condition, DatabaseId, Filter, NumberFormat, OptionColor,
    OptionId, Parent, Property, PropertyId, PropertyKind, PropertyValue, RichText, SelectCondition,
    SelectOption, Sort, SortDirection, ViewId, ViewKind,
};

use crate::{RemindrWorld, run_feature};

fn rows_of(step: &Step) -> &[Vec<String>] {
    &step.table.as_ref().expect("step has a table").rows
}

fn records(step: &Step) -> Vec<BTreeMap<&str, &str>> {
    let (header, rows) = rows_of(step).split_first().expect("table has a header");
    rows.iter()
        .map(|row| {
            header
                .iter()
                .zip(row)
                .map(|(name, cell)| (name.as_str(), cell.as_str()))
                .collect()
        })
        .collect()
}

fn first_column(step: &Step) -> Vec<&str> {
    rows_of(step)
        .iter()
        .map(|row| row.first().map_or("", String::as_str))
        .collect()
}

fn run_ok(world: &mut RemindrWorld, command: Command) {
    world.run(command);
    world.changes();
}

fn kind_named(name: &str) -> PropertyKind {
    match name {
        "title" => PropertyKind::Title,
        "text" => PropertyKind::Text,
        "number" => PropertyKind::Number {
            format: NumberFormat::Plain,
        },
        "select" => PropertyKind::Select {
            options: Vec::new(),
        },
        "multi-select" => PropertyKind::MultiSelect {
            options: Vec::new(),
        },
        "date" => PropertyKind::Date,
        "checkbox" => PropertyKind::Checkbox,
        "url" => PropertyKind::Url,
        other => panic!("unknown property kind {other}"),
    }
}

fn kind_label(kind: &PropertyKind) -> &'static str {
    match kind {
        PropertyKind::Title => "title",
        PropertyKind::Text => "text",
        PropertyKind::Number { .. } => "number",
        PropertyKind::Select { .. } => "select",
        PropertyKind::MultiSelect { .. } => "multi-select",
        PropertyKind::Date => "date",
        PropertyKind::Checkbox => "checkbox",
        PropertyKind::Url => "url",
    }
}

fn properties_of(world: &RemindrWorld, database: DatabaseId) -> Vec<Property> {
    world
        .session
        .workspace()
        .database(database)
        .expect("database exists")
        .schema
        .properties
        .clone()
}

fn register_properties(world: &mut RemindrWorld, database: DatabaseId) {
    for property in properties_of(world, database) {
        world.properties.insert(property.name, property.id);
    }
}

fn find_property(world: &RemindrWorld, property: PropertyId) -> Option<(DatabaseId, Property)> {
    world.databases.values().find_map(|database| {
        properties_of(world, *database)
            .into_iter()
            .find(|candidate| candidate.id == property)
            .map(|found| (*database, found))
    })
}

fn property_definition(world: &RemindrWorld, property: PropertyId) -> (DatabaseId, Property) {
    find_property(world, property).expect("property belongs to a registered database")
}

fn options_of(kind: &PropertyKind) -> &[SelectOption] {
    match kind {
        PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => options,
        _ => &[],
    }
}

fn option_in(kind: &PropertyKind, name: &str) -> OptionId {
    options_of(kind)
        .iter()
        .find(|option| option.name == name)
        .unwrap_or_else(|| panic!("unknown option {name}"))
        .id
}

fn parse_date(raw: &str) -> CalendarDate {
    let parts: Vec<&str> = raw.split('-').collect();
    let [year, month, day] = parts[..] else {
        panic!("date {raw} is not YYYY-MM-DD");
    };
    CalendarDate::new(
        year.parse().expect("year"),
        month.parse().expect("month"),
        day.parse().expect("day"),
    )
    .expect("valid date")
}

fn parse_value(world: &RemindrWorld, property: PropertyId, raw: &str) -> Option<PropertyValue> {
    if raw.is_empty() {
        return None;
    }
    let (_, definition) = property_definition(world, property);
    let kind = definition.kind;
    Some(match &kind {
        PropertyKind::Title => panic!("the title is the row text"),
        PropertyKind::Text => PropertyValue::Text(raw.to_owned()),
        PropertyKind::Url => PropertyValue::Url(raw.to_owned()),
        PropertyKind::Number { .. } => PropertyValue::Number(raw.parse().expect("a number")),
        PropertyKind::Select { .. } => PropertyValue::Select(option_in(&kind, raw)),
        PropertyKind::MultiSelect { .. } => PropertyValue::MultiSelect(
            raw.split(',')
                .map(|name| option_in(&kind, name.trim()))
                .collect(),
        ),
        PropertyKind::Date => PropertyValue::Date(parse_date(raw)),
        PropertyKind::Checkbox => PropertyValue::Checkbox(raw == "true"),
    })
}

fn display_value(kind: &PropertyKind, value: &PropertyValue) -> String {
    match value {
        PropertyValue::Text(text) | PropertyValue::Url(text) => text.clone(),
        PropertyValue::Number(number) => format!("{number}"),
        PropertyValue::Select(option) => option_name(kind, *option),
        PropertyValue::MultiSelect(options) => options
            .iter()
            .map(|option| option_name(kind, *option))
            .collect::<Vec<_>>()
            .join(", "),
        PropertyValue::Date(date) => {
            format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
        }
        PropertyValue::Checkbox(flag) => flag.to_string(),
    }
}

fn option_name(kind: &PropertyKind, option: OptionId) -> String {
    options_of(kind)
        .iter()
        .find(|candidate| candidate.id == option)
        .map(|candidate| candidate.name.clone())
        .expect("option exists")
}

fn current_value(world: &RemindrWorld, row: &str, property: &str) -> Option<String> {
    let property = world.property(property);
    let (_, definition) = find_property(world, property)?;
    world
        .session
        .workspace()
        .value(world.block(row), property)
        .map(|value| display_value(&definition.kind, value))
}

fn created_ids(world: &RemindrWorld) -> (Option<DatabaseId>, Option<BlockId>, Option<ViewId>) {
    let mut ids = (None, None, None);
    for created in &world.changes().created {
        match created {
            Created::Database(id) => ids.0 = Some(*id),
            Created::Block(id) => ids.1 = Some(*id),
            Created::View(id) => ids.2 = Some(*id),
        }
    }
    ids
}

fn rename_created_database(world: &mut RemindrWorld, database: DatabaseId, name: &str) {
    run_ok(
        world,
        Command::RenameDatabase {
            database,
            name: name.to_owned(),
        },
    );
    world.databases.insert(name.to_owned(), database);
    register_properties(world, database);
}

fn view_rows(world: &RemindrWorld, view: &str) -> Vec<String> {
    let workspace = world.session.workspace();
    workspace
        .query(world.view(view))
        .expect("view exists")
        .into_iter()
        .map(|row| workspace.block(row).expect("row exists").text.to_plain())
        .collect()
}

fn titles(world: &RemindrWorld, rows: &[BlockId]) -> String {
    rows.iter()
        .map(|row| {
            world
                .session
                .workspace()
                .block(*row)
                .expect("row exists")
                .text
                .to_plain()
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn add_row_with_title(world: &mut RemindrWorld, database: DatabaseId, title: &str) -> BlockId {
    run_ok(
        world,
        Command::AddRow {
            database,
            values: Vec::new(),
        },
    );
    let Some(Created::Block(row)) = world.changes().created.first().copied() else {
        panic!("no row was created");
    };
    world.run(Command::EditText {
        block: row,
        text: RichText::plain(title),
    });
    world.blocks.insert(title.to_owned(), row);
    row
}

fn anchor_in_page(world: &mut RemindrWorld, page: &str) -> BlockId {
    let page_id = world.block(page);
    let last = world
        .session
        .workspace()
        .children(Parent::Block(page_id))
        .last()
        .map(|block| block.id);
    if let Some(last) = last {
        return last;
    }
    run_ok(
        world,
        Command::InsertBlockAt {
            target: DropTarget {
                parent: Parent::Block(page_id),
                after: None,
            },
            kind: BlockKind::Text,
        },
    );
    match world.changes().created.first() {
        Some(Created::Block(anchor)) => *anchor,
        other => panic!("no block was created: {other:?}"),
    }
}

#[given(expr = "a database {string} shown in the page {string} as {string}")]
fn database_shown(world: &mut RemindrWorld, name: String, page: String, block: String) {
    let after = anchor_in_page(world, &page);
    run_ok(world, Command::CreateDatabaseWithView { after });
    let (Some(database), Some(embed), Some(view)) = created_ids(world) else {
        panic!("database, block and view were created");
    };
    rename_created_database(world, database, &name);
    world.blocks.insert(block.clone(), embed);
    world.views.insert(block, view);
}

#[given(expr = "the database {string} has the properties:")]
fn database_has_properties(world: &mut RemindrWorld, step: &Step, database: String) {
    let id = world.database(&database);
    for record in records(step) {
        run_ok(
            world,
            Command::AddProperty {
                database: id,
                name: record["name"].to_owned(),
                kind: kind_named(record["kind"]),
            },
        );
    }
    register_properties(world, id);
}

#[given(expr = "the property {string} has the options {string}")]
fn property_has_options(world: &mut RemindrWorld, property: String, names: String) {
    let id = world.property(&property);
    let (database, _) = property_definition(world, id);
    for name in names.split(',') {
        run_ok(
            world,
            Command::AddSelectOption {
                database,
                property: id,
                name: name.trim().to_owned(),
                color: OptionColor::Default,
            },
        );
    }
    let (_, definition) = property_definition(world, id);
    for option in options_of(&definition.kind) {
        world.options.insert(option.name.clone(), option.id);
    }
}

#[given(expr = "the database {string} has the rows:")]
fn database_has_rows(world: &mut RemindrWorld, step: &Step, database: String) {
    let id = world.database(&database);
    for record in records(step) {
        let row = add_row_with_title(world, id, record["name"]);
        for (property, raw) in &record {
            if *property == "name" {
                continue;
            }
            let property = world.property(property);
            if let Some(value) = parse_value(world, property, raw) {
                run_ok(
                    world,
                    Command::SetValue {
                        row,
                        property,
                        value: Some(value),
                    },
                );
            }
        }
    }
}

#[given(expr = "a board view {string} grouped by {string} in the block {string}")]
fn board_view(world: &mut RemindrWorld, name: String, property: String, block: String) {
    let command = Command::AddView {
        block: world.block(&block),
        name: name.clone(),
        kind: ViewKind::Board {
            group_by: world.property(&property),
        },
    };
    run_ok(world, command);
    let (_, _, Some(view)) = created_ids(world) else {
        panic!("no view was created");
    };
    world.views.insert(name, view);
}

#[given(expr = "a table view {string} in the block {string}")]
fn table_view(world: &mut RemindrWorld, name: String, block: String) {
    let command = Command::AddView {
        block: world.block(&block),
        name: name.clone(),
        kind: ViewKind::Table,
    };
    run_ok(world, command);
    let (_, _, Some(view)) = created_ids(world) else {
        panic!("no view was created");
    };
    world.views.insert(name, view);
}

#[given(expr = "I insert another view block of the database {string} after {string} as {string}")]
#[when(expr = "I insert another view block of the database {string} after {string} as {string}")]
fn another_view_block(world: &mut RemindrWorld, database: String, after: String, name: String) {
    let command = Command::InsertDatabaseView {
        after: world.block(&after),
        database: world.database(&database),
    };
    world.run(command);
    let (_, Some(block), Some(view)) = created_ids(world) else {
        panic!("block and view were created");
    };
    world.blocks.insert(name.clone(), block);
    world.views.insert(name, view);
}

#[given(expr = "I filter the view {string} to rows whose {string} is {string}")]
#[when(expr = "I filter the view {string} to rows whose {string} is {string}")]
fn filter_by_option(world: &mut RemindrWorld, view: String, property: String, option: String) {
    let property = world.property(&property);
    let (_, definition) = property_definition(world, property);
    let filter = Filter {
        property,
        condition: Condition::Select(SelectCondition::Is(option_in(&definition.kind, &option))),
    };
    world.run(Command::SetFilters {
        view: world.view(&view),
        filters: vec![filter],
    });
}

#[given(expr = "I sort the view {string} by:")]
#[when(expr = "I sort the view {string} by:")]
fn sort_view(world: &mut RemindrWorld, step: &Step, view: String) {
    let sorts = records(step)
        .into_iter()
        .map(|record| Sort {
            property: world.property(record["property"]),
            direction: match record["direction"] {
                "ascending" => SortDirection::Ascending,
                "descending" => SortDirection::Descending,
                other => panic!("unknown direction {other}"),
            },
        })
        .collect();
    world.run(Command::SetSorts {
        view: world.view(&view),
        sorts,
    });
}

#[when("I create a database from the sidebar")]
fn create_database(world: &mut RemindrWorld) {
    world.run(Command::CreateDatabase);
    let (Some(database), _, _) = created_ids(world) else {
        panic!("no database was created");
    };
    world.databases.insert("Untitled".to_owned(), database);
    register_properties(world, database);
}

#[when(expr = "I insert a new database in the page {string} as {string}")]
fn insert_database(world: &mut RemindrWorld, page: String, block: String) {
    let after = anchor_in_page(world, &page);
    world.run(Command::CreateDatabaseWithView { after });
    let (Some(database), Some(embed), Some(view)) = created_ids(world) else {
        panic!("database, block and view were created");
    };
    world.databases.insert("Untitled".to_owned(), database);
    register_properties(world, database);
    world.blocks.insert(block.clone(), embed);
    world.views.insert(block, view);
}

#[when(expr = "I create a page under the database {string}")]
fn create_page_under_database(world: &mut RemindrWorld, database: String) {
    world.run(Command::CreatePage {
        parent: Parent::Database(world.database(&database)),
        after: None,
    });
}

#[when(expr = "I add a row {string} to the database {string}")]
fn add_row(world: &mut RemindrWorld, title: String, database: String) {
    let id = world.database(&database);
    add_row_with_title(world, id, &title);
}

#[when(expr = "I try to add a row to the database {string}")]
fn try_add_row(world: &mut RemindrWorld, database: String) {
    world.run(Command::AddRow {
        database: world.database(&database),
        values: Vec::new(),
    });
}

#[when(expr = "I set the values of the row {string}:")]
fn set_values(world: &mut RemindrWorld, step: &Step, row: String) {
    for record in records(step) {
        let property = world.property(record["property"]);
        let value = parse_value(world, property, record["value"]);
        let command = Command::SetValue {
            row: world.block(&row),
            property,
            value,
        };
        run_ok(world, command);
    }
}

#[when(expr = "I set the value {string} on the property {string} of the row {string}")]
fn set_value(world: &mut RemindrWorld, raw: String, property: String, row: String) {
    let property = world.property(&property);
    let value = parse_value(world, property, &raw);
    world.run(Command::SetValue {
        row: world.block(&row),
        property,
        value,
    });
}

#[when(expr = "I set the text value {string} on the property {string} of the row {string}")]
fn set_raw_text_value(world: &mut RemindrWorld, text: String, property: String, row: String) {
    world.run(Command::SetValue {
        row: world.block(&row),
        property: world.property(&property),
        value: Some(PropertyValue::Text(text)),
    });
}

#[when(expr = "I remove the option {string} from the property {string}")]
fn remove_option(world: &mut RemindrWorld, option: String, property: String) {
    let property = world.property(&property);
    let (database, definition) = property_definition(world, property);
    world.run(Command::RemoveSelectOption {
        database,
        property,
        option: option_in(&definition.kind, &option),
    });
}

#[when(expr = "I remove the property {string} from the database {string}")]
fn remove_property(world: &mut RemindrWorld, property: String, database: String) {
    world.run(Command::RemoveProperty {
        database: world.database(&database),
        property: world.property(&property),
    });
}

#[when(expr = "I change the type of the property {string} to {word}")]
fn change_type(world: &mut RemindrWorld, property: String, kind: String) {
    let property = world.property(&property);
    let (database, _) = property_definition(world, property);
    world.run(Command::ChangePropertyKind {
        database,
        property,
        kind: kind_named(&kind),
    });
}

#[when(expr = "I move the row {string} before {string} in the view {string}")]
fn move_row(world: &mut RemindrWorld, row: String, before: String, view: String) {
    world.run(Command::MoveRow {
        view: world.view(&view),
        row: world.block(&row),
        before: Some(world.block(&before)),
    });
}

#[when(expr = "I move the card {string} to the group {string} of the board {string}")]
fn move_card(world: &mut RemindrWorld, row: String, group: String, board: String) {
    let view = world.view(&board);
    let ViewKind::Board { group_by } = world
        .session
        .workspace()
        .view(view)
        .expect("view exists")
        .kind
    else {
        panic!("{board} is not a board");
    };
    let (_, definition) = property_definition(world, group_by);
    world.run(Command::MoveCard {
        view,
        row: world.block(&row),
        group: Some(option_in(&definition.kind, &group)),
        before: None,
    });
}

#[when(expr = "I delete the view block {string}")]
fn delete_view_block(world: &mut RemindrWorld, block: String) {
    world.run(Command::DeleteBlocks {
        blocks: vec![world.block(&block)],
    });
}

#[when(expr = "I delete the database {string}")]
fn delete_database(world: &mut RemindrWorld, database: String) {
    world.run(Command::DeleteDatabase {
        database: world.database(&database),
    });
}

#[when(expr = "I remove the view {string}")]
fn remove_view(world: &mut RemindrWorld, view: String) {
    world.run(Command::RemoveView {
        view: world.view(&view),
    });
}

#[then(expr = "the database {string} is listed among the databases")]
fn database_is_listed(world: &mut RemindrWorld, name: String) {
    let id = world.database(&name);
    let listed = world.session.workspace().databases().any(|d| d.id == id);
    assert!(listed, "{name} is not listed");
}

#[then(expr = "the database {string} is not listed among the databases")]
fn database_is_not_listed(world: &mut RemindrWorld, name: String) {
    let id = world.database(&name);
    let listed = world.session.workspace().databases().any(|d| d.id == id);
    assert!(!listed, "{name} is listed");
}

#[then(expr = "the database {string} has the properties:")]
fn database_has_exactly_properties(world: &mut RemindrWorld, step: &Step, name: String) {
    let actual: Vec<(String, String)> = properties_of(world, world.database(&name))
        .into_iter()
        .map(|property| (property.name, kind_label(&property.kind).to_owned()))
        .collect();
    let expected: Vec<(String, String)> = records(step)
        .into_iter()
        .map(|record| (record["name"].to_owned(), record["kind"].to_owned()))
        .collect();
    assert_eq!(actual, expected);
}

#[then(expr = "the page {string} contains the database view block {string}")]
fn page_contains_view_block(world: &mut RemindrWorld, page: String, block: String) {
    let parent = Parent::Block(world.block(&page));
    let id = world.block(&block);
    let found = world
        .session
        .workspace()
        .children(parent)
        .any(|child| child.id == id && matches!(child.kind, BlockKind::DatabaseView { .. }));
    assert!(found, "{block} is not a database view block of {page}");
}

fn shown_database(world: &RemindrWorld, block: &str) -> (DatabaseId, bool) {
    let block = world
        .session
        .workspace()
        .block(world.block(block))
        .expect("block exists");
    let BlockKind::DatabaseView { database } = block.kind else {
        panic!("not a database view block");
    };
    let alive = world
        .session
        .workspace()
        .database(database)
        .is_some_and(|record| record.alive);
    (database, alive)
}

#[then(expr = "the block {string} shows the database {string}")]
fn block_shows_database(world: &mut RemindrWorld, block: String, database: String) {
    let (shown, alive) = shown_database(world, &block);
    assert_eq!(shown, world.database(&database));
    assert!(alive, "the database is not alive");
}

#[then(expr = "the block {string} shows a placeholder")]
fn block_shows_placeholder(world: &mut RemindrWorld, block: String) {
    let (_, alive) = shown_database(world, &block);
    assert!(!alive, "the database is still alive");
}

#[then(expr = "the block {string} has exactly one view named {string} of kind {word}")]
fn block_has_one_view(world: &mut RemindrWorld, block: String, name: String, kind: String) {
    let views: Vec<_> = world
        .session
        .workspace()
        .views(world.block(&block))
        .collect();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].name, name);
    let label = match views[0].kind {
        ViewKind::Table => "table",
        ViewKind::Board { .. } => "board",
    };
    assert_eq!(label, kind);
}

#[then(expr = "the rows of the database {string} read:")]
fn rows_read(world: &mut RemindrWorld, step: &Step, database: String) {
    let actual: Vec<String> = world
        .session
        .workspace()
        .rows(world.database(&database))
        .map(|row| row.text.to_plain())
        .collect();
    assert_eq!(actual, first_column(step));
}

#[then(expr = "the row {string} has the values:")]
fn row_has_values(world: &mut RemindrWorld, step: &Step, row: String) {
    for record in records(step) {
        let actual = current_value(world, &row, record["property"]);
        assert_eq!(
            actual.as_deref(),
            Some(record["value"]),
            "{}",
            record["property"]
        );
    }
}

#[then(expr = "the row {string} has the value {string} for {string}")]
fn row_has_value(world: &mut RemindrWorld, row: String, expected: String, property: String) {
    assert_eq!(
        current_value(world, &row, &property).as_deref(),
        Some(expected.as_str())
    );
}

#[then(expr = "the row {string} has no value for {string}")]
fn row_has_no_value(world: &mut RemindrWorld, row: String, property: String) {
    assert_eq!(current_value(world, &row, &property), None);
}

#[then(expr = "the row {string} has no stored value for {string}")]
fn row_has_no_stored_value(world: &mut RemindrWorld, row: String, property: String) {
    let stored = world
        .session
        .workspace()
        .value(world.block(&row), world.property(&property));
    assert_eq!(stored, None);
}

#[then(expr = "the property {string} has the options {string}")]
fn property_options_are(world: &mut RemindrWorld, property: String, names: String) {
    let (_, definition) = property_definition(world, world.property(&property));
    let actual: Vec<&str> = options_of(&definition.kind)
        .iter()
        .map(|option| option.name.as_str())
        .collect();
    let expected: Vec<&str> = names.split(',').map(str::trim).collect();
    assert_eq!(actual, expected);
}

#[then(expr = "the property {string} is a {word} property")]
fn property_is_a_kind(world: &mut RemindrWorld, property: String, kind: String) {
    let (_, definition) = property_definition(world, world.property(&property));
    assert_eq!(kind_label(&definition.kind), kind);
}

#[then(expr = "the view {string} shows the rows:")]
fn view_shows_rows(world: &mut RemindrWorld, step: &Step, view: String) {
    assert_eq!(view_rows(world, &view), first_column(step));
}

#[then(expr = "the view {string} shows no rows")]
fn view_shows_no_rows(world: &mut RemindrWorld, view: String) {
    assert!(view_rows(world, &view).is_empty());
}

#[then(expr = "the view {string} has {int} filters")]
fn view_has_filters(world: &mut RemindrWorld, view: String, count: usize) {
    let stored = world.session.workspace().view(world.view(&view)).unwrap();
    assert_eq!(stored.filters.len(), count);
}

#[then(expr = "the view {string} has {int} sorts")]
fn view_has_sorts(world: &mut RemindrWorld, view: String, count: usize) {
    let stored = world.session.workspace().view(world.view(&view)).unwrap();
    assert_eq!(stored.sorts.len(), count);
}

#[then(expr = "the view {string} is a {word} view")]
fn view_is_a_kind(world: &mut RemindrWorld, view: String, kind: String) {
    let stored = world.session.workspace().view(world.view(&view)).unwrap();
    let label = match stored.kind {
        ViewKind::Table => "table",
        ViewKind::Board { .. } => "board",
    };
    assert_eq!(label, kind);
}

#[then(expr = "the board {string} has the groups:")]
fn board_has_groups(world: &mut RemindrWorld, step: &Step, board: String) {
    let view = world.view(&board);
    let workspace = world.session.workspace();
    let ViewKind::Board { group_by } = workspace.view(view).expect("view exists").kind else {
        panic!("{board} is not a board");
    };
    let (_, definition) = property_definition(world, group_by);
    let actual: Vec<(String, String)> = workspace
        .board_groups(view)
        .expect("board groups")
        .into_iter()
        .map(|group| {
            let label = match group.option {
                Some(option) => option_name(&definition.kind, option),
                None => format!("No {}", definition.name),
            };
            (label, titles(world, &group.rows))
        })
        .collect();
    let expected: Vec<(String, String)> = records(step)
        .into_iter()
        .map(|record| (record["group"].to_owned(), record["rows"].to_owned()))
        .collect();
    assert_eq!(actual, expected);
}

#[test]
fn database_scenarios() {
    run_feature("tests/features/databases.feature");
}
