use remindr_core::domain::{BlockId, Workspace, WorkspaceSnapshot};

#[test]
fn empty_snapshot_builds_an_empty_workspace() {
    let workspace = Workspace::from_snapshot(WorkspaceSnapshot::default()).unwrap();
    assert_eq!(workspace.to_snapshot(), WorkspaceSnapshot::default());
}

#[test]
fn block_ids_are_unique() {
    assert_ne!(BlockId::new(), BlockId::new());
}
