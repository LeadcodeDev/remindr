use crate::domain::{BlockId, Timestamp, Transaction};

const COALESCE_WINDOW_MS: i64 = 1_000;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Edit {
    pub(crate) block: BlockId,
    pub(crate) at: Timestamp,
}

#[derive(Clone, Debug)]
pub(crate) struct Entry {
    pub(crate) forward: Transaction,
    pub(crate) inverse: Transaction,
    edit: Option<Edit>,
}

#[derive(Debug, Default)]
pub(crate) struct History {
    undo: Vec<Entry>,
    redo: Vec<Entry>,
}

impl History {
    pub(crate) fn record(
        &mut self,
        forward: Transaction,
        inverse: Transaction,
        edit: Option<Edit>,
    ) {
        self.redo.clear();
        if let Some(top) = self.undo.last_mut()
            && let (Some(previous), Some(next)) = (top.edit, edit)
            && previous.block == next.block
            && (0..COALESCE_WINDOW_MS).contains(&(next.at.as_millis() - previous.at.as_millis()))
        {
            top.forward = chain(&top.forward, &forward);
            top.inverse = chain(&inverse, &top.inverse);
            top.edit = Some(next);
            return;
        }
        self.undo.push(Entry {
            forward,
            inverse,
            edit,
        });
    }

    pub(crate) fn peek_undo(&self) -> Option<&Entry> {
        self.undo.last()
    }

    pub(crate) fn peek_redo(&self) -> Option<&Entry> {
        self.redo.last()
    }

    pub(crate) fn finish_undo(&mut self) {
        if let Some(mut entry) = self.undo.pop() {
            if let Some(top) = self.undo.last_mut() {
                top.edit = None;
            }
            entry.edit = None;
            self.redo.push(entry);
        }
    }

    pub(crate) fn finish_redo(&mut self) {
        if let Some(mut entry) = self.redo.pop() {
            entry.edit = None;
            self.undo.push(entry);
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

fn chain(first: &Transaction, second: &Transaction) -> Transaction {
    Transaction::new(
        first
            .operations()
            .iter()
            .chain(second.operations())
            .cloned()
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use crate::domain::{BlockId, Operation, Timestamp, Transaction};

    use super::{Edit, Entry, History};

    fn step(block: BlockId, alive: bool) -> Transaction {
        Transaction::new(vec![Operation::SetBlockAlive { block, alive }])
    }

    fn edit(block: BlockId, at: i64) -> Option<Edit> {
        Some(Edit {
            block,
            at: Timestamp::from_millis(at),
        })
    }

    fn undo_one(history: &mut History) -> Option<Entry> {
        let entry = history.peek_undo().cloned();
        history.finish_undo();
        entry
    }

    fn redo_one(history: &mut History) -> Option<Entry> {
        let entry = history.peek_redo().cloned();
        history.finish_redo();
        entry
    }

    fn undo_all(history: &mut History) -> usize {
        let mut count = 0;
        while undo_one(history).is_some() {
            count += 1;
        }
        count
    }

    #[test]
    fn a_fresh_history_has_nothing_to_undo_or_redo() {
        let mut history = History::default();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.peek_undo().is_none());
        assert!(history.peek_redo().is_none());
        history.finish_undo();
        history.finish_redo();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
    }

    #[test]
    fn consecutive_edits_on_one_block_merge_with_ordered_operations() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(block, false), step(block, true), edit(block, 400));
        let entry = undo_one(&mut history).unwrap();
        assert_eq!(
            entry.forward.operations(),
            [
                Operation::SetBlockAlive { block, alive: true },
                Operation::SetBlockAlive {
                    block,
                    alive: false
                },
            ]
        );
        assert_eq!(
            entry.inverse.operations(),
            [
                Operation::SetBlockAlive { block, alive: true },
                Operation::SetBlockAlive {
                    block,
                    alive: false
                },
            ]
        );
        assert!(undo_one(&mut history).is_none());
    }

    #[test]
    fn merged_inverse_puts_the_new_inverse_first() {
        let block = BlockId::new();
        let other = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(other, true), step(other, false), edit(block, 10));
        let entry = undo_one(&mut history).unwrap();
        assert_eq!(
            entry.inverse.operations(),
            [
                Operation::SetBlockAlive {
                    block: other,
                    alive: false
                },
                Operation::SetBlockAlive {
                    block,
                    alive: false
                },
            ]
        );
    }

    #[test]
    fn the_window_is_measured_from_the_last_merged_edit_and_is_strict() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(block, true), step(block, false), edit(block, 999));
        history.record(step(block, true), step(block, false), edit(block, 1_998));
        history.record(step(block, true), step(block, false), edit(block, 2_998));
        assert_eq!(undo_all(&mut history), 2);
    }

    #[test]
    fn edits_on_different_blocks_or_after_other_commands_do_not_merge() {
        let first = BlockId::new();
        let second = BlockId::new();
        let mut history = History::default();
        history.record(step(first, true), step(first, false), edit(first, 0));
        history.record(step(second, true), step(second, false), edit(second, 10));
        history.record(step(second, false), step(second, true), None);
        history.record(step(second, true), step(second, false), edit(second, 20));
        assert_eq!(undo_all(&mut history), 4);
    }

    #[test]
    fn a_clock_that_goes_backwards_does_not_merge() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 500));
        history.record(step(block, true), step(block, false), edit(block, 100));
        assert_eq!(undo_all(&mut history), 2);
    }

    #[test]
    fn recording_clears_the_redo_stack() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), None);
        history.finish_undo();
        assert!(history.can_redo());
        history.record(step(block, true), step(block, false), None);
        assert!(!history.can_redo());
    }

    #[test]
    fn finishing_an_undo_seals_the_entry_below_it() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(block, true), step(block, false), edit(block, 2_000));
        history.finish_undo();
        history.record(step(block, true), step(block, false), edit(block, 2_100));
        assert_eq!(undo_all(&mut history), 2);
    }

    #[test]
    fn peeking_leaves_the_history_and_its_coalescing_state_unchanged() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        assert!(history.peek_undo().is_some());
        assert!(history.peek_redo().is_none());
        history.record(step(block, true), step(block, false), edit(block, 200));
        assert_eq!(undo_all(&mut history), 1);
    }

    #[test]
    fn an_entry_moved_back_to_the_undo_stack_never_merges() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.finish_undo();
        history.finish_redo();
        history.record(step(block, true), step(block, false), edit(block, 10));
        assert_eq!(undo_all(&mut history), 2);
    }

    #[test]
    fn redo_returns_entries_in_the_order_they_were_undone() {
        let first = BlockId::new();
        let second = BlockId::new();
        let mut history = History::default();
        history.record(step(first, true), step(first, false), None);
        history.record(step(second, true), step(second, false), None);
        undo_one(&mut history);
        undo_one(&mut history);
        let entry = redo_one(&mut history).unwrap();
        assert_eq!(entry.forward, step(first, true));
        let entry = redo_one(&mut history).unwrap();
        assert_eq!(entry.forward, step(second, true));
        assert!(redo_one(&mut history).is_none());
    }
}
