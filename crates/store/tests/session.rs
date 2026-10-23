#[cfg(test)]
mod script {
    use remindr_core::application::{Changes, Command, Created, Session, TextPoint};
    use remindr_core::domain::{
        BlockId, CalendarDate, DatabaseId, Emoji, Link, OptionId, PropertyId, RichText, ViewId,
    };
    use remindr_core::testing::ManualClock;
    use remindr_store::SqliteStore;

    pub struct Script {
        pub session: Session<SqliteStore, ManualClock>,
        clock: ManualClock,
    }

    impl Script {
        pub fn new(session: Session<SqliteStore, ManualClock>, clock: ManualClock) -> Script {
            Script { session, clock }
        }

        pub fn run(&mut self, command: Command) -> Changes {
            self.clock.advance(60_000);
            match self.session.execute(command.clone()) {
                Ok(changes) => changes,
                Err(error) => panic!("command failed: {command:?}: {error:?}"),
            }
        }

        pub fn created_block(&mut self, command: Command) -> BlockId {
            let changes = self.run(command);
            created_block(&changes)
        }

        pub fn text(&mut self, block: BlockId, text: &str) {
            self.run(Command::EditText {
                block,
                text: RichText::plain(text),
            });
        }

        pub fn undo(&mut self) {
            self.clock.advance(60_000);
            match self.session.undo() {
                Ok(Some(_)) => {}
                other => panic!("undo failed: {other:?}"),
            }
        }

        pub fn redo(&mut self) {
            self.clock.advance(60_000);
            match self.session.redo() {
                Ok(Some(_)) => {}
                other => panic!("redo failed: {other:?}"),
            }
        }

        pub fn property(&self, database: DatabaseId, name: &str) -> PropertyId {
            let schema = &self.known_database(database).schema;
            match schema
                .properties
                .iter()
                .find(|property| property.name == name)
            {
                Some(property) => property.id,
                None => panic!("no property named {name}"),
            }
        }

        pub fn option(&self, database: DatabaseId, property: &str, name: &str) -> OptionId {
            let property = self.property(database, property);
            let schema = &self.known_database(database).schema;
            let options = schema
                .properties
                .iter()
                .find(|candidate| candidate.id == property)
                .and_then(|candidate| match &candidate.kind {
                    remindr_core::domain::PropertyKind::Select { options }
                    | remindr_core::domain::PropertyKind::MultiSelect { options } => Some(options),
                    _ => None,
                });
            match options.and_then(|options| options.iter().find(|option| option.name == name)) {
                Some(option) => option.id,
                None => panic!("no option named {name}"),
            }
        }

        fn known_database(&self, database: DatabaseId) -> &remindr_core::domain::Database {
            match self.session.workspace().database(database) {
                Some(database) => database,
                None => panic!("unknown database"),
            }
        }
    }

    pub fn created_block(changes: &Changes) -> BlockId {
        match changes.created.iter().find_map(|created| match created {
            Created::Block(id) => Some(*id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("no created block: {changes:?}"),
        }
    }

    pub fn created_view(changes: &Changes) -> ViewId {
        match changes.created.iter().find_map(|created| match created {
            Created::View(id) => Some(*id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("no created view: {changes:?}"),
        }
    }

    pub fn created_database(changes: &Changes) -> DatabaseId {
        match changes.created.iter().find_map(|created| match created {
            Created::Database(id) => Some(*id),
            _ => None,
        }) {
            Some(id) => id,
            None => panic!("no created database: {changes:?}"),
        }
    }

    pub fn point(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    pub fn emoji(raw: &str) -> Emoji {
        Emoji::try_from(raw).unwrap()
    }

    pub fn link(raw: &str) -> Link {
        Link::try_from(raw).unwrap()
    }

    pub fn date(year: i32, month: u8, day: u8) -> CalendarDate {
        CalendarDate::new(year, month, day).unwrap()
    }
}

use remindr_core::application::{Command, DropTarget, Session};
use remindr_core::domain::{
    BlockId, BlockKind, CodeLanguage, Condition, DatabaseId, DateCondition, Filter, HeadingLevel,
    MarkChange, MultiSelectCondition, NumberCondition, NumberFormat, OptionColor, Parent,
    PropertyKind, PropertyValue, SelectCondition, Sort, SortDirection, TextColor, TextCondition,
    Timestamp, ViewId, ViewKind,
};
use remindr_core::testing::ManualClock;
use remindr_store::SqliteStore;

use script::{Script, created_block, created_database, created_view, date, emoji, link, point};

struct Pages {
    page: BlockId,
    intro: BlockId,
}

struct Tasks {
    database: DatabaseId,
    first_block: BlockId,
    first_view: ViewId,
    rows: [BlockId; 2],
}

fn write_pages(script: &mut Script) -> Pages {
    let page = script.created_block(Command::CreatePage {
        parent: Parent::Root,
        after: None,
    });
    script.run(Command::SetPageIcon {
        page,
        icon: Some(emoji("📄")),
    });
    script.text(page, "Home");
    let intro = script.created_block(Command::InsertBlockAt {
        target: DropTarget {
            parent: Parent::Block(page),
            after: None,
        },
        kind: BlockKind::Text,
    });
    script.text(intro, "Hello world, this is Remindr");
    let link = link("https://example.com/a?b=1");
    let marks = [
        (0, 5, MarkChange::Bold(true)),
        (6, 11, MarkChange::Italic(true)),
        (13, 17, MarkChange::Underline(true)),
        (18, 20, MarkChange::Strikethrough(true)),
        (21, 28, MarkChange::Code(true)),
        (0, 5, MarkChange::Link(Some(link))),
        (6, 11, MarkChange::Color(Some(TextColor::BlueBackground))),
    ];
    for (start, end, change) in marks {
        script.run(Command::SetMarks {
            start: point(intro, start),
            end: point(intro, end),
            change,
        });
    }
    let todo = script.created_block(Command::InsertBlock {
        after: intro,
        kind: BlockKind::ToDo { checked: true },
    });
    script.text(todo, "Ship it");
    let callout = script.created_block(Command::InsertBlock {
        after: todo,
        kind: BlockKind::Callout {
            icon: emoji("💡")
        },
    });
    script.text(callout, "Remember \"this\"");
    let code = script.created_block(Command::InsertBlock {
        after: callout,
        kind: BlockKind::Code {
            language: CodeLanguage::new("rust"),
        },
    });
    script.text(code, "fn main() {}");
    let heading = script.created_block(Command::InsertBlock {
        after: code,
        kind: BlockKind::Heading(HeadingLevel::Three),
    });
    script.text(heading, "Plan");
    script.run(Command::ChangeKind {
        block: heading,
        kind: BlockKind::Heading(HeadingLevel::One),
    });
    let bullet = script.created_block(Command::InsertBlock {
        after: heading,
        kind: BlockKind::BulletedListItem,
    });
    script.text(bullet, "first second");
    let nested = script.created_block(Command::SplitBlock {
        at: point(bullet, 5),
    });
    script.run(Command::Indent { block: nested });
    let quote = script.created_block(Command::InsertBlock {
        after: bullet,
        kind: BlockKind::Quote,
    });
    script.text(quote, "To be or not");
    script.run(Command::ChangeKind {
        block: quote,
        kind: BlockKind::Toggle,
    });
    let divider = script.created_block(Command::InsertBlock {
        after: quote,
        kind: BlockKind::Divider,
    });
    let sub_page = script.created_block(Command::CreatePage {
        parent: Parent::Block(page),
        after: None,
    });
    script.text(sub_page, "Sub page");
    script.run(Command::MoveBlock {
        block: divider,
        target: DropTarget {
            parent: Parent::Block(page),
            after: Some(intro),
        },
    });
    script.run(Command::DeleteBlocks { blocks: vec![todo] });
    Pages { page, intro }
}

fn write_database(script: &mut Script, pages: &Pages) -> Tasks {
    let changes = script.run(Command::CreateDatabaseWithView { after: pages.intro });
    let database = created_database(&changes);
    let first_block = created_block(&changes);
    let first_view = created_view(&changes);
    script.run(Command::RenameDatabase {
        database,
        name: "Tasks \"2026\"".to_owned(),
    });
    script.run(Command::SetDatabaseIcon {
        database,
        icon: Some(emoji("✅")),
    });
    let kinds = [
        ("Notes", PropertyKind::Text),
        (
            "Cost",
            PropertyKind::Number {
                format: NumberFormat::Euro,
            },
        ),
        ("Status", PropertyKind::Select { options: vec![] }),
        ("Tags", PropertyKind::MultiSelect { options: vec![] }),
        ("Due", PropertyKind::Date),
        ("Done", PropertyKind::Checkbox),
        ("Link", PropertyKind::Url),
        ("Scratch", PropertyKind::Text),
    ];
    for (name, kind) in kinds {
        script.run(Command::AddProperty {
            database,
            name: name.to_owned(),
            kind,
        });
    }
    let scratch = script.property(database, "Scratch");
    script.run(Command::RemoveProperty {
        database,
        property: scratch,
    });
    let status = script.property(database, "Status");
    for (name, color) in [
        ("Todo", OptionColor::Gray),
        ("Doing", OptionColor::Yellow),
        ("Done", OptionColor::Green),
    ] {
        script.run(Command::AddSelectOption {
            database,
            property: status,
            name: name.to_owned(),
            color,
        });
    }
    let tags = script.property(database, "Tags");
    for (name, color) in [
        ("Urgent", OptionColor::Red),
        ("Later", OptionColor::Default),
    ] {
        script.run(Command::AddSelectOption {
            database,
            property: tags,
            name: name.to_owned(),
            color,
        });
    }
    let later = script.option(database, "Tags", "Later");
    script.run(Command::RenameSelectOption {
        database,
        property: tags,
        option: later,
        name: "Someday".to_owned(),
    });
    let notes = script.property(database, "Notes");
    script.run(Command::RenameProperty {
        database,
        property: notes,
        name: "Remarks".to_owned(),
    });
    let link = script.property(database, "Link");
    script.run(Command::MoveProperty {
        database,
        property: link,
        index: 1,
    });
    let rows = write_rows(script, database);
    Tasks {
        database,
        first_block,
        first_view,
        rows,
    }
}

fn write_rows(script: &mut Script, database: DatabaseId) -> [BlockId; 2] {
    let property = |script: &Script, name: &str| script.property(database, name);
    let (remarks, cost, status) = (
        property(script, "Remarks"),
        property(script, "Cost"),
        property(script, "Status"),
    );
    let (tags, due, done, link) = (
        property(script, "Tags"),
        property(script, "Due"),
        property(script, "Done"),
        property(script, "Link"),
    );
    let todo = script.option(database, "Status", "Todo");
    let doing = script.option(database, "Status", "Doing");
    let urgent = script.option(database, "Tags", "Urgent");
    let someday = script.option(database, "Tags", "Someday");
    let first = script.created_block(Command::AddRow {
        database,
        values: vec![
            (remarks, PropertyValue::Text("Draft \"v1\" é".to_owned())),
            (cost, PropertyValue::Number(1.1186231841461947)),
            (status, PropertyValue::Select(todo)),
            (tags, PropertyValue::MultiSelect(vec![urgent, someday])),
            (due, PropertyValue::Date(date(2026, 10, 23))),
            (done, PropertyValue::Checkbox(true)),
            (
                link,
                PropertyValue::Url("https://example.com/spec".to_owned()),
            ),
        ],
    });
    script.text(first, "Write the spec");
    let second = script.created_block(Command::AddRow {
        database,
        values: vec![
            (cost, PropertyValue::Number(123456789.12345679)),
            (status, PropertyValue::Select(doing)),
            (done, PropertyValue::Checkbox(false)),
        ],
    });
    script.text(second, "Review");
    script.run(Command::SetValue {
        row: second,
        property: due,
        value: Some(PropertyValue::Date(date(2026, 12, 31))),
    });
    script.run(Command::SetValue {
        row: second,
        property: done,
        value: None,
    });
    script.run(Command::SetValue {
        row: second,
        property: link,
        value: None,
    });
    [first, second]
}

fn write_views(script: &mut Script, tasks: &Tasks) {
    let database = tasks.database;
    let (title, remarks, cost) = (
        script.property(database, "Name"),
        script.property(database, "Remarks"),
        script.property(database, "Cost"),
    );
    let (status, tags, due, done) = (
        script.property(database, "Status"),
        script.property(database, "Tags"),
        script.property(database, "Due"),
        script.property(database, "Done"),
    );
    let doing = script.option(database, "Status", "Doing");
    let urgent = script.option(database, "Tags", "Urgent");
    let filter = |property, condition| Filter {
        property,
        condition,
    };
    script.run(Command::SetFilters {
        view: tasks.first_view,
        filters: vec![
            filter(title, Condition::Text(TextCondition::IsNotEmpty)),
            filter(
                remarks,
                Condition::Text(TextCondition::Contains("Draft".to_owned())),
            ),
            filter(cost, Condition::Number(NumberCondition::Gt(3.0))),
            filter(status, Condition::Select(SelectCondition::Is(doing))),
            filter(
                tags,
                Condition::MultiSelect(MultiSelectCondition::Contains(urgent)),
            ),
            filter(
                due,
                Condition::Date(DateCondition::Before(date(2027, 1, 1))),
            ),
            filter(done, Condition::Checkbox(true)),
        ],
    });
    script.run(Command::SetSorts {
        view: tasks.first_view,
        sorts: vec![
            Sort {
                property: cost,
                direction: SortDirection::Descending,
            },
            Sort {
                property: title,
                direction: SortDirection::Ascending,
            },
        ],
    });
    script.run(Command::SetVisibleProperties {
        view: tasks.first_view,
        properties: vec![title, cost, status],
    });
    script.run(Command::SetColumnWidth {
        view: tasks.first_view,
        property: title,
        width: 280,
    });
    script.run(Command::SetColumnWidth {
        view: tasks.first_view,
        property: cost,
        width: 96,
    });
    script.run(Command::RenameView {
        view: tasks.first_view,
        name: "Everything \"main\"".to_owned(),
    });

    let board = created_view(&script.run(Command::AddView {
        block: tasks.first_block,
        name: "Board".to_owned(),
        kind: ViewKind::Board { group_by: status },
    }));
    script.run(Command::MoveCard {
        view: board,
        row: tasks.rows[1],
        group: Some(script.option(database, "Status", "Done")),
        before: None,
    });

    let second = script.run(Command::InsertDatabaseView {
        after: tasks.first_block,
        database,
    });
    let second_view = created_view(&second);
    script.run(Command::MoveRow {
        view: second_view,
        row: tasks.rows[1],
        before: Some(tasks.rows[0]),
    });
    script.run(Command::SetFilters {
        view: second_view,
        filters: vec![filter(done, Condition::Checkbox(false))],
    });
    script.run(Command::SetViewKind {
        view: second_view,
        kind: ViewKind::Board { group_by: status },
    });
}

fn write_history(script: &mut Script, pages: &Pages, tasks: &Tasks) {
    let extra = script.created_block(Command::CreatePage {
        parent: Parent::Root,
        after: Some(pages.page),
    });
    script.text(extra, "Scratch page");
    script.undo();
    script.undo();
    script.redo();
    script.redo();
    script.run(Command::AddView {
        block: tasks.first_block,
        name: "Scratch".to_owned(),
        kind: ViewKind::Table,
    });
    script.undo();
}

#[test]
fn reopening_the_file_restores_the_whole_workspace() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let clock = ManualClock::new(Timestamp::from_millis(1_760_000_000_000));
    let store = SqliteStore::open(&path).unwrap();
    let mut script = Script::new(Session::open(store, clock.clone()).unwrap(), clock);

    let pages = write_pages(&mut script);
    let tasks = write_database(&mut script, &pages);
    write_views(&mut script, &tasks);
    write_history(&mut script, &pages, &tasks);

    let before = script.session.workspace().to_snapshot();
    drop(script);

    assert_eq!(before.databases.len(), 1);
    assert_eq!(before.views.len(), 3);
    assert!(before.blocks.iter().any(|block| !block.alive));
    assert!(before.values.len() >= 9);
    assert!(
        before
            .blocks
            .iter()
            .any(|block| block.text.to_plain() == "Scratch page")
    );
    assert!(before.views.iter().all(|view| view.name != "Scratch"));

    let store = SqliteStore::open(&path).unwrap();
    let clock = ManualClock::new(Timestamp::from_millis(1_760_100_000_000));
    let reopened = Session::open(store, clock).unwrap();
    assert_eq!(reopened.workspace().to_snapshot(), before);
}
