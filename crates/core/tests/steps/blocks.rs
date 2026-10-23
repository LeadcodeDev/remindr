use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use remindr_core::application::{Command, Created, DropTarget, TextPoint};
use remindr_core::domain::{
    BlockId, BlockKind, CodeLanguage, HeadingLevel, Parent, PropertyKind, PropertyValue, RichText,
};

use crate::{RemindrWorld, run_feature};

const SETTLE_MS: i64 = 1_000;

fn kind_named(name: &str) -> BlockKind {
    match name {
        "text" => BlockKind::Text,
        "heading" => BlockKind::Heading(HeadingLevel::One),
        "bulleted" => BlockKind::BulletedListItem,
        "numbered" => BlockKind::NumberedListItem,
        "todo" => BlockKind::ToDo { checked: false },
        "toggle" => BlockKind::Toggle,
        "quote" => BlockKind::Quote,
        "divider" => BlockKind::Divider,
        "code" => BlockKind::Code {
            language: CodeLanguage::plain_text(),
        },
        other => panic!("unknown block kind {other}"),
    }
}

fn kind_label(kind: &BlockKind) -> &'static str {
    match kind {
        BlockKind::Text => "text",
        BlockKind::Heading(_) => "heading",
        BlockKind::BulletedListItem => "bulleted",
        BlockKind::NumberedListItem => "numbered",
        BlockKind::ToDo { .. } => "todo",
        BlockKind::Toggle => "toggle",
        BlockKind::Quote => "quote",
        BlockKind::Callout { .. } => "callout",
        BlockKind::Code { .. } => "code",
        BlockKind::Divider => "divider",
        BlockKind::Page { .. } => "page",
        BlockKind::DatabaseView { .. } => "database view",
    }
}

fn table(step: &Step) -> &[Vec<String>] {
    &step.table.as_ref().expect("step has a table").rows
}

fn cell<'a>(header: &[String], row: &'a [String], column: &str) -> &'a str {
    header
        .iter()
        .position(|name| name == column)
        .and_then(|index| row.get(index))
        .map_or("", String::as_str)
}

fn unquote(raw: &str) -> &str {
    raw.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(raw)
}

fn created_block(world: &RemindrWorld) -> BlockId {
    match world.changes().created.first() {
        Some(Created::Block(id)) => *id,
        other => panic!("no block was created: {other:?}"),
    }
}

fn run_ok(world: &mut RemindrWorld, command: Command) {
    world.run(command);
    world.changes();
}

fn texts_under(world: &RemindrWorld, parent: BlockId) -> Vec<String> {
    world
        .session
        .workspace()
        .children(Parent::Block(parent))
        .map(|block| block.text.to_plain())
        .collect()
}

#[given(expr = "the page {string} has the blocks:")]
fn page_has_blocks(world: &mut RemindrWorld, step: &Step, page: String) {
    let rows = table(step);
    let (header, rows) = rows.split_first().expect("table has a header");
    let page = world.block(&page);
    let mut previous_top: Option<BlockId> = None;
    for row in rows {
        let parent = cell(header, row, "parent");
        let kind = cell(header, row, "kind");
        let text = cell(header, row, "text");
        let insertion = if parent.is_empty() {
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: Parent::Block(page),
                    after: previous_top,
                },
                kind: BlockKind::Text,
            }
        } else {
            Command::InsertBlock {
                after: world.block(parent),
                kind: BlockKind::Text,
            }
        };
        run_ok(world, insertion);
        let block = created_block(world);
        if parent.is_empty() {
            previous_top = Some(block);
        } else {
            run_ok(world, Command::Indent { block });
        }
        if !kind.is_empty() && kind != "text" {
            run_ok(
                world,
                Command::ChangeKind {
                    block,
                    kind: kind_named(kind),
                },
            );
        }
        if !text.is_empty() {
            run_ok(
                world,
                Command::EditText {
                    block,
                    text: RichText::plain(text),
                },
            );
        }
        world
            .blocks
            .insert(cell(header, row, "name").to_owned(), block);
    }
    world.clock.advance(SETTLE_MS);
}

#[given(expr = "the database {string} holds the row {string}")]
fn database_holds_row(world: &mut RemindrWorld, database_name: String, row: String) {
    run_ok(world, Command::CreateDatabase);
    let database = match world.changes().created.first() {
        Some(Created::Database(id)) => *id,
        other => panic!("no database was created: {other:?}"),
    };
    world.databases.insert(database_name, database);
    run_ok(
        world,
        Command::AddProperty {
            database,
            name: "Notes".to_owned(),
            kind: PropertyKind::Text,
        },
    );
    let property = world
        .session
        .workspace()
        .database(database)
        .expect("the database exists")
        .schema
        .properties
        .iter()
        .find(|property| property.name == "Notes")
        .expect("the property exists")
        .id;
    run_ok(
        world,
        Command::AddRow {
            database,
            values: vec![(property, PropertyValue::Text("x".to_owned()))],
        },
    );
    let block = created_block(world);
    world.blocks.insert(row, block);
}

#[when(expr = "I move the page {string} to the top level")]
fn move_to_top_level(world: &mut RemindrWorld, name: String) {
    let block = world.block(&name);
    let target = DropTarget {
        parent: Parent::Root,
        after: None,
    };
    world.run(Command::MoveBlock { block, target });
}

#[when(expr = "I move the page {string} into the database {string}")]
fn move_into_database(world: &mut RemindrWorld, name: String, database: String) {
    let block = world.block(&name);
    let target = DropTarget {
        parent: Parent::Database(world.database(&database)),
        after: None,
    };
    world.run(Command::MoveBlock { block, target });
}

#[when(expr = "I type {string} in the empty page {string}")]
fn type_in_empty_page(world: &mut RemindrWorld, text: String, page: String) {
    let target = DropTarget {
        parent: Parent::Block(world.block(&page)),
        after: None,
    };
    run_ok(
        world,
        Command::InsertBlockAt {
            target,
            kind: BlockKind::Text,
        },
    );
    let block = created_block(world);
    world.run(Command::EditText {
        block,
        text: RichText::plain(&text),
    });
}

#[when(expr = "I type {string} into {string}")]
fn type_into(world: &mut RemindrWorld, text: String, name: String) {
    let block = world.block(&name);
    world.run(Command::EditText {
        block,
        text: RichText::plain(&text),
    });
}

#[when(expr = "I split {string} at offset {int}")]
fn split(world: &mut RemindrWorld, name: String, offset: usize) {
    let block = world.block(&name);
    world.run(Command::SplitBlock {
        at: TextPoint { block, offset },
    });
}

#[when(expr = "I press Backspace at the start of {string}")]
fn backspace_at_start(world: &mut RemindrWorld, name: String) {
    let block = world.block(&name);
    world.run(Command::MergeWithPrevious { block });
}

#[when(expr = "I delete from offset {int} of {string} to offset {int} of {string}")]
fn delete_range(
    world: &mut RemindrWorld,
    start_offset: usize,
    start_name: String,
    end_offset: usize,
    end_name: String,
) {
    let start = TextPoint {
        block: world.block(&start_name),
        offset: start_offset,
    };
    let end = TextPoint {
        block: world.block(&end_name),
        offset: end_offset,
    };
    world.run(Command::DeleteRange { start, end });
}

#[when(expr = "I indent {string}")]
fn indent(world: &mut RemindrWorld, name: String) {
    let block = world.block(&name);
    world.run(Command::Indent { block });
}

#[when(expr = "I outdent {string}")]
fn outdent(world: &mut RemindrWorld, name: String) {
    let block = world.block(&name);
    world.run(Command::Outdent { block });
}

#[when(expr = "I move {string} under {string}")]
fn move_under(world: &mut RemindrWorld, name: String, parent: String) {
    let block = world.block(&name);
    let target = DropTarget {
        parent: Parent::Block(world.block(&parent)),
        after: None,
    };
    world.run(Command::MoveBlock { block, target });
}

#[when(expr = "I delete the page {string}")]
fn delete_page(world: &mut RemindrWorld, name: String) {
    let page = world.block(&name);
    world.run(Command::DeleteBlocks { blocks: vec![page] });
}

#[then(expr = "the children of {string} read:")]
fn children_read(world: &mut RemindrWorld, step: &Step, name: String) {
    let expected: Vec<&str> = table(step)
        .iter()
        .map(|row| unquote(row.first().map_or("", String::as_str)))
        .collect();
    let parent = world.block(&name);
    assert_eq!(texts_under(world, parent), expected);
}

#[then(expr = "block {string} has text {string}")]
fn block_has_text(world: &mut RemindrWorld, name: String, expected: String) {
    let block = world.session.workspace().block(world.block(&name)).unwrap();
    assert_eq!(block.text.to_plain(), expected);
}

#[then(expr = "block {string} is a {word} block")]
fn block_is_a_kind(world: &mut RemindrWorld, name: String, expected: String) {
    let block = world.session.workspace().block(world.block(&name)).unwrap();
    assert_eq!(kind_label(&block.kind), expected);
}

#[then(expr = "the first child of {string} is a {word} block")]
fn first_child_is_a_kind(world: &mut RemindrWorld, name: String, expected: String) {
    let parent = Parent::Block(world.block(&name));
    let first = world
        .session
        .workspace()
        .children(parent)
        .next()
        .expect("the page has a child");
    assert_eq!(kind_label(&first.kind), expected);
}

#[then(expr = "block {string} is deleted")]
fn block_is_deleted(world: &mut RemindrWorld, name: String) {
    let block = world.session.workspace().block(world.block(&name)).unwrap();
    assert!(!block.alive);
}

#[then(expr = "block {string} is alive")]
fn block_is_alive(world: &mut RemindrWorld, name: String) {
    let block = world.session.workspace().block(world.block(&name)).unwrap();
    assert!(block.alive);
}

#[then(expr = "the page {string} is listed among the pages")]
fn page_is_listed(world: &mut RemindrWorld, name: String) {
    let page = world.block(&name);
    let listed = world
        .session
        .workspace()
        .children(Parent::Root)
        .any(|block| block.id == page);
    assert!(listed);
}

#[test]
fn block_scenarios() {
    run_feature("tests/features/blocks.feature");
}
