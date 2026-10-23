use proptest::prelude::*;

use crate::application::{Built, Command, Created};
use crate::domain::{
    Block, BlockId, BlockKind, CalendarDate, Condition, DatabaseId, DateCondition, DomainError,
    Filter, MultiSelectCondition, NumberCondition, NumberFormat, Operation, OptionColor, OptionId,
    Parent, Property, PropertyKind, PropertyValue, RichText, SelectCondition, SelectOption, Sort,
    SortDirection, SortKey, TextCondition, Timestamp, Transaction, ViewId, ViewKind, Workspace,
    WorkspaceSnapshot,
};

use super::build;

fn now() -> Timestamp {
    Timestamp::from_millis(5_000)
}

fn run(workspace: &mut Workspace, command: Command) -> Built {
    let built = build(workspace, &command, now()).unwrap();
    workspace.apply(&built.transaction).unwrap();
    built
}

fn created_view(built: &Built) -> ViewId {
    built
        .created
        .iter()
        .find_map(|created| match created {
            Created::View(view) => Some(*view),
            _ => None,
        })
        .unwrap()
}

fn property_named(workspace: &Workspace, database: DatabaseId, name: &str) -> Property {
    workspace
        .database(database)
        .unwrap()
        .schema
        .properties
        .iter()
        .find(|property| property.name == name)
        .unwrap()
        .clone()
}

fn seed() -> Workspace {
    let page = BlockId::new();
    let intro = BlockId::new();
    let blocks = vec![
        Block {
            id: page,
            kind: BlockKind::Page { icon: None },
            parent: Parent::Root,
            sort_key: SortKey::between(None, None),
            text: RichText::plain("Home"),
            alive: true,
            created_at: now(),
        },
        Block {
            id: intro,
            kind: BlockKind::Text,
            parent: Parent::Block(page),
            sort_key: SortKey::between(None, None),
            text: RichText::plain("intro"),
            alive: true,
            created_at: now(),
        },
    ];
    let mut workspace = Workspace::from_snapshot(WorkspaceSnapshot {
        blocks,
        ..WorkspaceSnapshot::default()
    })
    .unwrap();
    let built = run(
        &mut workspace,
        Command::CreateDatabaseWithView { after: intro },
    );
    let (database, embed, view) = match built.created[..] {
        [
            Created::Database(database),
            Created::Block(embed),
            Created::View(view),
        ] => (database, embed, view),
        _ => panic!("created order is database, block, view"),
    };
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
        run(
            &mut workspace,
            Command::AddProperty {
                database,
                name: name.to_owned(),
                kind,
            },
        );
    }
    for (property, names) in [
        ("Status", ["Todo", "Doing", "Done"]),
        ("Tags", ["Red", "Blue", "Green"]),
    ] {
        let property = property_named(&workspace, database, property).id;
        for name in names {
            run(
                &mut workspace,
                Command::AddSelectOption {
                    database,
                    property,
                    name: name.to_owned(),
                    color: OptionColor::Default,
                },
            );
        }
    }
    let status = property_named(&workspace, database, "Status");
    let amount = property_named(&workspace, database, "Amount");
    let todo = status.kind.options().unwrap()[0].id;
    for (n, title) in ["a", "b", "c", "d"].into_iter().enumerate() {
        let built = run(
            &mut workspace,
            Command::AddRow {
                database,
                values: vec![
                    (status.id, PropertyValue::Select(todo)),
                    (amount.id, PropertyValue::Number(n as f64)),
                ],
            },
        );
        let Some(Created::Block(row)) = built.created.first().copied() else {
            panic!("no row");
        };
        run(
            &mut workspace,
            Command::EditText {
                block: row,
                text: RichText::plain(title),
            },
        );
    }
    let board = run(
        &mut workspace,
        Command::AddView {
            block: embed,
            name: "Board".to_owned(),
            kind: ViewKind::Board {
                group_by: status.id,
            },
        },
    );
    let board = created_view(&board);
    run(
        &mut workspace,
        Command::SetFilters {
            view: board,
            filters: vec![Filter {
                property: amount.id,
                condition: Condition::Number(NumberCondition::Ge(1.0)),
            }],
        },
    );
    run(
        &mut workspace,
        Command::InsertDatabaseView {
            after: embed,
            database,
        },
    );
    run(&mut workspace, Command::CreateDatabase);
    let _ = view;
    workspace
}

fn databases(workspace: &Workspace) -> Vec<DatabaseId> {
    let mut all: Vec<_> = workspace
        .databases
        .values()
        .map(|database| (database.sort_key.clone(), database.id))
        .collect();
    all.sort();
    all.into_iter().map(|(_, id)| id).collect()
}

fn rows(workspace: &Workspace, database: DatabaseId) -> Vec<BlockId> {
    workspace
        .children
        .get(&Parent::Database(database))
        .into_iter()
        .flatten()
        .map(|(_, id)| *id)
        .collect()
}

fn view_blocks(workspace: &Workspace) -> Vec<BlockId> {
    let mut all: Vec<BlockId> = workspace
        .blocks
        .values()
        .filter(|block| matches!(block.kind, BlockKind::DatabaseView { .. }))
        .map(|block| block.id)
        .collect();
    all.sort();
    all
}

fn views(workspace: &Workspace) -> Vec<ViewId> {
    let mut all: Vec<ViewId> = workspace.views.keys().copied().collect();
    all.sort();
    all
}

fn pick<T>(items: &[T], index: usize) -> Option<&T> {
    (!items.is_empty()).then(|| &items[index % items.len()])
}

fn pick_id<T: Copy>(items: &[T], index: usize) -> Option<T> {
    pick(items, index).copied()
}

fn schema(workspace: &Workspace, database: DatabaseId) -> Vec<Property> {
    workspace
        .database(database)
        .map(|found| found.schema.properties.clone())
        .unwrap_or_default()
}

fn view_schema(workspace: &Workspace, view: ViewId) -> Vec<Property> {
    workspace
        .view(view)
        .and_then(|found| workspace.view_database(found))
        .map(|found| found.schema.properties.clone())
        .unwrap_or_default()
}

fn kind_for(seed: u8) -> PropertyKind {
    match seed % 8 {
        0 => PropertyKind::Title,
        1 => PropertyKind::Text,
        2 => PropertyKind::Number {
            format: NumberFormat::Plain,
        },
        3 => PropertyKind::Select {
            options: Vec::new(),
        },
        4 => PropertyKind::MultiSelect {
            options: Vec::new(),
        },
        5 => PropertyKind::Date,
        6 => PropertyKind::Checkbox,
        _ => PropertyKind::Url,
    }
}

fn options_of(kind: &PropertyKind) -> &[SelectOption] {
    kind.options().unwrap_or_default()
}

fn value_for(kind: &PropertyKind, seed: u8) -> PropertyValue {
    if seed % 16 == 15 {
        return match kind {
            PropertyKind::Checkbox => PropertyValue::Text("wrong".to_owned()),
            _ => PropertyValue::Checkbox(true),
        };
    }
    match kind {
        PropertyKind::Title | PropertyKind::Text => PropertyValue::Text(format!("t{}", seed % 4)),
        PropertyKind::Url => PropertyValue::Url(format!("u{}", seed % 4)),
        PropertyKind::Number { .. } if seed == 254 => PropertyValue::Number(f64::NAN),
        PropertyKind::Number { .. } => PropertyValue::Number(f64::from(seed) / 4.0),
        PropertyKind::Select { options } => match pick(options, usize::from(seed)) {
            Some(option) => PropertyValue::Select(option.id),
            None => PropertyValue::Select(OptionId::new()),
        },
        PropertyKind::MultiSelect { options } => PropertyValue::MultiSelect(
            options
                .iter()
                .take(usize::from(seed % 3) + 1)
                .map(|option| option.id)
                .collect(),
        ),
        PropertyKind::Date => CalendarDate::new(2026, 1 + seed % 12, 1 + seed % 28)
            .map_or(PropertyValue::Checkbox(false), PropertyValue::Date),
        PropertyKind::Checkbox => PropertyValue::Checkbox(seed.is_multiple_of(2)),
    }
}

fn condition_for(kind: &PropertyKind, seed: u8) -> Condition {
    if seed >= 240 {
        return Condition::Checkbox(true);
    }
    let option = options_of(kind)
        .first()
        .map_or_else(OptionId::new, |option| option.id);
    let day = CalendarDate::new(2026, 6, 15).unwrap();
    match kind {
        PropertyKind::Title | PropertyKind::Text | PropertyKind::Url => {
            Condition::Text(match seed % 5 {
                0 => TextCondition::Contains("t1".to_owned()),
                1 => TextCondition::DoesNotContain("t1".to_owned()),
                2 => TextCondition::Is("t2".to_owned()),
                3 => TextCondition::IsEmpty,
                _ => TextCondition::IsNotEmpty,
            })
        }
        PropertyKind::Number { .. } => Condition::Number(match seed % 9 {
            0 => NumberCondition::Eq(1.0),
            1 => NumberCondition::Ne(1.0),
            2 => NumberCondition::Lt(2.0),
            3 => NumberCondition::Le(2.0),
            4 => NumberCondition::Gt(0.5),
            5 => NumberCondition::Ge(0.5),
            6 => NumberCondition::IsEmpty,
            7 => NumberCondition::IsNotEmpty,
            _ => NumberCondition::Eq(f64::NAN),
        }),
        PropertyKind::Select { .. } => Condition::Select(match seed % 4 {
            0 => SelectCondition::Is(option),
            1 => SelectCondition::IsNot(option),
            2 => SelectCondition::IsEmpty,
            _ => SelectCondition::IsNotEmpty,
        }),
        PropertyKind::MultiSelect { .. } => Condition::MultiSelect(match seed % 4 {
            0 => MultiSelectCondition::Contains(option),
            1 => MultiSelectCondition::DoesNotContain(option),
            2 => MultiSelectCondition::IsEmpty,
            _ => MultiSelectCondition::IsNotEmpty,
        }),
        PropertyKind::Date => Condition::Date(match seed % 5 {
            0 => DateCondition::Is(day),
            1 => DateCondition::Before(day),
            2 => DateCondition::After(day),
            3 => DateCondition::IsEmpty,
            _ => DateCondition::IsNotEmpty,
        }),
        PropertyKind::Checkbox => Condition::Checkbox(seed.is_multiple_of(2)),
    }
}

#[derive(Clone, Debug)]
enum Step {
    CreateDatabase,
    CreateDatabaseWithView {
        block: usize,
    },
    InsertDatabaseView {
        block: usize,
        database: usize,
    },
    RenameDatabase {
        database: usize,
    },
    DeleteDatabase {
        database: usize,
    },
    AddProperty {
        database: usize,
        kind: u8,
    },
    RenameProperty {
        database: usize,
        property: usize,
    },
    ChangePropertyKind {
        database: usize,
        property: usize,
        kind: u8,
    },
    RemoveProperty {
        database: usize,
        property: usize,
    },
    MoveProperty {
        database: usize,
        property: usize,
        index: usize,
    },
    AddSelectOption {
        database: usize,
        property: usize,
    },
    RenameSelectOption {
        database: usize,
        property: usize,
        option: usize,
    },
    RemoveSelectOption {
        database: usize,
        property: usize,
        option: usize,
    },
    AddRow {
        database: usize,
        values: Vec<(usize, u8)>,
    },
    SetValue {
        database: usize,
        row: usize,
        property: usize,
        seed: u8,
        clear: bool,
    },
    AddView {
        block: usize,
        property: Option<usize>,
    },
    RemoveView {
        view: usize,
    },
    SetFilters {
        view: usize,
        filters: Vec<(usize, u8)>,
    },
    SetSorts {
        view: usize,
        sorts: Vec<(usize, bool)>,
    },
    SetVisible {
        view: usize,
        properties: Vec<usize>,
    },
    SetColumnWidth {
        view: usize,
        property: usize,
        width: u32,
    },
    SetViewKind {
        view: usize,
        property: Option<usize>,
    },
    MoveRow {
        view: usize,
        row: usize,
        before: Option<usize>,
    },
    MoveCard {
        view: usize,
        row: usize,
        group: Option<usize>,
        before: Option<usize>,
    },
}

fn step() -> impl Strategy<Value = Step> {
    let index = any::<usize>();
    let seed = any::<u8>();
    let few = || proptest::collection::vec((any::<usize>(), any::<u8>()), 0..3);
    prop_oneof![
        1 => Just(Step::CreateDatabase),
        1 => index.prop_map(|block| Step::CreateDatabaseWithView { block }),
        2 => (index, index).prop_map(|(block, database)| Step::InsertDatabaseView { block, database }),
        1 => index.prop_map(|database| Step::RenameDatabase { database }),
        1 => index.prop_map(|database| Step::DeleteDatabase { database }),
        3 => (index, seed).prop_map(|(database, kind)| Step::AddProperty { database, kind }),
        1 => (index, index).prop_map(|(database, property)| Step::RenameProperty { database, property }),
        6 => (index, index, seed).prop_map(|(database, property, kind)| Step::ChangePropertyKind { database, property, kind }),
        4 => (index, index).prop_map(|(database, property)| Step::RemoveProperty { database, property }),
        2 => (index, index, index).prop_map(|(database, property, index)| Step::MoveProperty { database, property, index }),
        2 => (index, index).prop_map(|(database, property)| Step::AddSelectOption { database, property }),
        2 => (index, index, index).prop_map(|(database, property, option)| Step::RenameSelectOption { database, property, option }),
        4 => (index, index, index).prop_map(|(database, property, option)| Step::RemoveSelectOption { database, property, option }),
        5 => (index, proptest::collection::vec((any::<usize>(), any::<u8>()), 0..4))
            .prop_map(|(database, values)| Step::AddRow { database, values }),
        8 => (index, index, index, seed, any::<bool>())
            .prop_map(|(database, row, property, seed, clear)| Step::SetValue { database, row, property, seed, clear }),
        2 => (index, proptest::option::of(index)).prop_map(|(block, property)| Step::AddView { block, property }),
        2 => index.prop_map(|view| Step::RemoveView { view }),
        4 => (index, few()).prop_map(|(view, filters)| Step::SetFilters { view, filters }),
        2 => (index, proptest::collection::vec((any::<usize>(), any::<bool>()), 0..3))
            .prop_map(|(view, sorts)| Step::SetSorts { view, sorts }),
        2 => (index, proptest::collection::vec(any::<usize>(), 0..4))
            .prop_map(|(view, properties)| Step::SetVisible { view, properties }),
        1 => (index, index, 0u32..400).prop_map(|(view, property, width)| Step::SetColumnWidth { view, property, width }),
        2 => (index, proptest::option::of(index)).prop_map(|(view, property)| Step::SetViewKind { view, property }),
        3 => (index, index, proptest::option::of(index)).prop_map(|(view, row, before)| Step::MoveRow { view, row, before }),
        3 => (index, index, proptest::option::of(index), proptest::option::of(index))
            .prop_map(|(view, row, group, before)| Step::MoveCard { view, row, group, before }),
    ]
}

fn resolve(workspace: &Workspace, step: &Step) -> Option<Command> {
    let all_databases = databases(workspace);
    let database_at = |index: usize| pick_id(&all_databases, index);
    let blocks = view_blocks(workspace);
    let all_views = views(workspace);
    Some(match step {
        Step::CreateDatabase => Command::CreateDatabase,
        Step::CreateDatabaseWithView { block } => Command::CreateDatabaseWithView {
            after: pick_id(&blocks, *block)?,
        },
        Step::InsertDatabaseView { block, database } => Command::InsertDatabaseView {
            after: pick_id(&blocks, *block)?,
            database: database_at(*database)?,
        },
        Step::RenameDatabase { database } => Command::RenameDatabase {
            database: database_at(*database)?,
            name: "renamed".to_owned(),
        },
        Step::DeleteDatabase { database } => Command::DeleteDatabase {
            database: database_at(*database)?,
        },
        Step::AddProperty { database, kind } => Command::AddProperty {
            database: database_at(*database)?,
            name: "added".to_owned(),
            kind: kind_for(*kind),
        },
        Step::RenameProperty { database, property } => {
            let database = database_at(*database)?;
            Command::RenameProperty {
                database,
                property: pick(&schema(workspace, database), *property)?.id,
                name: "renamed".to_owned(),
            }
        }
        Step::ChangePropertyKind {
            database,
            property,
            kind,
        } => {
            let database = database_at(*database)?;
            Command::ChangePropertyKind {
                database,
                property: pick(&schema(workspace, database), *property)?.id,
                kind: kind_for(*kind),
            }
        }
        Step::RemoveProperty { database, property } => {
            let database = database_at(*database)?;
            Command::RemoveProperty {
                database,
                property: pick(&schema(workspace, database), *property)?.id,
            }
        }
        Step::MoveProperty {
            database,
            property,
            index,
        } => {
            let database = database_at(*database)?;
            Command::MoveProperty {
                database,
                property: pick(&schema(workspace, database), *property)?.id,
                index: *index % 10,
            }
        }
        Step::AddSelectOption { database, property } => {
            let database = database_at(*database)?;
            Command::AddSelectOption {
                database,
                property: pick(&schema(workspace, database), *property)?.id,
                name: "option".to_owned(),
                color: OptionColor::Blue,
            }
        }
        Step::RenameSelectOption {
            database,
            property,
            option,
        } => {
            let database = database_at(*database)?;
            let property = pick(&schema(workspace, database), *property)?.clone();
            Command::RenameSelectOption {
                database,
                property: property.id,
                option: pick(options_of(&property.kind), *option)?.id,
                name: "option renamed".to_owned(),
            }
        }
        Step::RemoveSelectOption {
            database,
            property,
            option,
        } => {
            let database = database_at(*database)?;
            let property = pick(&schema(workspace, database), *property)?.clone();
            Command::RemoveSelectOption {
                database,
                property: property.id,
                option: pick(options_of(&property.kind), *option)?.id,
            }
        }
        Step::AddRow { database, values } => {
            let database = database_at(*database)?;
            let properties = schema(workspace, database);
            let values = values
                .iter()
                .filter_map(|(property, seed)| {
                    let property = pick(&properties, *property)?;
                    Some((property.id, value_for(&property.kind, *seed)))
                })
                .collect();
            Command::AddRow { database, values }
        }
        Step::SetValue {
            database,
            row,
            property,
            seed,
            clear,
        } => {
            let database = database_at(*database)?;
            let property = pick(&schema(workspace, database), *property)?.clone();
            Command::SetValue {
                row: pick_id(&rows(workspace, database), *row)?,
                property: property.id,
                value: (!clear).then(|| value_for(&property.kind, *seed)),
            }
        }
        Step::AddView { block, property } => {
            let block = pick_id(&blocks, *block)?;
            let properties = match workspace.block(block).map(|found| &found.kind) {
                Some(BlockKind::DatabaseView { database }) => schema(workspace, *database),
                _ => Vec::new(),
            };
            Command::AddView {
                block,
                name: "view".to_owned(),
                kind: match property {
                    Some(property) => ViewKind::Board {
                        group_by: pick(&properties, *property)?.id,
                    },
                    None => ViewKind::Table,
                },
            }
        }
        Step::RemoveView { view } => Command::RemoveView {
            view: pick_id(&all_views, *view)?,
        },
        Step::SetFilters { view, filters } => {
            let view = pick_id(&all_views, *view)?;
            let properties = view_schema(workspace, view);
            Command::SetFilters {
                view,
                filters: filters
                    .iter()
                    .filter_map(|(property, seed)| {
                        let property = pick(&properties, *property)?;
                        Some(Filter {
                            property: property.id,
                            condition: condition_for(&property.kind, *seed),
                        })
                    })
                    .collect(),
            }
        }
        Step::SetSorts { view, sorts } => {
            let view = pick_id(&all_views, *view)?;
            let properties = view_schema(workspace, view);
            Command::SetSorts {
                view,
                sorts: sorts
                    .iter()
                    .filter_map(|(property, ascending)| {
                        Some(Sort {
                            property: pick(&properties, *property)?.id,
                            direction: if *ascending {
                                SortDirection::Ascending
                            } else {
                                SortDirection::Descending
                            },
                        })
                    })
                    .collect(),
            }
        }
        Step::SetVisible { view, properties } => {
            let view = pick_id(&all_views, *view)?;
            let available = view_schema(workspace, view);
            Command::SetVisibleProperties {
                view,
                properties: properties
                    .iter()
                    .filter_map(|property| pick(&available, *property).map(|found| found.id))
                    .collect(),
            }
        }
        Step::SetColumnWidth {
            view,
            property,
            width,
        } => {
            let view = pick_id(&all_views, *view)?;
            Command::SetColumnWidth {
                view,
                property: pick(&view_schema(workspace, view), *property)?.id,
                width: *width,
            }
        }
        Step::SetViewKind { view, property } => {
            let view = pick_id(&all_views, *view)?;
            let properties = view_schema(workspace, view);
            Command::SetViewKind {
                view,
                kind: match property {
                    Some(property) => ViewKind::Board {
                        group_by: pick(&properties, *property)?.id,
                    },
                    None => ViewKind::Table,
                },
            }
        }
        Step::MoveRow { view, row, before } => {
            let view = pick_id(&all_views, *view)?;
            let order = workspace.query(view).ok()?;
            Command::MoveRow {
                view,
                row: pick_id(&order, *row)?,
                before: before.and_then(|index| pick_id(&order, index)),
            }
        }
        Step::MoveCard {
            view,
            row,
            group,
            before,
        } => {
            let view = pick_id(&all_views, *view)?;
            let order = workspace.query(view).ok()?;
            let options: Vec<OptionId> = match workspace.view(view)?.kind {
                ViewKind::Board { group_by } => view_schema(workspace, view)
                    .iter()
                    .find(|property| property.id == group_by)
                    .map(|property| options_of(&property.kind).iter().map(|o| o.id).collect())
                    .unwrap_or_default(),
                ViewKind::Table => Vec::new(),
            };
            Command::MoveCard {
                view,
                row: pick_id(&order, *row)?,
                group: group.and_then(|index| pick_id(&options, index)),
                before: before.and_then(|index| pick_id(&order, index)),
            }
        }
    })
}

fn forward_has_no_removals(transaction: &Transaction) -> bool {
    transaction.operations().iter().all(|operation| {
        !matches!(
            operation,
            Operation::RemoveBlock { .. } | Operation::RemoveDatabase { .. }
        )
    })
}

fn created_exist(workspace: &Workspace, built: &Built) -> bool {
    built.created.iter().all(|created| match created {
        Created::Block(id) => workspace.block(*id).is_some(),
        Created::Database(id) => workspace.database(*id).is_some(),
        Created::View(id) => workspace.view(*id).is_some(),
    })
}

fn rejected_commands_leave_no_trace(
    workspace: &Workspace,
    before: &Workspace,
    error: &DomainError,
) -> bool {
    let _ = error;
    workspace == before
}

proptest! {
    #[test]
    fn random_database_commands_round_trip_and_reload(
        steps in proptest::collection::vec(step(), 1..30)
    ) {
        let mut workspace = seed();
        let initial = workspace.clone();
        let mut inverses = Vec::new();
        for step in &steps {
            let Some(command) = resolve(&workspace, step) else { continue };
            let before = workspace.clone();
            let built = match build(&workspace, &command, now()) {
                Ok(built) => built,
                Err(error) => {
                    prop_assert!(rejected_commands_leave_no_trace(&workspace, &before, &error));
                    continue;
                }
            };
            prop_assert!(forward_has_no_removals(&built.transaction), "{:?}", command);
            match workspace.apply(&built.transaction) {
                Ok(inverse) => {
                    prop_assert_eq!(workspace.validate_databases(), Ok(()), "{:?}", command);
                    let reloaded = Workspace::from_snapshot(workspace.to_snapshot());
                    prop_assert_eq!(reloaded.as_ref(), Ok(&workspace), "{:?}", command);
                    prop_assert!(created_exist(&workspace, &built), "{:?}", command);
                    let mut restored = workspace.clone();
                    prop_assert!(restored.apply(&inverse).is_ok(), "{:?}", command);
                    prop_assert_eq!(&restored, &before, "{:?}", command);
                    prop_assert!(restored.apply(&built.transaction).is_ok(), "{:?}", command);
                    prop_assert_eq!(&restored, &workspace, "{:?}", command);
                    inverses.push(inverse);
                }
                Err(error) => {
                    prop_assert!(
                        rejected_commands_leave_no_trace(&workspace, &before, &error),
                        "{:?}",
                        command
                    );
                }
            }
        }
        for inverse in inverses.iter().rev() {
            prop_assert!(workspace.apply(inverse).is_ok());
        }
        prop_assert_eq!(workspace, initial);
    }
}
