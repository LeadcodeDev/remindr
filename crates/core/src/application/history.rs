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

    pub(crate) fn take_undo(&mut self) -> Option<Entry> {
        let entry = self.undo.pop()?;
        if let Some(top) = self.undo.last_mut() {
            top.edit = None;
        }
        Some(entry)
    }

    pub(crate) fn take_redo(&mut self) -> Option<Entry> {
        self.redo.pop()
    }

    pub(crate) fn push_undo(&mut self, mut entry: Entry) {
        entry.edit = None;
        self.undo.push(entry);
    }

    pub(crate) fn push_redo(&mut self, mut entry: Entry) {
        entry.edit = None;
        self.redo.push(entry);
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

    use super::{Edit, History};

    fn step(block: BlockId, alive: bool) -> Transaction {
        Transaction::new(vec![Operation::SetBlockAlive { block, alive }])
    }

    fn edit(block: BlockId, at: i64) -> Option<Edit> {
        Some(Edit {
            block,
            at: Timestamp::from_millis(at),
        })
    }

    #[test]
    fn a_fresh_history_has_nothing_to_undo_or_redo() {
        let mut history = History::default();
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert!(history.take_undo().is_none());
        assert!(history.take_redo().is_none());
    }

    #[test]
    fn consecutive_edits_on_one_block_merge_with_ordered_operations() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(block, false), step(block, true), edit(block, 400));
        let entry = history.take_undo().unwrap();
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
        assert!(history.take_undo().is_none());
    }

    #[test]
    fn merged_inverse_puts_the_new_inverse_first() {
        let block = BlockId::new();
        let other = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(other, true), step(other, false), edit(block, 10));
        let entry = history.take_undo().unwrap();
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
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_none());
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
        let mut count = 0;
        while history.take_undo().is_some() {
            count += 1;
        }
        assert_eq!(count, 4);
    }

    #[test]
    fn a_clock_that_goes_backwards_does_not_merge() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 500));
        history.record(step(block, true), step(block, false), edit(block, 100));
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_some());
    }

    #[test]
    fn recording_clears_the_redo_stack() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), None);
        let entry = history.take_undo().unwrap();
        history.push_redo(entry);
        assert!(history.can_redo());
        history.record(step(block, true), step(block, false), None);
        assert!(!history.can_redo());
    }

    #[test]
    fn taking_an_entry_seals_the_one_below_it() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        history.record(step(block, true), step(block, false), edit(block, 2_000));
        let entry = history.take_undo().unwrap();
        history.push_redo(entry);
        history.record(step(block, true), step(block, false), edit(block, 2_100));
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_none());
    }

    #[test]
    fn an_entry_pushed_back_on_the_undo_stack_never_merges() {
        let block = BlockId::new();
        let mut history = History::default();
        history.record(step(block, true), step(block, false), edit(block, 0));
        let entry = history.take_undo().unwrap();
        history.push_redo(entry);
        let entry = history.take_redo().unwrap();
        history.push_undo(entry);
        history.record(step(block, true), step(block, false), edit(block, 10));
        assert!(history.take_undo().is_some());
        assert!(history.take_undo().is_some());
    }
}
