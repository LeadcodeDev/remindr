use remindr_core::ports::StoreError;
use rusqlite::Connection;

use crate::error::write_error;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

pub(crate) fn migrate(connection: &mut Connection) -> Result<(), StoreError> {
    let applied: u32 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(write_error)?;
    let latest = latest_version();
    if applied > latest {
        return Err(StoreError::Corrupt(format!(
            "user_version {applied} is newer than the supported {latest}"
        )));
    }
    if applied == latest {
        return Ok(());
    }
    let transaction = connection.transaction().map_err(write_error)?;
    for migration in MIGRATIONS
        .iter()
        .skip(usize::try_from(applied).unwrap_or(usize::MAX))
    {
        transaction.execute_batch(migration).map_err(write_error)?;
    }
    transaction
        .pragma_update(None, "user_version", latest)
        .map_err(write_error)?;
    transaction.commit().map_err(write_error)
}

fn latest_version() -> u32 {
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{MIGRATIONS, migrate};

    fn user_version(connection: &Connection) -> u32 {
        connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn open_sets_user_version_to_one() {
        let mut connection = Connection::open_in_memory().unwrap();
        migrate(&mut connection).unwrap();
        assert_eq!(
            usize::try_from(user_version(&connection)),
            Ok(MIGRATIONS.len())
        );
        assert_eq!(user_version(&connection), 1);
    }

    #[test]
    fn reopen_does_not_rerun_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.db");
        let mut first = Connection::open(&path).unwrap();
        migrate(&mut first).unwrap();
        first
            .execute(
                "INSERT INTO database (id, name, icon, sort_key, alive, created_at) VALUES ('d', 'n', NULL, 'V', 1, 0)",
                (),
            )
            .unwrap();
        drop(first);
        let mut second = Connection::open(&path).unwrap();
        migrate(&mut second).unwrap();
        let rows: i64 = second
            .query_row("SELECT count(*) FROM database", (), |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
        assert_eq!(user_version(&second), 1);
    }

    #[test]
    fn newer_schema_is_rejected() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection.pragma_update(None, "user_version", 7).unwrap();
        assert!(migrate(&mut connection).is_err());
    }
}
