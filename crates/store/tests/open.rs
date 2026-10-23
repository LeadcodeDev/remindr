#[test]
fn open_creates_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    remindr_store::SqliteStore::open(&path).unwrap();
    assert!(path.exists());
}
