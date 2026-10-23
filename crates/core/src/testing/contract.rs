use crate::ports::WorkspaceStore;

pub fn store_contract<S: WorkspaceStore>(open: impl Fn() -> S) {
    todo!()
}
