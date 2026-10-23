use gpui_kit::component::WindowExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::{Context, Window};
use remindr_core::application::{Changes, Command, DropTarget, SessionError, TextPoint};
use remindr_core::domain::{BlockId, BlockKind, Parent};

use super::caret::{Selection, grapheme_after, grapheme_before};
use super::rows::{Expansion, extent, position, rows};
use super::{BlockEditor, is_atomic_row};

impl BlockEditor {
    pub(super) fn dispatch(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Changes> {
        match self.execute(command, cx) {
            Ok(changes) => {
                self.settle(&changes, cx);
                Some(changes)
            }
            Err(error) => {
                window.push_notification(Notification::error(error.to_string()), cx);
                None
            }
        }
    }

    pub(super) fn dispatch_quiet(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Changes> {
        match self.execute(command, cx) {
            Ok(changes) => {
                self.settle(&changes, cx);
                Some(changes)
            }
            Err(SessionError::Domain(_)) => None,
            Err(error @ SessionError::Store(_)) => {
                window.push_notification(Notification::error(error.to_string()), cx);
                None
            }
        }
    }

    fn execute(
        &mut self,
        command: Command,
        cx: &mut Context<Self>,
    ) -> Result<Changes, SessionError> {
        let result = self
            .model
            .update(cx, |model, cx| model.execute(command, cx));
        if result.is_ok() {
            self.own_events += 1;
        }
        result
    }

    pub(super) fn type_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        if text.is_empty() || self.modal_text(text, cx) {
            return;
        }
        let Some(at) = self.insertion_point(window, cx) else {
            return;
        };
        self.insert_at(at, text, window, cx);
    }

    pub(super) fn insert_at(
        &mut self,
        at: TextPoint,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(command) = self.insert_command(at, text, cx) else {
            return false;
        };
        if self.dispatch(command, window, cx).is_none() {
            return false;
        }
        self.set_caret(
            TextPoint {
                block: at.block,
                offset: at.offset + text.len(),
            },
            cx,
        );
        self.after_typed(at, text, window, cx);
        true
    }

    fn insert_command(&self, at: TextPoint, text: &str, cx: &Context<Self>) -> Option<Command> {
        let block = self.block(at.block, cx)?;
        if is_atomic_row(&block.kind) {
            return None;
        }
        let marks = match &self.pending {
            Some(pending) if pending.at == at => pending.marks.clone(),
            _ => block.text.marks_at(at.offset),
        };
        let edited = block.text.insert(at.offset, text, &marks).ok()?;
        Some(Command::EditText {
            block: at.block,
            text: edited,
        })
    }

    pub(super) fn insertion_point(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TextPoint> {
        match self.selection {
            None => self.first_block(window, cx),
            Some(selection) if selection.is_collapsed() => Some(selection.head),
            Some(selection) => self.delete_selection(selection, window, cx),
        }
    }

    pub(super) fn first_block(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TextPoint> {
        if !self.visible_rows(cx).is_empty() {
            return None;
        }
        let command = Command::InsertBlockAt {
            target: DropTarget {
                parent: Parent::Block(self.page),
                after: None,
            },
            kind: BlockKind::Text,
        };
        self.dispatch(command, window, cx)?.caret
    }

    pub(super) fn deletion_span(
        &self,
        selection: Selection,
        cx: &Context<Self>,
    ) -> Option<(TextPoint, TextPoint)> {
        let rows = self.visible_rows(cx);
        let anchor = self.valid_point(selection.anchor, cx);
        let head = self.valid_point(selection.head, cx);
        let span = extent(&rows, anchor, head)?;
        let end = if span.start.block == span.end.block {
            span.end
        } else {
            self.with_hidden_descendants(span.end, cx)
        };
        Some((span.start, end))
    }

    fn with_hidden_descendants(&self, end: TextPoint, cx: &Context<Self>) -> TextPoint {
        let collapsed = matches!(self.kind_of(end.block, cx), Some(BlockKind::Toggle))
            && !self.expanded.contains(&end.block)
            && end.offset == self.text_len(end.block, cx);
        if !collapsed {
            return end;
        }
        let flow = rows(self.model.read(cx).workspace(), self.page, Expansion::All);
        let Some(index) = position(&flow, end.block) else {
            return end;
        };
        let depth = flow[index].depth;
        let last = flow[index + 1..]
            .iter()
            .take_while(|row| row.depth > depth)
            .last();
        match last {
            Some(row) => TextPoint {
                block: row.block,
                offset: self.text_len(row.block, cx),
            },
            None => end,
        }
    }

    pub(super) fn delete_selection(
        &mut self,
        selection: Selection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<TextPoint> {
        let (start, end) = self.deletion_span(selection, cx)?;
        self.dispatch(Command::DeleteRange { start, end }, window, cx)?
            .caret
    }

    pub(super) fn backspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        if !selection.is_collapsed() {
            self.delete_selection(selection, window, cx);
            return;
        }
        let at = selection.head;
        let Some(kind) = self.kind_of(at.block, cx) else {
            return;
        };
        if is_atomic_row(&kind) {
            self.delete_atomic(at.block, window, cx);
            return;
        }
        if at.offset > 0 {
            let plain = self.plain_of(at.block, cx).unwrap_or_default();
            if let Some(start) = grapheme_before(&plain, at.offset) {
                self.delete_text(at.block, start..at.offset, window, cx);
            }
            return;
        }
        if kind != BlockKind::Text {
            let command = Command::ChangeKind {
                block: at.block,
                kind: BlockKind::Text,
            };
            self.dispatch(command, window, cx);
            return;
        }
        if let Some(previous) = self.flow_neighbor(at.block, false, cx) {
            self.reveal(previous, cx);
        }
        self.dispatch(Command::MergeWithPrevious { block: at.block }, window, cx);
    }

    pub(super) fn delete_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        if !selection.is_collapsed() {
            self.delete_selection(selection, window, cx);
            return;
        }
        let at = selection.head;
        let Some(kind) = self.kind_of(at.block, cx) else {
            return;
        };
        if is_atomic_row(&kind) {
            self.delete_atomic(at.block, window, cx);
            return;
        }
        let plain = self.plain_of(at.block, cx).unwrap_or_default();
        if let Some(end) = grapheme_after(&plain, at.offset) {
            self.delete_text(at.block, at.offset..end, window, cx);
            return;
        }
        if let Some(following) = self.flow_neighbor(at.block, true, cx) {
            self.reveal(following, cx);
            self.dispatch(Command::MergeWithPrevious { block: following }, window, cx);
        }
    }

    pub(super) fn delete_text(
        &mut self,
        block: BlockId,
        range: std::ops::Range<usize>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = range.start;
        let Some(edited) = self
            .block(block, cx)
            .and_then(|found| found.text.delete(range).ok())
        else {
            return;
        };
        let command = Command::EditText {
            block,
            text: edited,
        };
        if self.dispatch(command, window, cx).is_some() {
            self.set_caret(
                TextPoint {
                    block,
                    offset: start,
                },
                cx,
            );
        }
    }

    pub(super) fn delete_atomic(
        &mut self,
        block: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let rows = self.visible_rows(cx);
        let index = position(&rows, block);
        let neighbor = index.and_then(|index| {
            let before = index.checked_sub(1).and_then(|i| rows.get(i));
            before
                .map(|row| (row.block, true))
                .or_else(|| rows.get(index + 1).map(|row| (row.block, false)))
        });
        let command = Command::DeleteBlocks {
            blocks: vec![block],
        };
        if self.dispatch(command, window, cx).is_none() {
            return;
        }
        match neighbor {
            Some((target, at_end)) => {
                let offset = if at_end { self.text_len(target, cx) } else { 0 };
                self.set_caret(
                    TextPoint {
                        block: target,
                        offset,
                    },
                    cx,
                );
            }
            None => self.set_selection(None, cx),
        }
    }

    pub(super) fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.selection.is_none() {
            self.first_block(window, cx);
            return;
        }
        let Some(at) = self.insertion_point(window, cx) else {
            return;
        };
        match self.kind_of(at.block, cx) {
            Some(kind) if is_atomic_row(&kind) => self.insert_after(at.block, window, cx),
            Some(BlockKind::Code { .. }) => {
                self.insert_at(at, "\n", window, cx);
            }
            Some(_) => {
                self.dispatch(Command::SplitBlock { at }, window, cx);
            }
            None => {}
        }
    }

    pub(super) fn insert_below(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(selection) = self.selection {
            self.insert_after(selection.head.block, window, cx);
        }
    }

    fn insert_after(&mut self, block: BlockId, window: &mut Window, cx: &mut Context<Self>) {
        let command = Command::InsertBlock {
            after: block,
            kind: BlockKind::Text,
        };
        self.dispatch(command, window, cx);
    }

    pub(super) fn indent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        let block = selection.head.block;
        if self
            .dispatch_quiet(Command::Indent { block }, window, cx)
            .is_some()
        {
            self.open_parent_toggle(block, cx);
        }
    }

    pub(super) fn outdent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(selection) = self.selection {
            let block = selection.head.block;
            self.dispatch_quiet(Command::Outdent { block }, window, cx);
        }
    }

    pub(super) fn open_parent_toggle(&mut self, block: BlockId, cx: &mut Context<Self>) {
        let parent = self.block(block, cx).map(|found| found.parent);
        if let Some(Parent::Block(owner)) = parent
            && matches!(self.kind_of(owner, cx), Some(BlockKind::Toggle))
        {
            self.expanded.insert(owner);
            cx.notify();
        }
    }

    pub(super) fn toggle_todo(
        &mut self,
        block: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(BlockKind::ToDo { checked }) = self.kind_of(block, cx) else {
            return;
        };
        let command = Command::ChangeKind {
            block,
            kind: BlockKind::ToDo { checked: !checked },
        };
        self.dispatch(command, window, cx);
    }

    pub(super) fn toggle_open(&mut self, block: BlockId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&block) {
            self.expanded.insert(block);
        }
        self.clamp_selection(cx);
        cx.notify();
    }

    pub(super) fn undo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self.model.update(cx, |model, cx| model.undo(cx));
        self.finish_history(result, window, cx);
    }

    pub(super) fn redo(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let result = self.model.update(cx, |model, cx| model.redo(cx));
        self.finish_history(result, window, cx);
    }

    fn finish_history(
        &mut self,
        result: Result<Option<Changes>, SessionError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(Some(changes)) => {
                self.own_events += 1;
                self.settle(&changes, cx);
            }
            Ok(None) => {}
            Err(error) => {
                window.push_notification(Notification::error(error.to_string()), cx);
            }
        }
    }

    fn flow_neighbor(&self, block: BlockId, forward: bool, cx: &Context<Self>) -> Option<BlockId> {
        let flow = rows(self.model.read(cx).workspace(), self.page, Expansion::All);
        let index = position(&flow, block)?;
        let neighbor = if forward {
            flow.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|before| flow.get(before))
        };
        neighbor.map(|row| row.block)
    }

    fn reveal(&mut self, block: BlockId, cx: &mut Context<Self>) {
        let workspace = self.model.read(cx).workspace();
        let mut opened = Vec::new();
        let mut current = workspace.block(block).map(|found| found.parent);
        while let Some(Parent::Block(owner)) = current {
            if owner == self.page {
                break;
            }
            if workspace
                .block(owner)
                .is_some_and(|found| matches!(found.kind, BlockKind::Toggle))
            {
                opened.push(owner);
            }
            current = workspace.block(owner).map(|found| found.parent);
        }
        if !opened.is_empty() {
            self.expanded.extend(opened);
            cx.notify();
        }
    }
}
