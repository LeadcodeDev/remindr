use std::collections::HashMap;

use remindr_core::domain::{DatabaseId, Property, WorkspaceSnapshot};
use remindr_core::ports::StoreError;
use rusqlite::{Connection, Row};

use crate::codec::{BlockRow, DatabaseRow, PropertyRow, ValueRow, ViewRow};
use crate::error::read_error;

pub(crate) fn load(connection: &Connection) -> Result<WorkspaceSnapshot, StoreError> {
    Ok(WorkspaceSnapshot {
        blocks: read_all(
            connection,
            BlockRow::SELECT,
            BlockRow::from_row,
            BlockRow::into_block,
        )?,
        databases: load_databases(connection)?,
        values: read_all(
            connection,
            ValueRow::SELECT,
            ValueRow::from_row,
            ValueRow::into_value,
        )?,
        views: read_all(
            connection,
            ViewRow::SELECT,
            ViewRow::from_row,
            ViewRow::into_view,
        )?,
    })
}

fn load_databases(
    connection: &Connection,
) -> Result<Vec<remindr_core::domain::Database>, StoreError> {
    let mut properties: HashMap<DatabaseId, Vec<Property>> = HashMap::new();
    let rows = read_all(
        connection,
        PropertyRow::SELECT,
        PropertyRow::from_row,
        PropertyRow::into_property,
    )?;
    for (database, property) in rows {
        properties.entry(database).or_default().push(property);
    }
    read_all(
        connection,
        DatabaseRow::SELECT,
        DatabaseRow::from_row,
        |row| row.into_database(&mut properties),
    )
}

fn read_all<R, T>(
    connection: &Connection,
    sql: &str,
    from_row: impl Fn(&Row<'_>) -> rusqlite::Result<R>,
    mut decode: impl FnMut(R) -> Result<T, StoreError>,
) -> Result<Vec<T>, StoreError> {
    let mut statement = connection.prepare_cached(sql).map_err(read_error)?;
    let rows = statement.query_map((), from_row).map_err(read_error)?;
    rows.map(|row| decode(row.map_err(read_error)?)).collect()
}
