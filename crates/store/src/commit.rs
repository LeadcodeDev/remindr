use remindr_core::domain::{
    Block, BlockId, BlockKind, Database, DatabaseId, Operation, Parent, Property, PropertyId,
    PropertyValue, SortKey, Transaction, View,
};
use remindr_core::ports::StoreError;
use rusqlite::{Connection, Params};

use crate::codec::{
    StoredId, encode_kind, encode_parent, encode_property_kind, encode_text, encode_value,
    encode_view,
};
use crate::error::write_error;

pub(crate) fn commit(
    connection: &mut Connection,
    transaction: &Transaction,
) -> Result<(), StoreError> {
    let scope = connection.transaction().map_err(write_error)?;
    for operation in transaction.operations() {
        apply(&scope, operation)?;
    }
    scope.commit().map_err(write_error)
}

fn apply(connection: &Connection, operation: &Operation) -> Result<(), StoreError> {
    match operation {
        Operation::InsertBlock(block) => insert_block(connection, block),
        Operation::RemoveBlock { block } => run(
            connection,
            "DELETE FROM block WHERE id = ?1",
            (block.to_text(),),
        ),
        Operation::SetText { block, text } => run(
            connection,
            "UPDATE block SET text = ?1 WHERE id = ?2",
            (encode_text(text)?, block.to_text()),
        ),
        Operation::SetKind { block, kind } => set_kind(connection, block, kind),
        Operation::Move {
            block,
            parent,
            sort_key,
        } => move_block(connection, block, parent, sort_key),
        Operation::SetBlockAlive { block, alive } => run(
            connection,
            "UPDATE block SET alive = ?1 WHERE id = ?2",
            (alive, block.to_text()),
        ),
        Operation::InsertDatabase(database) => insert_database(connection, database),
        Operation::RemoveDatabase { database } => remove_database(connection, database),
        Operation::SetDatabaseName { database, name } => run(
            connection,
            "UPDATE database SET name = ?1 WHERE id = ?2",
            (name, database.to_text()),
        ),
        Operation::SetDatabaseIcon { database, icon } => run(
            connection,
            "UPDATE database SET icon = ?1 WHERE id = ?2",
            (icon.as_ref().map(|icon| icon.as_str()), database.to_text()),
        ),
        Operation::SetDatabaseAlive { database, alive } => run(
            connection,
            "UPDATE database SET alive = ?1 WHERE id = ?2",
            (alive, database.to_text()),
        ),
        Operation::PutProperty {
            database,
            property,
            index,
        } => put_property(connection, database, property, *index),
        Operation::RemoveProperty { database, property } => {
            remove_property(connection, database, property)
        }
        Operation::SetValue {
            row,
            property,
            value,
        } => set_value(connection, row, property, value.as_ref()),
        Operation::PutView(view) => put_view(connection, view),
        Operation::RemoveView { view } => run(
            connection,
            "DELETE FROM view WHERE id = ?1",
            (view.to_text(),),
        ),
    }
}

fn run(connection: &Connection, sql: &str, params: impl Params) -> Result<(), StoreError> {
    connection
        .prepare_cached(sql)
        .map_err(write_error)?
        .execute(params)
        .map(drop)
        .map_err(write_error)
}

fn insert_block(connection: &Connection, block: &Block) -> Result<(), StoreError> {
    let (kind, kind_data) = encode_kind(&block.kind)?;
    let (parent_table, parent_id) = encode_parent(&block.parent);
    run(
        connection,
        "INSERT INTO block (id, kind, kind_data, parent_table, parent_id, sort_key, text, alive, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        (
            block.id.to_text(),
            kind,
            kind_data,
            parent_table,
            parent_id,
            block.sort_key.as_str(),
            encode_text(&block.text)?,
            block.alive,
            block.created_at.as_millis(),
        ),
    )
}

fn set_kind(connection: &Connection, block: &BlockId, kind: &BlockKind) -> Result<(), StoreError> {
    let (kind, kind_data) = encode_kind(kind)?;
    run(
        connection,
        "UPDATE block SET kind = ?1, kind_data = ?2 WHERE id = ?3",
        (kind, kind_data, block.to_text()),
    )
}

fn move_block(
    connection: &Connection,
    block: &BlockId,
    parent: &Parent,
    sort_key: &SortKey,
) -> Result<(), StoreError> {
    let (parent_table, parent_id) = encode_parent(parent);
    run(
        connection,
        "UPDATE block SET parent_table = ?1, parent_id = ?2, sort_key = ?3 WHERE id = ?4",
        (parent_table, parent_id, sort_key.as_str(), block.to_text()),
    )
}

fn insert_database(connection: &Connection, database: &Database) -> Result<(), StoreError> {
    let id = database.id.to_text();
    run(
        connection,
        "INSERT INTO database (id, name, icon, sort_key, alive, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        (
            &id,
            &database.name,
            database.icon.as_ref().map(|icon| icon.as_str()),
            database.sort_key.as_str(),
            database.alive,
            database.created_at.as_millis(),
        ),
    )?;
    database
        .schema
        .properties
        .iter()
        .enumerate()
        .try_for_each(|(position, property)| upsert_property(connection, &id, position, property))
}

fn remove_database(connection: &Connection, database: &DatabaseId) -> Result<(), StoreError> {
    let id = database.to_text();
    run(
        connection,
        "DELETE FROM property WHERE database_id = ?1",
        (&id,),
    )?;
    run(connection, "DELETE FROM database WHERE id = ?1", (&id,))
}

fn put_property(
    connection: &Connection,
    database: &DatabaseId,
    property: &Property,
    index: usize,
) -> Result<(), StoreError> {
    let database = database.to_text();
    let id = property.id.to_text();
    let mut ids = property_ids(connection, &database)?;
    ids.retain(|existing| *existing != id);
    let position = index.min(ids.len());
    upsert_property(connection, &database, position, property)?;
    ids.insert(position, id);
    write_positions(connection, &ids)
}

fn remove_property(
    connection: &Connection,
    database: &DatabaseId,
    property: &PropertyId,
) -> Result<(), StoreError> {
    run(
        connection,
        "DELETE FROM property WHERE id = ?1",
        (property.to_text(),),
    )?;
    let ids = property_ids(connection, &database.to_text())?;
    write_positions(connection, &ids)
}

fn upsert_property(
    connection: &Connection,
    database: &str,
    position: usize,
    property: &Property,
) -> Result<(), StoreError> {
    let (kind, kind_data) = encode_property_kind(&property.kind)?;
    run(
        connection,
        "INSERT INTO property (id, database_id, position, name, kind, kind_data) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (id) DO UPDATE SET database_id = excluded.database_id, position = excluded.position, name = excluded.name, kind = excluded.kind, kind_data = excluded.kind_data",
        (
            property.id.to_text(),
            database,
            column_position(position)?,
            &property.name,
            kind,
            kind_data,
        ),
    )
}

fn property_ids(connection: &Connection, database: &str) -> Result<Vec<String>, StoreError> {
    let mut statement = connection
        .prepare_cached("SELECT id FROM property WHERE database_id = ?1 ORDER BY position")
        .map_err(write_error)?;
    statement
        .query_map((database,), |row| row.get(0))
        .map_err(write_error)?
        .collect::<Result<_, _>>()
        .map_err(write_error)
}

fn write_positions(connection: &Connection, ids: &[String]) -> Result<(), StoreError> {
    ids.iter().enumerate().try_for_each(|(position, id)| {
        run(
            connection,
            "UPDATE property SET position = ?1 WHERE id = ?2",
            (column_position(position)?, id),
        )
    })
}

fn column_position(position: usize) -> Result<i64, StoreError> {
    i64::try_from(position).map_err(|error| StoreError::Write(Box::new(error)))
}

fn set_value(
    connection: &Connection,
    row: &BlockId,
    property: &PropertyId,
    value: Option<&PropertyValue>,
) -> Result<(), StoreError> {
    match value {
        Some(value) => run(
            connection,
            "INSERT INTO property_value (row_id, property_id, value) VALUES (?1, ?2, ?3) ON CONFLICT (row_id, property_id) DO UPDATE SET value = excluded.value",
            (row.to_text(), property.to_text(), encode_value(value)?),
        ),
        None => run(
            connection,
            "DELETE FROM property_value WHERE row_id = ?1 AND property_id = ?2",
            (row.to_text(), property.to_text()),
        ),
    }
}

fn put_view(connection: &Connection, view: &View) -> Result<(), StoreError> {
    let (kind, config) = encode_view(view)?;
    run(
        connection,
        "INSERT INTO view (id, block_id, name, kind, sort_key, config) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (id) DO UPDATE SET block_id = excluded.block_id, name = excluded.name, kind = excluded.kind, sort_key = excluded.sort_key, config = excluded.config",
        (
            view.id.to_text(),
            view.block.to_text(),
            &view.name,
            kind,
            view.sort_key.as_str(),
            config,
        ),
    )
}

#[cfg(test)]
mod tests {
    use remindr_core::domain::{
        Block, BlockId, BlockKind, Operation, Parent, RichText, SortKey, Timestamp, Transaction,
        WorkspaceSnapshot,
    };
    use remindr_core::ports::{StoreError, WorkspaceStore};

    use crate::SqliteStore;

    fn page() -> Block {
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

    fn insert(block: &Block) -> Transaction {
        Transaction::new(vec![Operation::InsertBlock(block.clone())])
    }

    #[test]
    fn failed_commit_leaves_file_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.db");
        let mut store = SqliteStore::open(&path).unwrap();
        store.commit(&insert(&page())).unwrap();
        let after_first = store.load().unwrap();
        assert_eq!(after_first.blocks.len(), 1);
        store
            .connection
            .pragma_update(None, "query_only", true)
            .unwrap();
        let result = store.commit(&insert(&page()));
        assert!(matches!(result, Err(StoreError::Write(_))), "{result:?}");
        drop(store);
        let reopened = SqliteStore::open(&path).unwrap();
        assert_eq!(reopened.load().unwrap(), after_first);
    }

    #[test]
    fn failed_operation_rolls_back_the_whole_transaction() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::open(&dir.path().join("workspace.db")).unwrap();
        let first = page();
        let second = page();
        let result = store.commit(&Transaction::new(vec![
            Operation::InsertBlock(first.clone()),
            Operation::InsertBlock(second),
            Operation::InsertBlock(first),
        ]));
        assert!(matches!(result, Err(StoreError::Write(_))), "{result:?}");
        assert_eq!(store.load().unwrap(), WorkspaceSnapshot::default());
    }
}
