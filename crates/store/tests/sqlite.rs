use std::collections::{BTreeMap, HashMap};

use remindr_core::domain::{
    Block, BlockId, BlockKind, Condition, DatabaseId, Filter, NumberCondition, NumberFormat,
    Operation, Parent, Property, PropertyId, PropertyKind, PropertyValue, RichText, SortKey,
    Timestamp, Transaction, View, ViewId, ViewKind, WorkspaceSnapshot,
};
use remindr_core::ports::{StoreError, WorkspaceStore};
use remindr_store::SqliteStore;

#[cfg(test)]
mod support {
    use remindr_core::domain::{
        Block, BlockId, BlockKind, Database, DatabaseId, Operation, Parent, Property, PropertyId,
        PropertyKind, RichText, Schema, SortKey, Timestamp, Transaction, WorkspaceSnapshot,
    };
    use remindr_core::ports::{StoreError, WorkspaceStore};
    use remindr_store::SqliteStore;

    pub fn page() -> Block {
        Block {
            id: BlockId::new(),
            kind: BlockKind::Page { icon: None },
            parent: Parent::Root,
            sort_key: SortKey::parse("V").unwrap(),
            text: RichText::plain("Home"),
            alive: true,
            created_at: Timestamp::from_millis(1),
        }
    }

    pub fn row_page(database: DatabaseId) -> Block {
        Block {
            parent: Parent::Database(database),
            ..page()
        }
    }

    pub fn awkward_numbers() -> Vec<f64> {
        let listed = [
            1.1186231841461947,
            123456789.12345679,
            f64::MAX,
            f64::MIN_POSITIVE,
            5e-324,
            -0.0,
        ];
        let mut numbers: Vec<f64> = listed
            .into_iter()
            .flat_map(|number| [number, -number])
            .collect();
        let mut state = 0x9E37_79B9_7F4A_7C15_u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        numbers.extend((0..1000).map(|_| (next() >> 11) as f64 / (1_u64 << 53) as f64 * 100.0));
        numbers.extend(
            (0..1000)
                .map(|_| f64::from_bits(next()))
                .filter(|number| number.is_finite()),
        );
        numbers
    }

    pub fn assert_rejected(operation: Operation) {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
        let transaction = Transaction::new(vec![Operation::InsertBlock(page()), operation.clone()]);
        let result = store.commit(&transaction);
        assert!(
            matches!(result, Err(StoreError::Write(_))),
            "{operation:?}: {result:?}"
        );
        assert_eq!(
            store.load().unwrap(),
            WorkspaceSnapshot::default(),
            "{operation:?}"
        );
    }

    pub fn property(name: &str, kind: PropertyKind) -> Property {
        Property {
            id: PropertyId::new(),
            name: name.to_owned(),
            kind,
        }
    }

    pub fn database(properties: Vec<Property>) -> Database {
        Database {
            id: DatabaseId::new(),
            name: "Tasks".to_owned(),
            icon: None,
            schema: Schema { properties },
            sort_key: SortKey::parse("V").unwrap(),
            alive: true,
            created_at: Timestamp::from_millis(1),
        }
    }

    pub fn names(store: &SqliteStore) -> Vec<String> {
        let snapshot = store.load().unwrap();
        snapshot.databases[0]
            .schema
            .properties
            .iter()
            .map(|property| property.name.clone())
            .collect()
    }

    pub fn put(database: &Database, property: &Property, index: usize) -> Transaction {
        Transaction::new(vec![Operation::PutProperty {
            database: database.id,
            property: property.clone(),
            index,
        }])
    }
}

use support::{assert_rejected, awkward_numbers, database, names, property, put, row_page};

#[test]
fn put_property_reorders_positions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
    let title = property("Name", PropertyKind::Title);
    let notes = property("Notes", PropertyKind::Text);
    let cost = property(
        "Cost",
        PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    );
    let database = database(vec![title.clone(), notes.clone()]);
    store
        .commit(&Transaction::new(vec![Operation::InsertDatabase(
            database.clone(),
        )]))
        .unwrap();
    assert_eq!(names(&store), ["Name", "Notes"]);

    store.commit(&put(&database, &cost, 1)).unwrap();
    assert_eq!(names(&store), ["Name", "Cost", "Notes"]);

    store.commit(&put(&database, &cost, 0)).unwrap();
    assert_eq!(names(&store), ["Cost", "Name", "Notes"]);

    store.commit(&put(&database, &cost, 99)).unwrap();
    assert_eq!(names(&store), ["Name", "Notes", "Cost"]);

    let renamed = Property {
        name: "Price".to_owned(),
        ..cost.clone()
    };
    store.commit(&put(&database, &renamed, 1)).unwrap();
    assert_eq!(names(&store), ["Name", "Price", "Notes"]);

    store
        .commit(&Transaction::new(vec![Operation::RemoveProperty {
            database: database.id,
            property: title.id,
        }]))
        .unwrap();
    assert_eq!(names(&store), ["Price", "Notes"]);

    store.commit(&put(&database, &title, 5)).unwrap();
    assert_eq!(names(&store), ["Price", "Notes", "Name"]);
}

#[test]
fn schema_order_survives_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let properties = vec![
        property("Z", PropertyKind::Title),
        property("A", PropertyKind::Text),
        property("M", PropertyKind::Checkbox),
    ];
    let database = database(properties);
    SqliteStore::open(&path)
        .unwrap()
        .commit(&Transaction::new(vec![Operation::InsertDatabase(
            database.clone(),
        )]))
        .unwrap();
    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap().databases, vec![database]);
}

#[test]
fn blocks_load_in_id_order_whatever_the_insertion_order() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
    let mut ids: Vec<BlockId> = (0..5).map(|_| BlockId::new()).collect();
    let blocks: Vec<Block> = ids
        .iter()
        .rev()
        .map(|id| Block {
            id: *id,
            kind: BlockKind::Text,
            parent: Parent::Root,
            sort_key: SortKey::parse("V").unwrap(),
            text: RichText::default(),
            alive: true,
            created_at: Timestamp::from_millis(0),
        })
        .collect();
    let operations = blocks.into_iter().map(Operation::InsertBlock).collect();
    store.commit(&Transaction::new(operations)).unwrap();
    ids.sort();
    let loaded: Vec<BlockId> = store
        .load()
        .unwrap()
        .blocks
        .iter()
        .map(|block| block.id)
        .collect();
    assert_eq!(loaded, ids);
}

#[test]
fn corrupt_rows_fail_the_load_with_a_corrupt_error() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let store = SqliteStore::open(&path).unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute(
            "INSERT INTO block (id, kind, kind_data, parent_table, parent_id, sort_key, text, alive, created_at) VALUES ('nope', 'text', NULL, 'root', NULL, 'V', '[]', 1, 0)",
            (),
        )
        .unwrap();
    match store.load() {
        Err(StoreError::Corrupt(message)) => {
            assert!(message.starts_with("block.id: "), "{message}")
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn block_operations_on_a_missing_block_fail_and_roll_back() {
    let block = BlockId::new();
    for operation in [
        Operation::RemoveBlock { block },
        Operation::SetText {
            block,
            text: RichText::plain("x"),
        },
        Operation::SetKind {
            block,
            kind: BlockKind::Quote,
        },
        Operation::Move {
            block,
            parent: Parent::Root,
            sort_key: SortKey::parse("V").unwrap(),
        },
        Operation::SetBlockAlive {
            block,
            alive: false,
        },
    ] {
        assert_rejected(operation);
    }
}

#[test]
fn database_operations_on_a_missing_database_fail_and_roll_back() {
    let database = DatabaseId::new();
    for operation in [
        Operation::SetDatabaseName {
            database,
            name: "x".to_owned(),
        },
        Operation::SetDatabaseIcon {
            database,
            icon: None,
        },
        Operation::SetDatabaseAlive {
            database,
            alive: false,
        },
        Operation::RemoveDatabase { database },
    ] {
        assert_rejected(operation);
    }
}

#[test]
fn removing_a_missing_property_fails_and_rolls_back() {
    assert_rejected(Operation::RemoveProperty {
        database: DatabaseId::new(),
        property: PropertyId::new(),
    });
}

#[test]
fn removing_a_missing_view_fails_and_rolls_back() {
    assert_rejected(Operation::RemoveView {
        view: ViewId::new(),
    });
}

#[test]
fn removing_a_missing_property_of_an_existing_database_fails() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
    let database = database(vec![property("Name", PropertyKind::Title)]);
    store
        .commit(&Transaction::new(vec![Operation::InsertDatabase(
            database.clone(),
        )]))
        .unwrap();
    let result = store.commit(&Transaction::new(vec![Operation::RemoveProperty {
        database: database.id,
        property: PropertyId::new(),
    }]));
    assert!(matches!(result, Err(StoreError::Write(_))), "{result:?}");
    assert_eq!(names(&store), ["Name"]);
}

#[test]
fn clearing_an_absent_value_is_a_no_op() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
    let transaction = Transaction::new(vec![Operation::SetValue {
        row: BlockId::new(),
        property: PropertyId::new(),
        value: None,
    }]);
    store.commit(&transaction).unwrap();
    assert_eq!(store.load().unwrap(), WorkspaceSnapshot::default());
    let present = Operation::SetValue {
        row: BlockId::new(),
        property: PropertyId::new(),
        value: Some(PropertyValue::Checkbox(true)),
    };
    store.commit(&Transaction::new(vec![present])).unwrap();
    assert_eq!(store.load().unwrap().values.len(), 1);
}

#[test]
fn numbers_survive_a_reload_bit_for_bit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    let cost = property(
        "Cost",
        PropertyKind::Number {
            format: NumberFormat::Plain,
        },
    );
    let database = database(vec![property("Name", PropertyKind::Title), cost.clone()]);
    let numbers = awkward_numbers();
    let rows: Vec<Block> = numbers.iter().map(|_| row_page(database.id)).collect();
    let filters: Vec<Filter> = numbers[..12]
        .iter()
        .map(|number| Filter {
            property: cost.id,
            condition: Condition::Number(NumberCondition::Eq(*number)),
        })
        .collect();
    let view = View {
        id: ViewId::new(),
        block: BlockId::new(),
        name: "Table".to_owned(),
        sort_key: SortKey::parse("V").unwrap(),
        kind: ViewKind::Table,
        filters,
        sorts: vec![],
        visible: vec![],
        column_widths: BTreeMap::new(),
        manual_order: vec![],
    };
    let mut operations = vec![Operation::InsertDatabase(database)];
    operations.extend(rows.iter().cloned().map(Operation::InsertBlock));
    operations.extend(
        rows.iter()
            .zip(&numbers)
            .map(|(row, number)| Operation::SetValue {
                row: row.id,
                property: cost.id,
                value: Some(PropertyValue::Number(*number)),
            }),
    );
    operations.push(Operation::PutView(view.clone()));
    SqliteStore::open(&path)
        .unwrap()
        .commit(&Transaction::new(operations))
        .unwrap();

    let snapshot = SqliteStore::open(&path).unwrap().load().unwrap();
    let loaded: HashMap<BlockId, u64> = snapshot
        .values
        .iter()
        .map(|(row, _, value)| match value {
            PropertyValue::Number(number) => (*row, number.to_bits()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(loaded.len(), numbers.len());
    for (row, number) in rows.iter().zip(&numbers) {
        assert_eq!(loaded[&row.id], number.to_bits(), "{number:e}");
    }
    let bits = |view: &View| -> Vec<u64> {
        view.filters
            .iter()
            .map(|filter| match filter.condition {
                Condition::Number(NumberCondition::Eq(number)) => number.to_bits(),
                ref other => panic!("{other:?}"),
            })
            .collect()
    };
    assert_eq!(bits(&snapshot.views[0]), bits(&view));
}
