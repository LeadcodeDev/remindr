use remindr_core::testing::store_contract;
use remindr_store::SqliteStore;

#[test]
fn sqlite_store_honours_the_contract() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.db");
    store_contract(|| SqliteStore::open(&path).unwrap());
}
