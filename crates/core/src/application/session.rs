use crate::domain::{DomainError, Operation, Transaction, Workspace};
use crate::ports::{Clock, StoreError, WorkspaceStore};

use super::history::{Edit, History};
use super::{Changes, Command, Created, TextPoint};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Domain(#[from] DomainError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct Session<S: WorkspaceStore, C: Clock> {
    workspace: Workspace,
    store: S,
    clock: C,
    history: History,
}

impl<S: WorkspaceStore, C: Clock> Session<S, C> {
    pub fn open(store: S, clock: C) -> Result<Self, SessionError> {
        let workspace = Workspace::from_snapshot(store.load()?)?;
        Ok(Self {
            workspace,
            store,
            clock,
            history: History::default(),
        })
    }

    #[must_use]
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub fn execute(&mut self, command: Command) -> Result<Changes, SessionError> {
        let now = self.clock.now();
        let built = super::command::build(&self.workspace, &command, now)?;
        if built.transaction.is_empty() {
            return Ok(Changes {
                created: built.created,
                caret: built.caret,
                ..Changes::default()
            });
        }
        let inverse = apply_and_commit(&mut self.workspace, &mut self.store, &built.transaction)?;
        let edit = match &command {
            Command::EditText { block, .. } => Some(Edit {
                block: *block,
                at: now,
            }),
            _ => None,
        };
        let changes = changes_of(&built.transaction, built.created, built.caret);
        self.history.record(built.transaction, inverse, edit);
        Ok(changes)
    }

    pub fn undo(&mut self) -> Result<Option<Changes>, SessionError> {
        let Some(entry) = self.history.peek_undo() else {
            return Ok(None);
        };
        apply_and_commit(&mut self.workspace, &mut self.store, &entry.inverse)?;
        let changes = changes_of(&entry.inverse, Vec::new(), None);
        self.history.finish_undo();
        Ok(Some(changes))
    }

    pub fn redo(&mut self) -> Result<Option<Changes>, SessionError> {
        let Some(entry) = self.history.peek_redo() else {
            return Ok(None);
        };
        apply_and_commit(&mut self.workspace, &mut self.store, &entry.forward)?;
        let changes = changes_of(&entry.forward, Vec::new(), None);
        self.history.finish_redo();
        Ok(Some(changes))
    }

    #[must_use]
    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    #[must_use]
    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
}

fn apply_and_commit<S: WorkspaceStore>(
    workspace: &mut Workspace,
    store: &mut S,
    transaction: &Transaction,
) -> Result<Transaction, SessionError> {
    let inverse = workspace.apply(transaction)?;
    if let Err(error) = store.commit(transaction) {
        if workspace.apply(&inverse).is_err() {
            *workspace = Workspace::from_snapshot(store.load()?)?;
        }
        return Err(error.into());
    }
    Ok(inverse)
}

fn changes_of(
    transaction: &Transaction,
    created: Vec<Created>,
    caret: Option<TextPoint>,
) -> Changes {
    let mut changes = Changes {
        created,
        caret,
        ..Changes::default()
    };
    for operation in transaction.operations() {
        match operation {
            Operation::InsertBlock(block) => {
                changes.blocks.insert(block.id);
            }
            Operation::RemoveBlock { block }
            | Operation::SetText { block, .. }
            | Operation::SetKind { block, .. }
            | Operation::Move { block, .. }
            | Operation::SetBlockAlive { block, .. } => {
                changes.blocks.insert(*block);
            }
            Operation::SetValue { row, .. } => {
                changes.blocks.insert(*row);
            }
            Operation::InsertDatabase(database) => {
                changes.databases.insert(database.id);
            }
            Operation::RemoveDatabase { database }
            | Operation::SetDatabaseName { database, .. }
            | Operation::SetDatabaseIcon { database, .. }
            | Operation::SetDatabaseAlive { database, .. }
            | Operation::PutProperty { database, .. }
            | Operation::RemoveProperty { database, .. } => {
                changes.databases.insert(*database);
            }
            Operation::PutView(view) => {
                changes.views.insert(view.id);
            }
            Operation::RemoveView { view } => {
                changes.views.insert(*view);
            }
        }
    }
    changes
}

#[cfg(test)]
mod tests {
    use crate::application::{Changes, Command, Created, DropTarget, TextPoint};
    use crate::domain::{
        BlockId, BlockKind, DatabaseId, Operation, Parent, RichText, Timestamp, Workspace,
    };
    use crate::ports::{Clock, StoreError};
    use crate::testing::{ManualClock, MemoryStore};

    use super::{Session, SessionError, changes_of};

    type TestSession = Session<MemoryStore, ManualClock>;

    struct Harness {
        session: TestSession,
        store: MemoryStore,
        clock: ManualClock,
    }

    fn harness() -> Harness {
        let store = MemoryStore::new();
        let clock = ManualClock::new(Timestamp::from_millis(1_000));
        let session = Session::open(store.shared(), clock.clone()).unwrap();
        Harness {
            session,
            store,
            clock,
        }
    }

    fn created_block(changes: &Changes) -> BlockId {
        match changes.created.first() {
            Some(Created::Block(id)) => *id,
            other => panic!("no created block: {other:?}"),
        }
    }

    fn page_with_block(harness: &mut Harness) -> BlockId {
        let page = created_block(
            &harness
                .session
                .execute(Command::CreatePage {
                    parent: Parent::Root,
                    after: None,
                })
                .unwrap(),
        );
        created_block(
            &harness
                .session
                .execute(Command::InsertBlockAt {
                    target: DropTarget {
                        parent: Parent::Block(page),
                        after: None,
                    },
                    kind: BlockKind::Text,
                })
                .unwrap(),
        )
    }

    fn edit(harness: &mut Harness, block: BlockId, text: &str) {
        harness
            .session
            .execute(Command::EditText {
                block,
                text: RichText::plain(text),
            })
            .unwrap();
    }

    fn text_of(harness: &Harness, block: BlockId) -> String {
        harness
            .session
            .workspace()
            .block(block)
            .unwrap()
            .text
            .to_plain()
    }

    #[test]
    fn typing_within_a_second_is_one_undo_step() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        for text in ["a", "ab", "abc"] {
            edit(&mut harness, block, text);
            harness.clock.advance(300);
        }
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
        harness.session.redo().unwrap();
        assert_eq!(text_of(&harness, block), "abc");
    }

    #[test]
    fn typing_after_a_pause_is_two_steps() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.clock.advance(1_000);
        edit(&mut harness, block, "ab");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "a");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn a_gap_just_under_a_second_still_coalesces() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.clock.advance(999);
        edit(&mut harness, block, "ab");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn coalescing_measures_the_gap_from_the_last_edit() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        for text in ["a", "ab", "abc", "abcd"] {
            edit(&mut harness, block, text);
            harness.clock.advance(600);
        }
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn editing_another_block_or_running_another_command_breaks_the_step() {
        let mut harness = harness();
        let first = page_with_block(&mut harness);
        let second = created_block(
            &harness
                .session
                .execute(Command::InsertBlock {
                    after: first,
                    kind: BlockKind::Text,
                })
                .unwrap(),
        );
        edit(&mut harness, first, "a");
        edit(&mut harness, second, "b");
        edit(&mut harness, first, "ab");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, first), "a");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, second), "");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, first), "");
    }

    #[test]
    fn typing_after_an_undo_starts_a_new_step() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.clock.advance(300);
        edit(&mut harness, block, "ab");
        harness.clock.advance(100);
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
        harness.clock.advance(100);
        edit(&mut harness, block, "x");
        harness.clock.advance(100);
        edit(&mut harness, block, "xy");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
        assert!(harness.session.can_redo());
    }

    #[test]
    fn typing_after_undoing_the_second_of_two_steps_does_not_merge_into_the_first() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.clock.advance(1_000);
        edit(&mut harness, block, "ab");
        harness.clock.advance(10);
        harness.session.undo().unwrap();
        harness.clock.advance(10);
        edit(&mut harness, block, "ax");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "a");
    }

    #[test]
    fn a_new_command_clears_the_redo_stack() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.session.undo().unwrap();
        assert!(harness.session.can_redo());
        edit(&mut harness, block, "b");
        assert!(!harness.session.can_redo());
        assert_eq!(harness.session.redo().unwrap(), None);
    }

    #[test]
    fn undo_and_redo_on_empty_history_do_nothing() {
        let mut harness = harness();
        assert!(!harness.session.can_undo());
        assert!(!harness.session.can_redo());
        assert_eq!(harness.session.undo().unwrap(), None);
        assert_eq!(harness.session.redo().unwrap(), None);
    }

    #[test]
    fn failed_commit_leaves_workspace_unchanged() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "kept");
        let before = harness.session.workspace().clone();
        let commits = harness.store.committed();
        harness.store.fail_next_commit();
        let result = harness.session.execute(Command::EditText {
            block,
            text: RichText::plain("lost"),
        });
        assert!(matches!(
            result,
            Err(SessionError::Store(StoreError::Write(_)))
        ));
        assert_eq!(*harness.session.workspace(), before);
        assert_eq!(harness.store.committed(), commits);
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn failed_commit_does_not_record_history_or_clear_redo() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.session.undo().unwrap();
        harness.store.fail_next_commit();
        let result = harness.session.execute(Command::EditText {
            block,
            text: RichText::plain("b"),
        });
        assert!(result.is_err());
        assert!(harness.session.can_redo());
    }

    #[test]
    fn rejected_command_leaves_workspace_and_history_unchanged() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        let before = harness.session.workspace().clone();
        let result = harness.session.execute(Command::Indent { block });
        assert!(matches!(result, Err(SessionError::Domain(_))));
        assert_eq!(*harness.session.workspace(), before);
        harness.session.undo().unwrap();
        assert!(harness.session.can_undo());
    }

    #[test]
    fn failed_undo_commit_keeps_the_entry_and_the_workspace() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.store.fail_next_commit();
        assert!(harness.session.undo().is_err());
        assert_eq!(text_of(&harness, block), "a");
        assert!(!harness.session.can_redo());
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn failed_undo_leaves_the_history_and_the_coalescing_state_unchanged() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.store.fail_next_commit();
        assert!(harness.session.undo().is_err());
        harness.clock.advance(200);
        edit(&mut harness, block, "ab");
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
        assert!(harness.session.can_redo());
    }

    #[test]
    fn failed_redo_leaves_the_history_unchanged() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.session.undo().unwrap();
        harness.store.fail_next_commit();
        assert!(harness.session.redo().is_err());
        harness.session.redo().unwrap();
        assert_eq!(text_of(&harness, block), "a");
        assert!(!harness.session.can_redo());
        harness.session.undo().unwrap();
        assert_eq!(text_of(&harness, block), "");
    }

    #[test]
    fn failed_redo_commit_keeps_the_entry_and_the_workspace() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.session.undo().unwrap();
        harness.store.fail_next_commit();
        assert!(harness.session.redo().is_err());
        assert_eq!(text_of(&harness, block), "");
        assert!(harness.session.can_redo());
        harness.session.redo().unwrap();
        assert_eq!(text_of(&harness, block), "a");
    }

    #[test]
    fn undo_after_reopen_is_empty() {
        let mut harness = harness();
        page_with_block(&mut harness);
        let reopened = Session::open(harness.store.shared(), harness.clock.clone()).unwrap();
        assert!(!reopened.can_undo());
        assert!(!reopened.can_redo());
    }

    #[test]
    fn reopen_restores_workspace() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "persisted");
        let reopened: TestSession =
            Session::open(harness.store.shared(), harness.clock.clone()).unwrap();
        assert_eq!(reopened.workspace(), harness.session.workspace());
    }

    #[test]
    fn undo_and_redo_are_committed_so_a_reopen_sees_them() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        harness.clock.advance(100);
        edit(&mut harness, block, "ab");
        let commits = harness.store.committed();
        harness.session.undo().unwrap();
        assert_eq!(harness.store.committed(), commits + 1);
        let reopened: TestSession =
            Session::open(harness.store.shared(), harness.clock.clone()).unwrap();
        assert_eq!(reopened.workspace(), harness.session.workspace());
        harness.session.redo().unwrap();
        assert_eq!(harness.store.committed(), commits + 2);
        let reopened: TestSession =
            Session::open(harness.store.shared(), harness.clock.clone()).unwrap();
        assert_eq!(reopened.workspace(), harness.session.workspace());
    }

    #[test]
    fn empty_transactions_are_neither_committed_nor_recorded() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "same");
        harness.session.undo().unwrap();
        harness.session.redo().unwrap();
        let commits = harness.store.committed();
        let changes = harness
            .session
            .execute(Command::EditText {
                block,
                text: RichText::plain("same"),
            })
            .unwrap();
        assert_eq!(changes, Changes::default());
        assert_eq!(harness.store.committed(), commits);
        assert!(harness.session.can_undo());
        assert!(!harness.session.can_redo());
    }

    #[test]
    fn empty_transactions_keep_the_caret() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        let point = TextPoint { block, offset: 0 };
        let changes = harness
            .session
            .execute(Command::DeleteRange {
                start: point,
                end: point,
            })
            .unwrap();
        assert_eq!(changes.caret, Some(point));
        assert!(changes.blocks.is_empty());
    }

    #[test]
    fn execute_reports_touched_blocks_created_ids_and_caret() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "Hello world");
        let changes = harness
            .session
            .execute(Command::SplitBlock {
                at: TextPoint { block, offset: 5 },
            })
            .unwrap();
        let created = created_block(&changes);
        assert_eq!(changes.blocks.len(), 2);
        assert!(changes.blocks.contains(&block));
        assert!(changes.blocks.contains(&created));
        assert_eq!(
            changes.caret,
            Some(TextPoint {
                block: created,
                offset: 0
            })
        );
    }

    #[test]
    fn undo_and_redo_report_touched_blocks_without_caret_or_created() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        edit(&mut harness, block, "a");
        let undone = harness.session.undo().unwrap().unwrap();
        assert!(undone.blocks.contains(&block));
        assert_eq!(undone.caret, None);
        assert!(undone.created.is_empty());
        let redone = harness.session.redo().unwrap().unwrap();
        assert!(redone.blocks.contains(&block));
        assert_eq!(redone.caret, None);
        assert!(redone.created.is_empty());
    }

    #[test]
    fn undoing_a_creation_removes_the_block_and_redo_brings_it_back() {
        let mut harness = harness();
        let block = page_with_block(&mut harness);
        harness.session.undo().unwrap();
        assert!(harness.session.workspace().block(block).is_none());
        harness.session.redo().unwrap();
        assert!(harness.session.workspace().block(block).is_some());
    }

    #[test]
    fn move_commands_are_undoable() {
        let mut harness = harness();
        let first = page_with_block(&mut harness);
        let second = created_block(
            &harness
                .session
                .execute(Command::InsertBlock {
                    after: first,
                    kind: BlockKind::Text,
                })
                .unwrap(),
        );
        let before = harness.session.workspace().clone();
        harness
            .session
            .execute(Command::Indent { block: second })
            .unwrap();
        assert_ne!(*harness.session.workspace(), before);
        harness.session.undo().unwrap();
        assert_eq!(*harness.session.workspace(), before);
        let target = DropTarget {
            parent: Parent::Block(first),
            after: None,
        };
        harness
            .session
            .execute(Command::MoveBlock {
                block: second,
                target,
            })
            .unwrap();
        harness.session.undo().unwrap();
        assert_eq!(*harness.session.workspace(), before);
    }

    #[test]
    fn the_session_clock_stamps_new_blocks() {
        let mut harness = harness();
        harness.clock.advance(5);
        let block = page_with_block(&mut harness);
        let stamped = harness.session.workspace().block(block).unwrap().created_at;
        assert_eq!(stamped, harness.clock.now());
    }

    #[test]
    fn changes_of_collects_ids_by_kind_of_operation() {
        let block = BlockId::new();
        let database = DatabaseId::new();
        let transaction = crate::domain::Transaction::new(vec![
            Operation::SetBlockAlive {
                block,
                alive: false,
            },
            Operation::RemoveDatabase { database },
        ]);
        let changes = changes_of(&transaction, Vec::new(), None);
        assert!(changes.blocks.contains(&block));
        assert!(changes.databases.contains(&database));
        assert!(changes.views.is_empty());
    }

    #[test]
    fn open_on_an_empty_store_gives_an_empty_workspace() {
        let store = MemoryStore::with_snapshot(crate::domain::WorkspaceSnapshot::default());
        let session = Session::open(store, ManualClock::new(Timestamp::from_millis(0))).unwrap();
        assert_eq!(*session.workspace(), Workspace::default());
    }
}
