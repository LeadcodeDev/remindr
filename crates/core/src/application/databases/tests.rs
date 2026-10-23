use crate::application::{Built, Command, Created};
use crate::domain::{
    Block, BlockId, BlockKind, CalendarDate, Condition, DatabaseId, DomainError, Filter,
    MultiSelectCondition, NumberCondition, NumberFormat, Operation, OptionColor, OptionId, Parent,
    PropertyId, PropertyKind, PropertyValue, RichText, SelectCondition, Sort, SortDirection,
    SortKey, TextCondition, Timestamp, Transaction, ViewId, ViewKind, Workspace, WorkspaceSnapshot,
};

use super::build;

fn now() -> Timestamp {
    Timestamp::from_millis(1_000)
}

struct Env {
    workspace: Workspace,
    page: BlockId,
    intro: BlockId,
    database: DatabaseId,
    embed: BlockId,
    view: ViewId,
}

impl Env {
    fn bare() -> Env {
        let page = BlockId::new();
        let block = Block {
            id: page,
            kind: BlockKind::Page { icon: None },
            parent: Parent::Root,
            sort_key: SortKey::between(None, None),
            text: RichText::plain("Home"),
            alive: true,
            created_at: now(),
        };
        let intro = BlockId::new();
        let first_child = Block {
            id: intro,
            kind: BlockKind::Text,
            parent: Parent::Block(page),
            sort_key: SortKey::between(None, None),
            text: RichText::plain("intro"),
            alive: true,
            created_at: now(),
        };
        let snapshot = WorkspaceSnapshot {
            blocks: vec![block, first_child],
            ..WorkspaceSnapshot::default()
        };
        Env {
            workspace: Workspace::from_snapshot(snapshot).unwrap(),
            page,
            intro,
            database: DatabaseId::new(),
            embed: BlockId::new(),
            view: ViewId::new(),
        }
    }

    fn new() -> Env {
        let mut env = Env::bare();
        let built = env.run(Command::CreateDatabaseWithView { after: env.intro });
        for created in built.created {
            match created {
                Created::Database(id) => env.database = id,
                Created::Block(id) => env.embed = id,
                Created::View(id) => env.view = id,
            }
        }
        env
    }

    fn with_schema() -> Env {
        let mut env = Env::new();
        let database = env.database;
        let kinds = [
            (
                "Status",
                PropertyKind::Select {
                    options: Vec::new(),
                },
            ),
            (
                "Amount",
                PropertyKind::Number {
                    format: NumberFormat::Plain,
                },
            ),
            ("Notes", PropertyKind::Text),
            (
                "Tags",
                PropertyKind::MultiSelect {
                    options: Vec::new(),
                },
            ),
            ("Due", PropertyKind::Date),
            ("Done", PropertyKind::Checkbox),
            ("Link", PropertyKind::Url),
        ];
        for (name, kind) in kinds {
            env.run(Command::AddProperty {
                database,
                name: name.to_owned(),
                kind,
            });
        }
        for name in ["Todo", "Doing", "Done"] {
            env.add_option("Status", name);
        }
        for name in ["Red", "Blue"] {
            env.add_option("Tags", name);
        }
        env
    }

    fn add_option(&mut self, property: &str, name: &str) {
        let command = Command::AddSelectOption {
            database: self.database,
            property: self.property(property),
            name: name.to_owned(),
            color: OptionColor::Default,
        };
        self.run(command);
    }

    fn try_run(&mut self, command: Command) -> Result<(Built, Transaction), DomainError> {
        let built = build(&self.workspace, &command, now())?;
        let inverse = self.workspace.apply(&built.transaction)?;
        Ok((built, inverse))
    }

    fn run(&mut self, command: Command) -> Built {
        self.try_run(command).unwrap().0
    }

    fn rejected(&mut self, command: Command) -> DomainError {
        let before = self.workspace.clone();
        let error = self.try_run(command).unwrap_err();
        assert_eq!(self.workspace, before);
        error
    }

    fn property(&self, name: &str) -> PropertyId {
        self.workspace
            .database(self.database)
            .unwrap()
            .schema
            .properties
            .iter()
            .find(|property| property.name == name)
            .unwrap()
            .id
    }

    fn kind(&self, name: &str) -> PropertyKind {
        self.workspace
            .database(self.database)
            .unwrap()
            .schema
            .properties
            .iter()
            .find(|property| property.name == name)
            .unwrap()
            .kind
            .clone()
    }

    fn option(&self, property: &str, name: &str) -> OptionId {
        self.kind(property)
            .options()
            .unwrap()
            .iter()
            .find(|option| option.name == name)
            .unwrap()
            .id
    }

    fn add_row(&mut self, title: &str) -> BlockId {
        let built = self.run(Command::AddRow {
            database: self.database,
            values: Vec::new(),
        });
        let Some(Created::Block(row)) = built.created.first().copied() else {
            panic!("no row created");
        };
        let text = RichText::plain(title);
        self.workspace
            .apply(&Transaction::new(vec![Operation::SetText {
                block: row,
                text,
            }]))
            .unwrap();
        row
    }

    fn set(&mut self, row: BlockId, property: &str, value: Option<PropertyValue>) {
        let command = Command::SetValue {
            row,
            property: self.property(property),
            value,
        };
        self.run(command);
    }

    fn titles_of(&self, view: ViewId) -> Vec<String> {
        self.workspace
            .query(view)
            .unwrap()
            .into_iter()
            .map(|row| self.workspace.block(row).unwrap().text.to_plain())
            .collect()
    }

    fn titles(&self) -> Vec<String> {
        self.titles_of(self.view)
    }

    fn value(&self, row: BlockId, property: &str) -> Option<PropertyValue> {
        self.workspace.value(row, self.property(property)).cloned()
    }

    fn view_of(&self, id: ViewId) -> crate::domain::View {
        self.workspace.view(id).unwrap().clone()
    }
}

fn text(value: &str) -> Option<PropertyValue> {
    Some(PropertyValue::Text(value.to_owned()))
}

fn assert_undo_restores(command: Command, env: &mut Env) {
    let before = env.workspace.clone();
    let (_, inverse) = env.try_run(command).unwrap();
    assert_ne!(env.workspace, before);
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
}

#[test]
fn create_database_adds_a_database_with_a_title_property_and_no_block() {
    let mut env = Env::bare();
    let blocks_before = env.workspace.to_snapshot().blocks;
    let built = env.run(Command::CreateDatabase);
    let Some(Created::Database(id)) = built.created.first().copied() else {
        panic!("no database created");
    };
    assert_eq!(built.created.len(), 1);
    let database = env.workspace.database(id).unwrap();
    assert_eq!(database.name, "Untitled");
    assert_eq!(database.icon, None);
    assert!(database.alive);
    assert_eq!(database.created_at, now());
    assert_eq!(database.schema.properties.len(), 1);
    assert_eq!(database.schema.properties[0].name, "Name");
    assert_eq!(database.schema.properties[0].kind, PropertyKind::Title);
    assert_eq!(env.workspace.databases().count(), 1);
    assert_eq!(env.workspace.to_snapshot().blocks, blocks_before);
}

#[test]
fn create_database_orders_after_the_last_database() {
    let mut env = Env::bare();
    env.run(Command::CreateDatabase);
    env.run(Command::CreateDatabase);
    env.run(Command::CreateDatabase);
    let keys: Vec<SortKey> = env
        .workspace
        .databases()
        .map(|database| database.sort_key.clone())
        .collect();
    assert_eq!(keys.len(), 3);
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]));
    let created_order: Vec<_> = env.workspace.databases().map(|d| d.id).collect();
    let mut by_id = created_order.clone();
    by_id.sort();
    assert_eq!(created_order, by_id);
}

#[test]
fn create_database_with_view_inserts_a_block_and_a_table_view_named_table() {
    let mut env = Env::bare();
    let built = env.run(Command::CreateDatabaseWithView { after: env.intro });
    assert_eq!(built.created.len(), 3);
    let (Created::Database(database), Created::Block(block), Created::View(view)) =
        (built.created[0], built.created[1], built.created[2])
    else {
        panic!("created order is database, block, view");
    };
    let title = env.workspace.database(database).unwrap().schema.properties[0].id;
    let embed = env.workspace.block(block).unwrap();
    assert_eq!(embed.kind, BlockKind::DatabaseView { database });
    assert_eq!(embed.parent, Parent::Block(env.page));
    let siblings: Vec<BlockId> = env
        .workspace
        .children(Parent::Block(env.page))
        .map(|sibling| sibling.id)
        .collect();
    assert_eq!(siblings, [env.intro, block]);
    let view = env.workspace.view(view).unwrap();
    assert_eq!(view.block, block);
    assert_eq!(view.name, "Table");
    assert_eq!(view.kind, ViewKind::Table);
    assert_eq!(view.visible, [title]);
    assert!(view.filters.is_empty() && view.sorts.is_empty());
    assert_eq!(env.workspace.databases().count(), 1);
}

#[test]
fn create_database_with_view_places_the_block_after_a_sibling() {
    let mut env = Env::new();
    let first = env.embed;
    let tail = env.run(Command::InsertBlock {
        after: first,
        kind: BlockKind::Text,
    });
    let Some(Created::Block(tail)) = tail.created.first().copied() else {
        panic!("no block");
    };
    let built = env.run(Command::CreateDatabaseWithView { after: first });
    let Some(Created::Block(second)) = built.created.get(1).copied() else {
        panic!("no block");
    };
    let order: Vec<BlockId> = env
        .workspace
        .children(Parent::Block(env.page))
        .map(|block| block.id)
        .collect();
    assert_eq!(order, [env.intro, first, second, tail]);
}

#[test]
fn create_database_with_view_rejects_an_unknown_anchor() {
    let mut env = Env::bare();
    let unknown = BlockId::new();
    assert_eq!(
        env.rejected(Command::CreateDatabaseWithView { after: unknown }),
        DomainError::UnknownBlock(unknown)
    );
}

#[test]
fn create_database_with_view_after_a_root_page_is_rejected() {
    let mut env = Env::bare();
    assert_eq!(
        env.rejected(Command::CreateDatabaseWithView { after: env.page }),
        DomainError::InvalidParent
    );
}

#[test]
fn insert_database_view_after_a_root_page_is_rejected() {
    let mut env = Env::new();
    assert_eq!(
        env.rejected(Command::InsertDatabaseView {
            after: env.page,
            database: env.database,
        }),
        DomainError::InvalidParent
    );
}

#[test]
fn create_database_with_view_undo_removes_everything() {
    let mut env = Env::bare();
    let before = env.workspace.clone();
    let (_, inverse) = env
        .try_run(Command::CreateDatabaseWithView { after: env.intro })
        .unwrap();
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(
        env.workspace.to_snapshot().databases,
        before.to_snapshot().databases
    );
    assert!(env.workspace.to_snapshot().views.is_empty());
    assert_eq!(env.workspace.children(Parent::Block(env.page)).count(), 1);
}

#[test]
fn insert_database_view_shows_an_existing_database_with_all_properties_visible() {
    let mut env = Env::with_schema();
    let before_views = env.workspace.views(env.embed).count();
    let built = env.run(Command::InsertDatabaseView {
        after: env.embed,
        database: env.database,
    });
    assert_eq!(before_views, 1);
    let (Created::Block(block), Created::View(view)) = (built.created[0], built.created[1]) else {
        panic!("created order is block, view");
    };
    assert_eq!(built.created.len(), 2);
    assert_eq!(
        env.workspace.block(block).unwrap().kind,
        BlockKind::DatabaseView {
            database: env.database
        }
    );
    let all: Vec<PropertyId> = env
        .workspace
        .database(env.database)
        .unwrap()
        .schema
        .properties
        .iter()
        .map(|property| property.id)
        .collect();
    let view = env.workspace.view(view).unwrap();
    assert_eq!(view.visible, all);
    assert_eq!(view.name, "Table");
    assert_eq!(view.kind, ViewKind::Table);
    assert_eq!(env.workspace.databases().count(), 1);
}

#[test]
fn insert_database_view_rejects_an_unknown_database() {
    let mut env = Env::new();
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::InsertDatabaseView {
            after: env.embed,
            database: unknown
        }),
        DomainError::UnknownDatabase(unknown)
    );
}

#[test]
fn rename_database_and_set_icon_are_single_operations() {
    let mut env = Env::new();
    let built = env.run(Command::RenameDatabase {
        database: env.database,
        name: "Tasks".to_owned(),
    });
    assert_eq!(built.transaction.operations().len(), 1);
    assert!(built.created.is_empty());
    let icon = crate::domain::Emoji::try_from("📋").unwrap();
    let built = env.run(Command::SetDatabaseIcon {
        database: env.database,
        icon: Some(icon.clone()),
    });
    assert_eq!(built.transaction.operations().len(), 1);
    let database = env.workspace.database(env.database).unwrap();
    assert_eq!(database.name, "Tasks");
    assert_eq!(database.icon, Some(icon));
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::RenameDatabase {
            database: unknown,
            name: "x".to_owned()
        }),
        DomainError::UnknownDatabase(unknown)
    );
}

#[test]
fn delete_database_hides_the_database_and_its_rows_and_undo_restores_them() {
    let mut env = Env::with_schema();
    let a = env.add_row("a");
    let b = env.add_row("b");
    env.workspace
        .apply(&Transaction::new(vec![Operation::SetBlockAlive {
            block: b,
            alive: false,
        }]))
        .unwrap();
    let before = env.workspace.clone();
    let (built, inverse) = env
        .try_run(Command::DeleteDatabase {
            database: env.database,
        })
        .unwrap();
    assert!(built.created.is_empty());
    assert!(!env.workspace.database(env.database).unwrap().alive);
    assert!(!env.workspace.block(a).unwrap().alive);
    assert_eq!(env.workspace.databases().count(), 0);
    assert_eq!(env.workspace.query(env.view), Ok(Vec::new()));
    assert!(env.workspace.block(env.embed).is_some());
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
}

#[test]
fn delete_database_rejects_an_unknown_database() {
    let mut env = Env::new();
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::DeleteDatabase { database: unknown }),
        DomainError::UnknownDatabase(unknown)
    );
}

#[test]
fn deleting_a_database_view_block_leaves_database_and_rows_unchanged() {
    let mut env = Env::with_schema();
    env.add_row("a");
    let databases = env.workspace.to_snapshot().databases;
    let rows: Vec<BlockId> = env.workspace.rows(env.database).map(|row| row.id).collect();
    let built = env.run(Command::DeleteBlocks {
        blocks: vec![env.embed],
    });
    assert!(!built.transaction.is_empty());
    assert!(!env.workspace.block(env.embed).unwrap().alive);
    assert_eq!(env.workspace.to_snapshot().databases, databases);
    let after: Vec<BlockId> = env.workspace.rows(env.database).map(|row| row.id).collect();
    assert_eq!(after, rows);
    assert!(env.workspace.database(env.database).unwrap().alive);
}

#[test]
fn add_property_appends_it_and_shows_it_in_every_view() {
    let mut env = Env::new();
    let other = env.run(Command::InsertDatabaseView {
        after: env.embed,
        database: env.database,
    });
    let Some(Created::View(other_view)) = other.created.get(1).copied() else {
        panic!("no view");
    };
    let before = env.workspace.clone();
    let built = env.run(Command::AddProperty {
        database: env.database,
        name: "Amount".to_owned(),
        kind: PropertyKind::Number {
            format: NumberFormat::Euro,
        },
    });
    assert!(built.created.is_empty());
    let schema = &env.workspace.database(env.database).unwrap().schema;
    assert_eq!(schema.properties.len(), 2);
    let added = schema.properties[1].clone();
    assert_eq!(added.name, "Amount");
    assert_eq!(
        added.kind,
        PropertyKind::Number {
            format: NumberFormat::Euro
        }
    );
    for view in [env.view, other_view] {
        assert_eq!(env.view_of(view).visible.last(), Some(&added.id));
    }
    assert_ne!(env.workspace, before);
    assert_undo_restores(
        Command::AddProperty {
            database: env.database,
            name: "Again".to_owned(),
            kind: PropertyKind::Checkbox,
        },
        &mut env,
    );
}

#[test]
fn add_property_rejects_a_second_title() {
    let mut env = Env::new();
    assert_eq!(
        env.rejected(Command::AddProperty {
            database: env.database,
            name: "Another".to_owned(),
            kind: PropertyKind::Title,
        }),
        DomainError::TitleIsBlockText
    );
}

#[test]
fn add_property_rejects_an_unknown_database() {
    let mut env = Env::new();
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::AddProperty {
            database: unknown,
            name: "x".to_owned(),
            kind: PropertyKind::Text,
        }),
        DomainError::UnknownDatabase(unknown)
    );
}

#[test]
fn rename_property_keeps_kind_and_position() {
    let mut env = Env::with_schema();
    let amount = env.property("Amount");
    let kind = env.kind("Amount");
    env.run(Command::RenameProperty {
        database: env.database,
        property: amount,
        name: "Cost".to_owned(),
    });
    let schema = &env.workspace.database(env.database).unwrap().schema;
    assert_eq!(schema.properties[2].id, amount);
    assert_eq!(schema.properties[2].name, "Cost");
    assert_eq!(schema.properties[2].kind, kind);
    let unknown = PropertyId::new();
    assert_eq!(
        env.rejected(Command::RenameProperty {
            database: env.database,
            property: unknown,
            name: "x".to_owned()
        }),
        DomainError::UnknownProperty(unknown)
    );
}

#[test]
fn move_property_reorders_the_schema() {
    let mut env = Env::with_schema();
    let link = env.property("Link");
    env.run(Command::MoveProperty {
        database: env.database,
        property: link,
        index: 1,
    });
    let ids: Vec<PropertyId> = env
        .workspace
        .database(env.database)
        .unwrap()
        .schema
        .properties
        .iter()
        .map(|property| property.id)
        .collect();
    assert_eq!(ids[1], link);
    assert_eq!(ids.len(), 8);
    assert_undo_restores(
        Command::MoveProperty {
            database: env.database,
            property: link,
            index: 5,
        },
        &mut env,
    );
}

#[test]
fn select_options_can_be_added_renamed_and_removed() {
    let mut env = Env::with_schema();
    let status = env.property("Status");
    let names = |env: &Env| -> Vec<String> {
        env.kind("Status")
            .options()
            .unwrap()
            .iter()
            .map(|option| option.name.clone())
            .collect()
    };
    assert_eq!(names(&env), ["Todo", "Doing", "Done"]);
    env.run(Command::AddSelectOption {
        database: env.database,
        property: status,
        name: "Later".to_owned(),
        color: OptionColor::Red,
    });
    let later = env.option("Status", "Later");
    assert_eq!(
        env.kind("Status").options().unwrap().last().unwrap().color,
        OptionColor::Red
    );
    env.run(Command::RenameSelectOption {
        database: env.database,
        property: status,
        option: later,
        name: "Someday".to_owned(),
    });
    assert_eq!(names(&env), ["Todo", "Doing", "Done", "Someday"]);
    env.run(Command::RemoveSelectOption {
        database: env.database,
        property: status,
        option: later,
    });
    assert_eq!(names(&env), ["Todo", "Doing", "Done"]);
}

#[test]
fn select_option_commands_reject_bad_targets() {
    let mut env = Env::with_schema();
    let status = env.property("Status");
    let unknown = OptionId::new();
    assert_eq!(
        env.rejected(Command::RenameSelectOption {
            database: env.database,
            property: status,
            option: unknown,
            name: "x".to_owned()
        }),
        DomainError::UnknownOption(unknown)
    );
    assert_eq!(
        env.rejected(Command::RemoveSelectOption {
            database: env.database,
            property: status,
            option: unknown
        }),
        DomainError::UnknownOption(unknown)
    );
    assert_eq!(
        env.rejected(Command::AddSelectOption {
            database: env.database,
            property: env.property("Notes"),
            name: "x".to_owned(),
            color: OptionColor::Default
        }),
        DomainError::ValueKindMismatch
    );
}

#[test]
fn removing_a_select_option_clears_the_values_that_reference_it_in_one_transaction() {
    let mut env = Env::with_schema();
    let done = env.option("Status", "Done");
    let todo = env.option("Status", "Todo");
    let red = env.option("Tags", "Red");
    let blue = env.option("Tags", "Blue");
    let a = env.add_row("a");
    let b = env.add_row("b");
    let c = env.add_row("c");
    env.set(a, "Status", Some(PropertyValue::Select(done)));
    env.set(b, "Status", Some(PropertyValue::Select(todo)));
    env.set(a, "Tags", Some(PropertyValue::MultiSelect(vec![red, blue])));
    env.set(b, "Tags", Some(PropertyValue::MultiSelect(vec![red])));
    env.set(c, "Tags", Some(PropertyValue::MultiSelect(vec![blue])));
    let before = env.workspace.clone();
    let status = env.property("Status");
    let (built, inverse) = env
        .try_run(Command::RemoveSelectOption {
            database: env.database,
            property: status,
            option: done,
        })
        .unwrap();
    assert!(built.created.is_empty());
    assert_eq!(env.value(a, "Status"), None);
    assert_eq!(env.value(b, "Status"), Some(PropertyValue::Select(todo)));
    assert_eq!(env.kind("Status").options().unwrap().len(), 2);
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
    let tags = env.property("Tags");
    env.run(Command::RemoveSelectOption {
        database: env.database,
        property: tags,
        option: red,
    });
    assert_eq!(
        env.value(a, "Tags"),
        Some(PropertyValue::MultiSelect(vec![blue]))
    );
    assert_eq!(env.value(b, "Tags"), None);
    assert_eq!(
        env.value(c, "Tags"),
        Some(PropertyValue::MultiSelect(vec![blue]))
    );
}

#[test]
fn removing_a_select_option_drops_the_filters_that_mention_it() {
    let mut env = Env::with_schema();
    let done = env.option("Status", "Done");
    let todo = env.option("Status", "Todo");
    let status = env.property("Status");
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![
            Filter {
                property: status,
                condition: Condition::Select(SelectCondition::Is(done)),
            },
            Filter {
                property: status,
                condition: Condition::Select(SelectCondition::IsNot(todo)),
            },
            Filter {
                property: status,
                condition: Condition::Select(SelectCondition::IsEmpty),
            },
        ],
    });
    env.run(Command::RemoveSelectOption {
        database: env.database,
        property: status,
        option: done,
    });
    let filters = env.view_of(env.view).filters;
    assert_eq!(filters.len(), 2);
    assert!(
        filters
            .iter()
            .all(|filter| filter.condition != Condition::Select(SelectCondition::Is(done)))
    );
}

#[test]
fn add_row_creates_a_page_under_the_database_with_the_given_values() {
    let mut env = Env::with_schema();
    let amount = env.property("Amount");
    let done = env.option("Status", "Done");
    let status = env.property("Status");
    let first = env.add_row("first");
    let built = env.run(Command::AddRow {
        database: env.database,
        values: vec![
            (amount, PropertyValue::Number(7.0)),
            (status, PropertyValue::Select(done)),
        ],
    });
    let Some(Created::Block(row)) = built.created.first().copied() else {
        panic!("no row created");
    };
    assert_eq!(built.created.len(), 1);
    let block = env.workspace.block(row).unwrap();
    assert_eq!(block.kind, BlockKind::Page { icon: None });
    assert_eq!(block.parent, Parent::Database(env.database));
    assert!(block.text.is_empty());
    assert!(block.alive);
    assert_eq!(block.created_at, now());
    assert_eq!(env.value(row, "Amount"), Some(PropertyValue::Number(7.0)));
    assert_eq!(env.value(row, "Status"), Some(PropertyValue::Select(done)));
    let order: Vec<BlockId> = env.workspace.rows(env.database).map(|row| row.id).collect();
    assert_eq!(order, [first, row]);
}

#[test]
fn add_row_rejects_an_unknown_database_and_a_bad_value_atomically() {
    let mut env = Env::with_schema();
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::AddRow {
            database: unknown,
            values: Vec::new()
        }),
        DomainError::UnknownDatabase(unknown)
    );
    let amount = env.property("Amount");
    assert_eq!(
        env.rejected(Command::AddRow {
            database: env.database,
            values: vec![(amount, PropertyValue::Text("x".to_owned()))],
        }),
        DomainError::ValueKindMismatch
    );
    assert_eq!(env.workspace.rows(env.database).count(), 0);
}

#[test]
fn set_value_stores_each_kind_and_rejects_the_wrong_kind() {
    let mut env = Env::with_schema();
    let row = env.add_row("a");
    let date = PropertyValue::Date(CalendarDate::new(2026, 10, 23).unwrap());
    let status = env.option("Status", "Todo");
    let red = env.option("Tags", "Red");
    let cases = [
        ("Notes", PropertyValue::Text("hi".to_owned())),
        ("Link", PropertyValue::Url("https://x.dev".to_owned())),
        ("Amount", PropertyValue::Number(3.5)),
        ("Status", PropertyValue::Select(status)),
        ("Tags", PropertyValue::MultiSelect(vec![red])),
        ("Due", date),
        ("Done", PropertyValue::Checkbox(true)),
    ];
    for (property, value) in cases {
        env.set(row, property, Some(value.clone()));
        assert_eq!(env.value(row, property), Some(value));
    }
    let amount = env.property("Amount");
    assert_eq!(
        env.rejected(Command::SetValue {
            row,
            property: amount,
            value: text("x"),
        }),
        DomainError::ValueKindMismatch
    );
    env.set(row, "Amount", None);
    assert_eq!(env.value(row, "Amount"), None);
}

#[test]
fn removing_a_property_removes_its_values_filters_sorts_and_the_board_grouping() {
    let mut env = Env::with_schema();
    let status = env.property("Status");
    let amount = env.property("Amount");
    let done = env.option("Status", "Done");
    let a = env.add_row("a");
    let b = env.add_row("b");
    env.set(a, "Status", Some(PropertyValue::Select(done)));
    env.set(b, "Status", Some(PropertyValue::Select(done)));
    env.set(a, "Amount", Some(PropertyValue::Number(1.0)));
    let main = env.view;
    env.run(Command::SetFilters {
        view: main,
        filters: vec![
            Filter {
                property: status,
                condition: Condition::Select(SelectCondition::Is(done)),
            },
            Filter {
                property: amount,
                condition: Condition::Number(NumberCondition::Gt(0.0)),
            },
        ],
    });
    env.run(Command::SetSorts {
        view: main,
        sorts: vec![
            Sort {
                property: status,
                direction: SortDirection::Ascending,
            },
            Sort {
                property: amount,
                direction: SortDirection::Descending,
            },
        ],
    });
    env.run(Command::SetColumnWidth {
        view: main,
        property: status,
        width: 200,
    });
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Board".to_owned(),
        kind: ViewKind::Board { group_by: status },
    });
    let Some(Created::View(board)) = built.created.first().copied() else {
        panic!("no view");
    };
    let before = env.workspace.clone();
    let (built, inverse) = env
        .try_run(Command::RemoveProperty {
            database: env.database,
            property: status,
        })
        .unwrap();
    assert!(built.created.is_empty());
    assert!(env.workspace.value(a, status).is_none());
    assert!(env.workspace.value(b, status).is_none());
    assert_eq!(env.value(a, "Amount"), Some(PropertyValue::Number(1.0)));
    let table = env.view_of(main);
    assert_eq!(table.filters.len(), 1);
    assert_eq!(table.filters[0].property, amount);
    assert_eq!(table.sorts.len(), 1);
    assert_eq!(table.sorts[0].property, amount);
    assert!(!table.visible.contains(&status));
    assert!(!table.column_widths.contains_key(&status));
    let grouped = env.view_of(board);
    assert_eq!(grouped.kind, ViewKind::Table);
    assert!(!grouped.visible.contains(&status));
    assert!(
        env.workspace
            .database(env.database)
            .unwrap()
            .schema
            .properties
            .iter()
            .all(|property| property.id != status)
    );
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
}

#[test]
fn remove_property_rejects_the_title_and_unknown_properties() {
    let mut env = Env::with_schema();
    let title = env.property("Name");
    assert_eq!(
        env.rejected(Command::RemoveProperty {
            database: env.database,
            property: title
        }),
        DomainError::TitleIsBlockText
    );
    let unknown = PropertyId::new();
    assert_eq!(
        env.rejected(Command::RemoveProperty {
            database: env.database,
            property: unknown
        }),
        DomainError::UnknownProperty(unknown)
    );
}

#[test]
fn add_view_appends_a_view_with_every_property_visible() {
    let mut env = Env::with_schema();
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Second".to_owned(),
        kind: ViewKind::Table,
    });
    let Some(Created::View(second)) = built.created.first().copied() else {
        panic!("no view");
    };
    assert_eq!(built.created.len(), 1);
    let order: Vec<ViewId> = env.workspace.views(env.embed).map(|view| view.id).collect();
    assert_eq!(order, [env.view, second]);
    let view = env.view_of(second);
    assert_eq!(view.name, "Second");
    assert_eq!(view.visible.len(), 8);
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Third".to_owned(),
        kind: ViewKind::Table,
    });
    let Some(Created::View(third)) = built.created.first().copied() else {
        panic!("no view");
    };
    let order: Vec<ViewId> = env.workspace.views(env.embed).map(|view| view.id).collect();
    assert_eq!(order, [env.view, second, third]);
}

#[test]
fn add_view_rejects_a_block_that_is_not_a_database_view_and_a_bad_group_by() {
    let mut env = Env::with_schema();
    assert_eq!(
        env.rejected(Command::AddView {
            block: env.page,
            name: "x".to_owned(),
            kind: ViewKind::Table
        }),
        DomainError::InvalidParent
    );
    let unknown = BlockId::new();
    assert_eq!(
        env.rejected(Command::AddView {
            block: unknown,
            name: "x".to_owned(),
            kind: ViewKind::Table
        }),
        DomainError::UnknownBlock(unknown)
    );
    let notes = env.property("Notes");
    assert_eq!(
        env.rejected(Command::AddView {
            block: env.embed,
            name: "x".to_owned(),
            kind: ViewKind::Board { group_by: notes }
        }),
        DomainError::ValueKindMismatch
    );
}

#[test]
fn view_edit_commands_change_only_their_field() {
    let mut env = Env::with_schema();
    let status = env.property("Status");
    let amount = env.property("Amount");
    let view = env.view;
    env.run(Command::RenameView {
        view,
        name: "Renamed".to_owned(),
    });
    env.run(Command::SetSorts {
        view,
        sorts: vec![Sort {
            property: amount,
            direction: SortDirection::Ascending,
        }],
    });
    env.run(Command::SetVisibleProperties {
        view,
        properties: vec![status],
    });
    env.run(Command::SetColumnWidth {
        view,
        property: status,
        width: 240,
    });
    let filter = Filter {
        property: status,
        condition: Condition::Select(SelectCondition::IsEmpty),
    };
    env.run(Command::SetFilters {
        view,
        filters: vec![filter.clone()],
    });
    env.run(Command::SetViewKind {
        view,
        kind: ViewKind::Board { group_by: status },
    });
    let stored = env.view_of(view);
    assert_eq!(stored.name, "Renamed");
    assert_eq!(stored.sorts.len(), 1);
    assert_eq!(stored.visible, [status]);
    assert_eq!(stored.column_widths.get(&status), Some(&240));
    assert_eq!(stored.filters, [filter]);
    assert_eq!(stored.kind, ViewKind::Board { group_by: status });
    let unknown = ViewId::new();
    assert_eq!(
        env.rejected(Command::RenameView {
            view: unknown,
            name: "x".to_owned()
        }),
        DomainError::UnknownView(unknown)
    );
    let notes = env.property("Notes");
    assert_eq!(
        env.rejected(Command::SetViewKind {
            view,
            kind: ViewKind::Board { group_by: notes }
        }),
        DomainError::ValueKindMismatch
    );
    assert_eq!(
        env.rejected(Command::SetFilters {
            view,
            filters: vec![Filter {
                property: notes,
                condition: Condition::Checkbox(true)
            }],
        }),
        DomainError::ConditionKindMismatch
    );
}

#[test]
fn removing_the_last_view_of_a_block_is_rejected() {
    let mut env = Env::with_schema();
    assert_eq!(
        env.rejected(Command::RemoveView { view: env.view }),
        DomainError::LastView
    );
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Second".to_owned(),
        kind: ViewKind::Table,
    });
    let Some(Created::View(second)) = built.created.first().copied() else {
        panic!("no view");
    };
    env.run(Command::RemoveView { view: second });
    assert_eq!(env.workspace.views(env.embed).count(), 1);
    assert_eq!(
        env.rejected(Command::RemoveView { view: env.view }),
        DomainError::LastView
    );
    let unknown = ViewId::new();
    assert_eq!(
        env.rejected(Command::RemoveView { view: unknown }),
        DomainError::UnknownView(unknown)
    );
}

fn three_rows(env: &mut Env) -> [BlockId; 3] {
    [env.add_row("a"), env.add_row("b"), env.add_row("c")]
}

#[test]
fn move_row_reorders_the_view_and_leaves_other_views_alone() {
    let mut env = Env::with_schema();
    let [a, _, c] = three_rows(&mut env);
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Second".to_owned(),
        kind: ViewKind::Table,
    });
    let Some(Created::View(second)) = built.created.first().copied() else {
        panic!("no view");
    };
    env.run(Command::MoveRow {
        view: env.view,
        row: c,
        before: Some(a),
    });
    assert_eq!(env.titles(), ["c", "a", "b"]);
    assert_eq!(env.titles_of(second), ["a", "b", "c"]);
    env.run(Command::MoveRow {
        view: second,
        row: a,
        before: None,
    });
    assert_eq!(env.titles_of(second), ["b", "c", "a"]);
    assert_eq!(env.titles(), ["c", "a", "b"]);
}

#[test]
fn move_row_to_the_end_and_onto_itself() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    env.run(Command::MoveRow {
        view: env.view,
        row: a,
        before: None,
    });
    assert_eq!(env.titles(), ["b", "c", "a"]);
    env.run(Command::MoveRow {
        view: env.view,
        row: b,
        before: Some(b),
    });
    assert_eq!(env.titles(), ["b", "c", "a"]);
}

#[test]
fn move_row_works_on_the_filtered_order() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    env.set(a, "Done", Some(PropertyValue::Checkbox(true)));
    env.set(c, "Done", Some(PropertyValue::Checkbox(true)));
    let done = env.property("Done");
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![Filter {
            property: done,
            condition: Condition::Checkbox(true),
        }],
    });
    env.run(Command::MoveRow {
        view: env.view,
        row: c,
        before: Some(a),
    });
    assert_eq!(env.titles(), ["c", "a"]);
    env.run(Command::SetFilters {
        view: env.view,
        filters: Vec::new(),
    });
    assert_eq!(env.titles(), ["c", "a", "b"]);
    let _ = b;
}

#[test]
fn move_row_with_sorts_is_rejected_and_changes_nothing() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    let amount = env.property("Amount");
    env.run(Command::SetSorts {
        view: env.view,
        sorts: vec![Sort {
            property: amount,
            direction: SortDirection::Ascending,
        }],
    });
    assert_eq!(
        env.rejected(Command::MoveRow {
            view: env.view,
            row: a,
            before: Some(b)
        }),
        DomainError::ManualOrderWithSorts
    );
}

#[test]
fn move_row_rejects_rows_outside_the_view() {
    let mut env = Env::with_schema();
    let [a, _, _] = three_rows(&mut env);
    let unknown = BlockId::new();
    assert_eq!(
        env.rejected(Command::MoveRow {
            view: env.view,
            row: unknown,
            before: None
        }),
        DomainError::UnknownBlock(unknown)
    );
    assert_eq!(
        env.rejected(Command::MoveRow {
            view: env.view,
            row: a,
            before: Some(unknown)
        }),
        DomainError::UnknownBlock(unknown)
    );
    let unknown_view = ViewId::new();
    assert_eq!(
        env.rejected(Command::MoveRow {
            view: unknown_view,
            row: a,
            before: None
        }),
        DomainError::UnknownView(unknown_view)
    );
}

fn board(env: &mut Env) -> ViewId {
    let status = env.property("Status");
    let built = env.run(Command::AddView {
        block: env.embed,
        name: "Board".to_owned(),
        kind: ViewKind::Board { group_by: status },
    });
    let Some(Created::View(view)) = built.created.first().copied() else {
        panic!("no view");
    };
    view
}

#[test]
fn move_card_to_another_group_sets_the_value_and_positions_the_card() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    let board = board(&mut env);
    let done = env.option("Status", "Done");
    env.set(b, "Status", Some(PropertyValue::Select(done)));
    env.set(c, "Status", Some(PropertyValue::Select(done)));
    env.run(Command::MoveCard {
        view: board,
        row: a,
        group: Some(done),
        before: Some(c),
    });
    assert_eq!(env.value(a, "Status"), Some(PropertyValue::Select(done)));
    let groups = env.workspace.board_groups(board).unwrap();
    let done_group = groups
        .iter()
        .find(|group| group.option == Some(done))
        .unwrap();
    assert_eq!(done_group.rows, [b, a, c]);
    env.run(Command::MoveCard {
        view: board,
        row: a,
        group: Some(done),
        before: None,
    });
    let groups = env.workspace.board_groups(board).unwrap();
    let done_group = groups
        .iter()
        .find(|group| group.option == Some(done))
        .unwrap();
    assert_eq!(done_group.rows, [b, c, a]);
}

#[test]
fn move_card_to_the_empty_group_clears_the_value() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    let board = board(&mut env);
    let todo = env.option("Status", "Todo");
    env.set(a, "Status", Some(PropertyValue::Select(todo)));
    env.run(Command::MoveCard {
        view: board,
        row: a,
        group: None,
        before: Some(b),
    });
    assert_eq!(env.value(a, "Status"), None);
    let groups = env.workspace.board_groups(board).unwrap();
    assert_eq!(groups[0].rows.first(), Some(&a));
}

#[test]
fn move_card_within_a_group_reorders_without_touching_the_value() {
    let mut env = Env::with_schema();
    let [a, _, c] = three_rows(&mut env);
    let board = board(&mut env);
    let built = env.run(Command::MoveCard {
        view: board,
        row: c,
        group: None,
        before: Some(a),
    });
    assert_eq!(built.transaction.operations().len(), 1);
    let groups = env.workspace.board_groups(board).unwrap();
    assert_eq!(groups[0].rows.first(), Some(&c));
}

#[test]
fn move_card_with_sorts_sets_the_value_but_does_not_reorder() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    let board = board(&mut env);
    let amount = env.property("Amount");
    env.run(Command::SetSorts {
        view: board,
        sorts: vec![Sort {
            property: amount,
            direction: SortDirection::Ascending,
        }],
    });
    let done = env.option("Status", "Done");
    let built = env.run(Command::MoveCard {
        view: board,
        row: a,
        group: Some(done),
        before: None,
    });
    assert_eq!(built.transaction.operations().len(), 1);
    assert!(env.view_of(board).manual_order.is_empty());
    assert_eq!(env.value(a, "Status"), Some(PropertyValue::Select(done)));
    assert_eq!(
        env.rejected(Command::MoveCard {
            view: board,
            row: b,
            group: None,
            before: None
        }),
        DomainError::ManualOrderWithSorts
    );
}

#[test]
fn move_card_rejects_a_table_view_and_bad_targets() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    assert_eq!(
        env.rejected(Command::MoveCard {
            view: env.view,
            row: a,
            group: None,
            before: None
        }),
        DomainError::ValueKindMismatch
    );
    let board = board(&mut env);
    let unknown = OptionId::new();
    assert_eq!(
        env.rejected(Command::MoveCard {
            view: board,
            row: a,
            group: Some(unknown),
            before: None
        }),
        DomainError::UnknownOption(unknown)
    );
    let done = env.option("Status", "Done");
    assert_eq!(
        env.rejected(Command::MoveCard {
            view: board,
            row: a,
            group: Some(done),
            before: Some(b)
        }),
        DomainError::UnknownBlock(b)
    );
    let missing = BlockId::new();
    assert_eq!(
        env.rejected(Command::MoveCard {
            view: board,
            row: missing,
            group: None,
            before: None
        }),
        DomainError::UnknownBlock(missing)
    );
}

#[test]
fn two_view_blocks_of_one_database_keep_separate_filters_and_share_values() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    let second = env.run(Command::InsertDatabaseView {
        after: env.embed,
        database: env.database,
    });
    let Some(Created::View(other)) = second.created.get(1).copied() else {
        panic!("no view");
    };
    let done = env.property("Done");
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![Filter {
            property: done,
            condition: Condition::Checkbox(true),
        }],
    });
    assert!(env.view_of(other).filters.is_empty());
    env.set(a, "Done", Some(PropertyValue::Checkbox(true)));
    assert_eq!(env.titles(), ["a"]);
    assert_eq!(env.titles_of(other), ["a", "b", "c"]);
    env.set(b, "Done", Some(PropertyValue::Checkbox(true)));
    assert_eq!(env.titles(), ["a", "b"]);
}

#[test]
fn changing_text_to_number_keeps_numbers_drops_the_rest_and_undo_restores_both() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    env.set(a, "Notes", text("42"));
    env.set(b, "Notes", text("abc"));
    env.set(c, "Notes", text(" 3.5 "));
    let before = env.workspace.clone();
    let notes = env.property("Notes");
    let (built, inverse) = env
        .try_run(Command::ChangePropertyKind {
            database: env.database,
            property: notes,
            kind: PropertyKind::Number {
                format: NumberFormat::Plain,
            },
        })
        .unwrap();
    assert!(built.created.is_empty());
    assert_eq!(env.value(a, "Notes"), Some(PropertyValue::Number(42.0)));
    assert_eq!(env.value(b, "Notes"), None);
    assert_eq!(env.value(c, "Notes"), Some(PropertyValue::Number(3.5)));
    assert_eq!(
        env.kind("Notes"),
        PropertyKind::Number {
            format: NumberFormat::Plain
        }
    );
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
    assert_eq!(env.value(a, "Notes"), text("42"));
    assert_eq!(env.value(b, "Notes"), text("abc"));
}

#[test]
fn text_to_number_drops_non_finite_numbers() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    env.set(a, "Notes", text("NaN"));
    env.set(b, "Notes", text("inf"));
    env.set(c, "Notes", text(""));
    let notes = env.property("Notes");
    env.run(Command::ChangePropertyKind {
        database: env.database,
        property: notes,
        kind: PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    });
    for row in [a, b, c] {
        assert_eq!(env.value(row, "Notes"), None);
    }
}

#[test]
fn url_converts_to_number_like_text() {
    let mut env = Env::with_schema();
    let a = env.add_row("a");
    env.set(a, "Link", Some(PropertyValue::Url("7".to_owned())));
    let link = env.property("Link");
    env.run(Command::ChangePropertyKind {
        database: env.database,
        property: link,
        kind: PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    });
    assert_eq!(env.value(a, "Link"), Some(PropertyValue::Number(7.0)));
}

fn change(env: &mut Env, property: &str, kind: PropertyKind) {
    let command = Command::ChangePropertyKind {
        database: env.database,
        property: env.property(property),
        kind,
    };
    env.run(command);
}

#[test]
fn anything_converts_to_text_by_stringifying() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    let done = env.option("Status", "Done");
    let todo = env.option("Status", "Todo");
    let red = env.option("Tags", "Red");
    let blue = env.option("Tags", "Blue");
    env.set(a, "Amount", Some(PropertyValue::Number(12.5)));
    env.set(b, "Amount", Some(PropertyValue::Number(3.0)));
    env.set(a, "Status", Some(PropertyValue::Select(done)));
    env.set(b, "Status", Some(PropertyValue::Select(todo)));
    env.set(a, "Tags", Some(PropertyValue::MultiSelect(vec![red, blue])));
    env.set(
        a,
        "Due",
        Some(PropertyValue::Date(CalendarDate::new(2026, 3, 9).unwrap())),
    );
    env.set(a, "Done", Some(PropertyValue::Checkbox(true)));
    env.set(b, "Done", Some(PropertyValue::Checkbox(false)));
    env.set(
        a,
        "Link",
        Some(PropertyValue::Url("https://x.dev".to_owned())),
    );
    env.set(c, "Notes", text(""));
    change(&mut env, "Amount", PropertyKind::Text);
    change(&mut env, "Status", PropertyKind::Text);
    change(&mut env, "Tags", PropertyKind::Text);
    change(&mut env, "Due", PropertyKind::Text);
    change(&mut env, "Done", PropertyKind::Text);
    change(&mut env, "Link", PropertyKind::Text);
    change(&mut env, "Notes", PropertyKind::Url);
    assert_eq!(env.value(a, "Amount"), text("12.5"));
    assert_eq!(env.value(b, "Amount"), text("3"));
    assert_eq!(env.value(a, "Status"), text("Done"));
    assert_eq!(env.value(b, "Status"), text("Todo"));
    assert_eq!(env.value(a, "Tags"), text("Red, Blue"));
    assert_eq!(env.value(a, "Due"), text("2026-03-09"));
    assert_eq!(env.value(a, "Done"), text("true"));
    assert_eq!(env.value(b, "Done"), None);
    assert_eq!(env.value(a, "Link"), text("https://x.dev"));
    assert_eq!(env.value(c, "Notes"), None);
}

#[test]
fn text_and_url_convert_into_each_other_unchanged() {
    let mut env = Env::with_schema();
    let a = env.add_row("a");
    env.set(a, "Notes", text("same"));
    env.set(a, "Link", Some(PropertyValue::Url("also".to_owned())));
    change(&mut env, "Notes", PropertyKind::Url);
    change(&mut env, "Link", PropertyKind::Text);
    assert_eq!(
        env.value(a, "Notes"),
        Some(PropertyValue::Url("same".to_owned()))
    );
    assert_eq!(env.value(a, "Link"), text("also"));
}

#[test]
fn text_converts_to_select_with_one_option_per_distinct_trimmed_value() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    let d = env.add_row("d");
    let e = env.add_row("e");
    env.set(a, "Notes", text("Red"));
    env.set(b, "Notes", text(" Blue "));
    env.set(c, "Notes", text("Red"));
    env.set(d, "Notes", text("   "));
    let before = env.workspace.clone();
    let notes = env.property("Notes");
    let (_, inverse) = env
        .try_run(Command::ChangePropertyKind {
            database: env.database,
            property: notes,
            kind: PropertyKind::Select {
                options: Vec::new(),
            },
        })
        .unwrap();
    let options = env.kind("Notes").options().unwrap().to_vec();
    let names: Vec<&str> = options.iter().map(|option| option.name.as_str()).collect();
    assert_eq!(names, ["Red", "Blue"]);
    assert!(
        options
            .iter()
            .all(|option| option.color == OptionColor::Default)
    );
    assert_eq!(
        env.value(a, "Notes"),
        Some(PropertyValue::Select(options[0].id))
    );
    assert_eq!(
        env.value(b, "Notes"),
        Some(PropertyValue::Select(options[1].id))
    );
    assert_eq!(
        env.value(c, "Notes"),
        Some(PropertyValue::Select(options[0].id))
    );
    assert_eq!(env.value(d, "Notes"), None);
    assert_eq!(env.value(e, "Notes"), None);
    env.workspace.apply(&inverse).unwrap();
    assert_eq!(env.workspace, before);
}

#[test]
fn select_and_multi_select_convert_into_each_other_keeping_options() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    let todo = env.option("Status", "Todo");
    let done = env.option("Status", "Done");
    env.set(a, "Status", Some(PropertyValue::Select(todo)));
    env.set(b, "Status", Some(PropertyValue::Select(done)));
    let options = env.kind("Status").options().unwrap().to_vec();
    change(
        &mut env,
        "Status",
        PropertyKind::MultiSelect {
            options: Vec::new(),
        },
    );
    assert_eq!(env.kind("Status").options().unwrap(), options.as_slice());
    assert_eq!(
        env.value(a, "Status"),
        Some(PropertyValue::MultiSelect(vec![todo]))
    );
    assert_eq!(env.value(c, "Status"), None);
    let red = env.option("Tags", "Red");
    let blue = env.option("Tags", "Blue");
    env.set(a, "Tags", Some(PropertyValue::MultiSelect(vec![blue, red])));
    env.set(b, "Tags", Some(PropertyValue::MultiSelect(Vec::new())));
    let tag_options = env.kind("Tags").options().unwrap().to_vec();
    change(
        &mut env,
        "Tags",
        PropertyKind::Select {
            options: Vec::new(),
        },
    );
    assert_eq!(env.kind("Tags").options().unwrap(), tag_options.as_slice());
    assert_eq!(env.value(a, "Tags"), Some(PropertyValue::Select(blue)));
    assert_eq!(env.value(b, "Tags"), None);
}

#[test]
fn every_other_pair_drops_the_values() {
    let mut env = Env::with_schema();
    let a = env.add_row("a");
    env.set(a, "Amount", Some(PropertyValue::Number(1.0)));
    env.set(
        a,
        "Due",
        Some(PropertyValue::Date(CalendarDate::new(2026, 1, 1).unwrap())),
    );
    env.set(a, "Done", Some(PropertyValue::Checkbox(true)));
    env.set(a, "Notes", text("x"));
    let todo = env.option("Status", "Todo");
    env.set(a, "Status", Some(PropertyValue::Select(todo)));
    change(&mut env, "Amount", PropertyKind::Date);
    change(&mut env, "Due", PropertyKind::Checkbox);
    change(
        &mut env,
        "Done",
        PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    );
    change(&mut env, "Notes", PropertyKind::Checkbox);
    change(&mut env, "Status", PropertyKind::Date);
    for property in ["Amount", "Due", "Done", "Notes", "Status"] {
        assert_eq!(env.value(a, property), None, "{property}");
    }
}

#[test]
fn converting_to_the_same_kind_keeps_the_values() {
    let mut env = Env::with_schema();
    let a = env.add_row("a");
    env.set(a, "Amount", Some(PropertyValue::Number(1.0)));
    env.set(a, "Done", Some(PropertyValue::Checkbox(true)));
    let due = PropertyValue::Date(CalendarDate::new(2026, 1, 1).unwrap());
    env.set(a, "Due", Some(due.clone()));
    let todo = env.option("Status", "Todo");
    env.set(a, "Status", Some(PropertyValue::Select(todo)));
    change(
        &mut env,
        "Amount",
        PropertyKind::Number {
            format: NumberFormat::Percent,
        },
    );
    change(&mut env, "Done", PropertyKind::Checkbox);
    change(&mut env, "Due", PropertyKind::Date);
    change(
        &mut env,
        "Status",
        PropertyKind::Select {
            options: Vec::new(),
        },
    );
    assert_eq!(env.value(a, "Amount"), Some(PropertyValue::Number(1.0)));
    assert_eq!(env.value(a, "Done"), Some(PropertyValue::Checkbox(true)));
    assert_eq!(env.value(a, "Due"), Some(due));
    assert_eq!(env.value(a, "Status"), Some(PropertyValue::Select(todo)));
}

#[test]
fn changing_a_kind_drops_filters_that_no_longer_fit_and_ungroups_boards() {
    let mut env = Env::with_schema();
    let board = board(&mut env);
    let status = env.property("Status");
    let notes = env.property("Notes");
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![
            Filter {
                property: status,
                condition: Condition::Select(SelectCondition::IsEmpty),
            },
            Filter {
                property: notes,
                condition: Condition::Text(TextCondition::IsEmpty),
            },
        ],
    });
    change(
        &mut env,
        "Status",
        PropertyKind::MultiSelect {
            options: Vec::new(),
        },
    );
    assert_eq!(env.view_of(board).kind, ViewKind::Table);
    let filters = env.view_of(env.view).filters;
    assert_eq!(filters.len(), 1);
    assert_eq!(filters[0].property, notes);
    change(&mut env, "Notes", PropertyKind::Url);
    assert_eq!(env.view_of(env.view).filters.len(), 1);
    change(
        &mut env,
        "Notes",
        PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    );
    assert!(env.view_of(env.view).filters.is_empty());
}

#[test]
fn a_board_stays_a_board_when_the_group_property_stays_a_select() {
    let mut env = Env::with_schema();
    let board = board(&mut env);
    let status = env.property("Status");
    change(
        &mut env,
        "Status",
        PropertyKind::Select {
            options: Vec::new(),
        },
    );
    assert_eq!(
        env.view_of(board).kind,
        ViewKind::Board { group_by: status }
    );
}

#[test]
fn change_property_kind_rejects_the_title_in_either_position() {
    let mut env = Env::with_schema();
    let title = env.property("Name");
    let notes = env.property("Notes");
    assert_eq!(
        env.rejected(Command::ChangePropertyKind {
            database: env.database,
            property: title,
            kind: PropertyKind::Text
        }),
        DomainError::TitleIsBlockText
    );
    assert_eq!(
        env.rejected(Command::ChangePropertyKind {
            database: env.database,
            property: notes,
            kind: PropertyKind::Title
        }),
        DomainError::TitleIsBlockText
    );
}

#[test]
fn change_property_kind_undo_restores_filters_boards_and_values() {
    let mut env = Env::with_schema();
    let board = board(&mut env);
    let [a, _, _] = three_rows(&mut env);
    let todo = env.option("Status", "Todo");
    env.set(a, "Status", Some(PropertyValue::Select(todo)));
    let status = env.property("Status");
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![Filter {
            property: status,
            condition: Condition::Select(SelectCondition::Is(todo)),
        }],
    });
    let _ = board;
    assert_undo_restores(
        Command::ChangePropertyKind {
            database: env.database,
            property: status,
            kind: PropertyKind::Text,
        },
        &mut env,
    );
}

#[test]
fn multi_select_filters_and_manual_order_survive_unrelated_commands() {
    let mut env = Env::with_schema();
    let [a, b, c] = three_rows(&mut env);
    let tags = env.property("Tags");
    let red = env.option("Tags", "Red");
    env.set(a, "Tags", Some(PropertyValue::MultiSelect(vec![red])));
    env.set(c, "Tags", Some(PropertyValue::MultiSelect(vec![red])));
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![Filter {
            property: tags,
            condition: Condition::MultiSelect(MultiSelectCondition::Contains(red)),
        }],
    });
    env.run(Command::MoveRow {
        view: env.view,
        row: c,
        before: Some(a),
    });
    env.run(Command::AddProperty {
        database: env.database,
        name: "Extra".to_owned(),
        kind: PropertyKind::Text,
    });
    assert_eq!(env.titles(), ["c", "a"]);
    let _ = b;
}

fn deleted_database() -> Env {
    let mut env = Env::with_schema();
    env.add_row("a");
    env.run(Command::DeleteDatabase {
        database: env.database,
    });
    env
}

#[test]
fn add_row_to_a_deleted_database_is_rejected() {
    let mut env = Env::bare();
    let built = env.run(Command::CreateDatabase);
    let Some(Created::Database(database)) = built.created.first().copied() else {
        panic!("no database created");
    };
    env.run(Command::DeleteDatabase { database });
    assert_eq!(
        env.rejected(Command::AddRow {
            database,
            values: Vec::new()
        }),
        DomainError::UnknownDatabase(database)
    );
    assert_eq!(env.workspace.rows(database).count(), 0);
}

#[test]
fn commands_naming_a_deleted_database_are_unknown_database() {
    let mut env = deleted_database();
    let database = env.database;
    let notes = env.property("Notes");
    let status = env.property("Status");
    let todo = env.option("Status", "Todo");
    let commands = vec![
        Command::AddRow {
            database,
            values: Vec::new(),
        },
        Command::AddProperty {
            database,
            name: "x".to_owned(),
            kind: PropertyKind::Text,
        },
        Command::RenameProperty {
            database,
            property: notes,
            name: "x".to_owned(),
        },
        Command::ChangePropertyKind {
            database,
            property: notes,
            kind: PropertyKind::Url,
        },
        Command::RemoveProperty {
            database,
            property: notes,
        },
        Command::MoveProperty {
            database,
            property: notes,
            index: 1,
        },
        Command::AddSelectOption {
            database,
            property: status,
            name: "x".to_owned(),
            color: OptionColor::Default,
        },
        Command::RenameSelectOption {
            database,
            property: status,
            option: todo,
            name: "x".to_owned(),
        },
        Command::RemoveSelectOption {
            database,
            property: status,
            option: todo,
        },
        Command::RenameDatabase {
            database,
            name: "x".to_owned(),
        },
        Command::SetDatabaseIcon {
            database,
            icon: None,
        },
        Command::InsertDatabaseView {
            after: env.intro,
            database,
        },
        Command::AddView {
            block: env.embed,
            name: "x".to_owned(),
            kind: ViewKind::Table,
        },
    ];
    for command in commands {
        let label = format!("{command:?}");
        assert_eq!(
            env.rejected(command),
            DomainError::UnknownDatabase(database),
            "{label}"
        );
    }
}

#[test]
fn set_value_on_a_dead_row_is_unknown_block() {
    let mut env = Env::with_schema();
    let row = env.add_row("a");
    env.run(Command::DeleteBlocks { blocks: vec![row] });
    let property = env.property("Notes");
    assert_eq!(
        env.rejected(Command::SetValue {
            row,
            property,
            value: text("x"),
        }),
        DomainError::UnknownBlock(row)
    );
}

#[test]
fn view_commands_on_a_view_of_a_dead_block_are_unknown_view() {
    let mut env = Env::with_schema();
    let [a, b, _] = three_rows(&mut env);
    env.run(Command::AddView {
        block: env.embed,
        name: "Second".to_owned(),
        kind: ViewKind::Table,
    });
    let status = env.property("Status");
    let view = env.view;
    env.run(Command::DeleteBlocks {
        blocks: vec![env.embed],
    });
    let commands = vec![
        Command::RenameView {
            view,
            name: "x".to_owned(),
        },
        Command::RemoveView { view },
        Command::SetFilters {
            view,
            filters: Vec::new(),
        },
        Command::SetSorts {
            view,
            sorts: Vec::new(),
        },
        Command::SetVisibleProperties {
            view,
            properties: Vec::new(),
        },
        Command::SetColumnWidth {
            view,
            property: status,
            width: 10,
        },
        Command::SetViewKind {
            view,
            kind: ViewKind::Table,
        },
        Command::MoveRow {
            view,
            row: a,
            before: Some(b),
        },
        Command::MoveCard {
            view,
            row: a,
            group: None,
            before: None,
        },
    ];
    for command in commands {
        let label = format!("{command:?}");
        assert_eq!(
            env.rejected(command),
            DomainError::UnknownView(view),
            "{label}"
        );
    }
}

#[test]
fn view_blocks_inserted_after_a_dead_block_are_unknown_block() {
    let mut env = Env::new();
    let intro = env.intro;
    env.run(Command::DeleteBlocks {
        blocks: vec![intro],
    });
    assert_eq!(
        env.rejected(Command::CreateDatabaseWithView { after: intro }),
        DomainError::UnknownBlock(intro)
    );
    assert_eq!(
        env.rejected(Command::InsertDatabaseView {
            after: intro,
            database: env.database
        }),
        DomainError::UnknownBlock(intro)
    );
}

#[test]
fn delete_database_on_a_deleted_database_is_an_empty_transaction() {
    let mut env = deleted_database();
    let before = env.workspace.clone();
    let built = env.run(Command::DeleteDatabase {
        database: env.database,
    });
    assert!(built.transaction.is_empty());
    assert_eq!(env.workspace, before);
    let unknown = DatabaseId::new();
    assert_eq!(
        env.rejected(Command::DeleteDatabase { database: unknown }),
        DomainError::UnknownDatabase(unknown)
    );
}

#[test]
fn set_filters_rejects_non_finite_number_targets() {
    let mut env = Env::with_schema();
    let amount = env.property("Amount");
    let constructors: [fn(f64) -> NumberCondition; 6] = [
        NumberCondition::Eq,
        NumberCondition::Ne,
        NumberCondition::Lt,
        NumberCondition::Le,
        NumberCondition::Gt,
        NumberCondition::Ge,
    ];
    for constructor in constructors {
        for target in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let command = Command::SetFilters {
                view: env.view,
                filters: vec![Filter {
                    property: amount,
                    condition: Condition::Number(constructor(target)),
                }],
            };
            assert_eq!(env.rejected(command), DomainError::ValueKindMismatch);
        }
    }
    env.run(Command::SetFilters {
        view: env.view,
        filters: vec![
            Filter {
                property: amount,
                condition: Condition::Number(NumberCondition::Eq(2.5)),
            },
            Filter {
                property: amount,
                condition: Condition::Number(NumberCondition::IsEmpty),
            },
        ],
    });
    assert_eq!(env.view_of(env.view).filters.len(), 2);
}
