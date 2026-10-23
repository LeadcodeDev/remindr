use remindr_core::domain::WorkspaceSnapshot;
use remindr_core::ports::WorkspaceStore;
use remindr_store::SqliteStore;

#[test]
fn open_creates_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    SqliteStore::open(&path).unwrap();
    assert!(path.exists());
}

#[test]
fn reopening_a_migrated_file_succeeds_and_loads_empty() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    SqliteStore::open(&path).unwrap();
    let reopened = SqliteStore::open(&path).unwrap();
    assert_eq!(reopened.load().unwrap(), WorkspaceSnapshot::default());
}
