use remindr_core::domain::{
    Block, BlockId, BlockKind, NumberFormat, Operation, Parent, Property, PropertyKind, RichText,
    SortKey, Timestamp, Transaction,
};
use remindr_core::ports::{StoreError, WorkspaceStore};
use remindr_store::SqliteStore;

#[cfg(test)]
mod support {
    use remindr_core::domain::{
        Database, DatabaseId, Operation, Property, PropertyId, PropertyKind, Schema, SortKey,
        Timestamp, Transaction,
    };
    use remindr_core::ports::WorkspaceStore;
    use remindr_store::SqliteStore;

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

use support::{database, names, property, put};

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
