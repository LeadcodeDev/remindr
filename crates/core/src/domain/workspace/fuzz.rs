use std::collections::BTreeMap;

use proptest::prelude::*;
use proptest::sample::Index;

use crate::domain::{
    Block, BlockId, BlockKind, CalendarDate, Condition, Database, DatabaseId, DateCondition, Emoji,
    Filter, HeadingLevel, MultiSelectCondition, NumberCondition, NumberFormat, Operation,
    OptionColor, OptionId, Parent, Property, PropertyId, PropertyKind, PropertyValue, RichText,
    Schema, SelectCondition, SelectOption, Sort, SortDirection, SortKey, TextCondition, Timestamp,
    Transaction, View, ViewId, ViewKind, Workspace, WorkspaceSnapshot,
};

const KEYS: [&str; 7] = ["1", "5", "9", "A", "V", "k", "t"];

#[derive(Clone, Debug)]
struct Pick {
    op: u8,
    a: Index,
    b: Index,
    c: Index,
    flag: bool,
    small: u8,
}

fn pick() -> impl Strategy<Value = Pick> {
    (
        0u8..20,
        any::<Index>(),
        any::<Index>(),
        any::<Index>(),
        any::<bool>(),
        0u8..16,
    )
        .prop_map(|(op, a, b, c, flag, small)| Pick {
            op,
            a,
            b,
            c,
            flag,
            small,
        })
}

fn choose<'a, T>(items: &'a [T], index: &Index) -> Option<&'a T> {
    if items.is_empty() {
        None
    } else {
        items.get(index.index(items.len()))
    }
}

fn sort_key(pick: &Pick) -> SortKey {
    SortKey::parse(KEYS[usize::from(pick.small) % KEYS.len()]).unwrap()
}

fn text(pick: &Pick) -> RichText {
    match pick.small % 3 {
        0 => RichText::default(),
        1 => RichText::plain("a"),
        _ => RichText::plain("bc"),
    }
}

fn block_kind(pick: &Pick) -> BlockKind {
    match pick.small % 8 {
        0 => BlockKind::Text,
        1 => BlockKind::Heading(HeadingLevel::One),
        2 => BlockKind::ToDo { checked: pick.flag },
        3 => BlockKind::Toggle,
        4 => BlockKind::Divider,
        5 => BlockKind::Page { icon: None },
        6 => BlockKind::Quote,
        _ => BlockKind::Page {
            icon: Some(Emoji::try_from("📄").unwrap()),
        },
    }
}

fn options(pick: &Pick) -> Vec<SelectOption> {
    (0..pick.small % 3)
        .map(|index| SelectOption {
            id: OptionId::new(),
            name: format!("o{index}"),
            color: OptionColor::Default,
        })
        .collect()
}

fn property_kind(pick: &Pick) -> PropertyKind {
    match pick.small % 7 {
        0 => PropertyKind::Text,
        1 => PropertyKind::Number {
            format: NumberFormat::Plain,
        },
        2 => PropertyKind::Select {
            options: options(pick),
        },
        3 => PropertyKind::MultiSelect {
            options: options(pick),
        },
        4 => PropertyKind::Date,
        5 => PropertyKind::Checkbox,
        _ => PropertyKind::Url,
    }
}

fn value_for(kind: &PropertyKind, pick: &Pick) -> Option<PropertyValue> {
    Some(match kind {
        PropertyKind::Title => return None,
        PropertyKind::Text => PropertyValue::Text("v".to_owned()),
        PropertyKind::Url => PropertyValue::Url("u".to_owned()),
        PropertyKind::Number { .. } => PropertyValue::Number(1.5),
        PropertyKind::Select { options } => PropertyValue::Select(choose(options, &pick.c)?.id),
        PropertyKind::MultiSelect { options } => {
            PropertyValue::MultiSelect(choose(options, &pick.c).map(|o| o.id).into_iter().collect())
        }
        PropertyKind::Date => PropertyValue::Date(CalendarDate::new(2026, 2, 28).unwrap()),
        PropertyKind::Checkbox => PropertyValue::Checkbox(pick.flag),
    })
}

fn condition_for(kind: &PropertyKind, pick: &Pick) -> Option<Condition> {
    Some(match kind {
        PropertyKind::Title | PropertyKind::Text | PropertyKind::Url => {
            Condition::Text(TextCondition::IsEmpty)
        }
        PropertyKind::Number { .. } => Condition::Number(NumberCondition::IsEmpty),
        PropertyKind::Select { options } => {
            let option = choose(options, &pick.c)?.id;
            Condition::Select(SelectCondition::Is(option))
        }
        PropertyKind::MultiSelect { options } => {
            let option = choose(options, &pick.c)?.id;
            Condition::MultiSelect(MultiSelectCondition::Contains(option))
        }
        PropertyKind::Date => Condition::Date(DateCondition::IsEmpty),
        PropertyKind::Checkbox => Condition::Checkbox(pick.flag),
    })
}

fn new_database(pick: &Pick) -> Database {
    let mut properties = vec![Property {
        id: PropertyId::new(),
        name: "Name".to_owned(),
        kind: PropertyKind::Title,
    }];
    for index in 0..pick.small % 4 {
        let shifted = Pick {
            small: pick.small.wrapping_add(index),
            ..pick.clone()
        };
        properties.push(Property {
            id: PropertyId::new(),
            name: format!("p{index}"),
            kind: property_kind(&shifted),
        });
    }
    Database {
        id: DatabaseId::new(),
        name: "Tasks".to_owned(),
        icon: None,
        schema: Schema { properties },
        sort_key: sort_key(pick),
        alive: pick.flag || pick.small.is_multiple_of(2),
        created_at: Timestamp::from_millis(0),
    }
}

fn block_with(kind: BlockKind, parent: Parent, pick: &Pick, text: RichText) -> Block {
    Block {
        id: BlockId::new(),
        kind,
        parent,
        sort_key: sort_key(pick),
        text,
        alive: pick.flag || !pick.small.is_multiple_of(5),
        created_at: Timestamp::from_millis(0),
    }
}

fn operation_for(snapshot: &WorkspaceSnapshot, pick: &Pick) -> Option<Operation> {
    let blocks = &snapshot.blocks;
    let databases = &snapshot.databases;
    match pick.op {
        0 => Some(Operation::InsertBlock(block_with(
            BlockKind::Page { icon: None },
            Parent::Root,
            pick,
            text(pick),
        ))),
        1 | 2 => {
            let parent = choose(blocks, &pick.a)?;
            Some(Operation::InsertBlock(block_with(
                block_kind(pick),
                Parent::Block(parent.id),
                pick,
                text(pick),
            )))
        }
        3 => Some(Operation::InsertDatabase(new_database(pick))),
        4 | 5 => {
            let database = choose(databases, &pick.a)?;
            Some(Operation::InsertBlock(block_with(
                BlockKind::Page { icon: None },
                Parent::Database(database.id),
                pick,
                text(pick),
            )))
        }
        6 => {
            let database = choose(databases, &pick.a)?;
            let parent = choose(blocks, &pick.b)?;
            Some(Operation::InsertBlock(block_with(
                BlockKind::DatabaseView {
                    database: database.id,
                },
                Parent::Block(parent.id),
                pick,
                RichText::default(),
            )))
        }
        7 | 8 => {
            let database = choose(databases, &pick.a)?;
            let existing = choose(&database.schema.properties, &pick.b)?;
            let property = if pick.flag {
                let kind = if existing.kind == PropertyKind::Title {
                    PropertyKind::Title
                } else {
                    property_kind(pick)
                };
                Property {
                    id: existing.id,
                    name: "renamed".to_owned(),
                    kind,
                }
            } else {
                Property {
                    id: PropertyId::new(),
                    name: "new".to_owned(),
                    kind: property_kind(pick),
                }
            };
            Some(Operation::PutProperty {
                database: database.id,
                property,
                index: usize::from(pick.small % 5),
            })
        }
        9 => {
            let database = choose(databases, &pick.a)?;
            let property = choose(&database.schema.properties, &pick.b)?;
            Some(Operation::RemoveProperty {
                database: database.id,
                property: property.id,
            })
        }
        10..=12 => {
            let database = choose(databases, &pick.a)?;
            let property = choose(&database.schema.properties, &pick.b)?;
            let rows: Vec<&Block> = blocks
                .iter()
                .filter(|block| block.parent == Parent::Database(database.id))
                .collect();
            let row = choose(&rows, &pick.c)?;
            let value = if pick.small.is_multiple_of(5) {
                None
            } else {
                value_for(&property.kind, pick)
            };
            Some(Operation::SetValue {
                row: row.id,
                property: property.id,
                value,
            })
        }
        13 | 14 => {
            let embeds: Vec<&Block> = blocks
                .iter()
                .filter(|block| matches!(block.kind, BlockKind::DatabaseView { .. }))
                .collect();
            let embed = choose(&embeds, &pick.a)?;
            let BlockKind::DatabaseView { database } = embed.kind else {
                return None;
            };
            let record = databases
                .iter()
                .find(|candidate| candidate.id == database)?;
            let properties = &record.schema.properties;
            let property = choose(properties, &pick.b)?;
            let filters = condition_for(&property.kind, pick)
                .map(|condition| {
                    vec![Filter {
                        property: property.id,
                        condition,
                    }]
                })
                .unwrap_or_default();
            let sorts = choose(properties, &pick.c)
                .map(|sorted| {
                    vec![Sort {
                        property: sorted.id,
                        direction: SortDirection::Ascending,
                    }]
                })
                .unwrap_or_default();
            let rows: Vec<BlockId> = blocks
                .iter()
                .filter(|block| block.parent == Parent::Database(database))
                .map(|block| block.id)
                .collect();
            let selects: Vec<&Property> = properties
                .iter()
                .filter(|candidate| matches!(candidate.kind, PropertyKind::Select { .. }))
                .collect();
            let kind = match choose(&selects, &pick.c) {
                Some(select) if pick.flag => ViewKind::Board {
                    group_by: select.id,
                },
                _ => ViewKind::Table,
            };
            let existing: Vec<&View> = snapshot
                .views
                .iter()
                .filter(|view| view.block == embed.id)
                .collect();
            let id = match choose(&existing, &pick.b) {
                Some(view) if pick.small.is_multiple_of(2) => view.id,
                _ => ViewId::new(),
            };
            Some(Operation::PutView(View {
                id,
                block: embed.id,
                name: "View".to_owned(),
                sort_key: sort_key(pick),
                kind,
                filters,
                sorts,
                visible: vec![property.id],
                column_widths: BTreeMap::from([(property.id, 100)]),
                manual_order: rows,
            }))
        }
        15 => {
            let view = choose(&snapshot.views, &pick.a)?;
            Some(Operation::RemoveView { view: view.id })
        }
        16 => Some(Operation::RemoveBlock {
            block: choose(blocks, &pick.a)?.id,
        }),
        17 => Some(Operation::RemoveDatabase {
            database: choose(databases, &pick.a)?.id,
        }),
        18 => {
            let block = choose(blocks, &pick.a)?;
            let parent = match pick.small % 3 {
                0 => Parent::Root,
                1 => Parent::Block(choose(blocks, &pick.b)?.id),
                _ => Parent::Database(choose(databases, &pick.b)?.id),
            };
            Some(Operation::Move {
                block: block.id,
                parent,
                sort_key: sort_key(pick),
            })
        }
        _ => match pick.small % 6 {
            0 => Some(Operation::SetText {
                block: choose(blocks, &pick.a)?.id,
                text: text(pick),
            }),
            1 => Some(Operation::SetBlockAlive {
                block: choose(blocks, &pick.a)?.id,
                alive: pick.flag,
            }),
            2 => {
                let kind = match choose(databases, &pick.b) {
                    Some(database) if pick.flag => BlockKind::DatabaseView {
                        database: database.id,
                    },
                    _ => block_kind(pick),
                };
                Some(Operation::SetKind {
                    block: choose(blocks, &pick.a)?.id,
                    kind,
                })
            }
            3 => Some(Operation::SetDatabaseName {
                database: choose(databases, &pick.a)?.id,
                name: "Renamed".to_owned(),
            }),
            4 => Some(Operation::SetDatabaseIcon {
                database: choose(databases, &pick.a)?.id,
                icon: pick.flag.then(|| Emoji::try_from("🔥").unwrap()),
            }),
            _ => Some(Operation::SetDatabaseAlive {
                database: choose(databases, &pick.a)?.id,
                alive: pick.flag,
            }),
        },
    }
}

fn transaction_picks() -> impl Strategy<Value = Vec<Pick>> {
    proptest::collection::vec(pick(), 1..=3)
}

proptest! {
    #[test]
    fn any_operation_sequence_keeps_the_workspace_reversible_and_reloadable(
        steps in proptest::collection::vec(transaction_picks(), 0..80)
    ) {
        let mut workspace = Workspace::default();
        for picks in &steps {
            let snapshot = workspace.to_snapshot();
            let operations: Vec<Operation> = picks
                .iter()
                .filter_map(|pick| operation_for(&snapshot, pick))
                .collect();
            if operations.is_empty() {
                continue;
            }
            let transaction = Transaction::new(operations);
            let before = workspace.clone();
            match workspace.apply(&transaction) {
                Err(_) => prop_assert_eq!(&workspace, &before, "{:?}", transaction),
                Ok(inverse) => {
                    let rebuilt = Workspace::from_snapshot(workspace.to_snapshot())
                        .map_err(|error| {
                            TestCaseError::fail(format!("{error:?} after {transaction:?}"))
                        })?;
                    prop_assert_eq!(&rebuilt, &workspace);
                    let mut undone = workspace.clone();
                    let undo = undone.apply(&inverse);
                    prop_assert!(undo.is_ok(), "{:?} inverse of {:?}", undo.err(), transaction);
                    prop_assert_eq!(&undone, &before, "{:?}", transaction);
                    let mut redone = undone.clone();
                    prop_assert!(redone.apply(&transaction).is_ok());
                    prop_assert_eq!(&redone, &workspace);
                }
            }
        }
    }
}
