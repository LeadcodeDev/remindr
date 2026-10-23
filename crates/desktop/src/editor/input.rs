use std::ops::Range;

use gpui_kit::{
    Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window,
    point,
};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::BlockId;

use super::caret::{Marked, Selection, byte_to_utf16, utf16_to_byte};
use super::frame::{CARET_WIDTH, Hit};
use super::rows::position;
use super::{BlockEditor, is_atomic_row};

impl BlockEditor {
    fn ime_block(&self, cx: &Context<Self>) -> Option<BlockId> {
        let block = self.selection?.head.block;
        let kind = self.kind_of(block, cx)?;
        (!is_atomic_row(&kind)).then_some(block)
    }

    fn displayed_text(&self, block: BlockId, cx: &Context<Self>) -> Option<String> {
        let found = self.block(block, cx)?;
        Some(match &self.marked {
            Some(marked) if marked.block == block => {
                super::text::composed(&found.text, marked).to_plain()
            }
            _ => found.text.to_plain(),
        })
    }

    pub(super) fn commit_marked(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(marked) = self.marked.take() else {
            return;
        };
        self.replace_in_block(marked.block, marked.replaced, &marked.text, window, cx);
        cx.notify();
    }

    fn replace_in_block(
        &mut self,
        block: BlockId,
        range: Range<usize>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let start = range.start;
        let edited = self
            .block(block, cx)
            .filter(|found| !is_atomic_row(&found.kind))
            .and_then(|found| {
                let marks = found.text.marks_at(start);
                found
                    .text
                    .delete(range)
                    .and_then(|rest| rest.insert(start, text, &marks))
                    .ok()
            });
        let Some(edited) = edited else {
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
                    offset: start + text.len(),
                },
                cx,
            );
        }
    }

    fn utf16_range(
        &self,
        block: BlockId,
        range: Range<usize>,
        cx: &Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.displayed_text(block, cx)?;
        Some(utf16_to_byte(&text, range.start)..utf16_to_byte(&text, range.end))
    }
}

impl EntityInputHandler for BlockEditor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted: &mut Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        let block = self.ime_block(cx)?;
        let text = self.displayed_text(block, cx)?;
        let bytes = utf16_to_byte(&text, range.start)..utf16_to_byte(&text, range.end);
        adjusted.replace(byte_to_utf16(&text, bytes.start)..byte_to_utf16(&text, bytes.end));
        text.get(bytes).map(str::to_owned)
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let block = self.ime_block(cx)?;
        let text = self.displayed_text(block, cx)?;
        if let Some(marked) = self.marked.as_ref().filter(|marked| marked.block == block) {
            let cursor = marked.display_cursor();
            return Some(UTF16Selection {
                range: byte_to_utf16(&text, cursor.start)..byte_to_utf16(&text, cursor.end),
                reversed: false,
            });
        }
        let Selection { anchor, head } = self.selection?;
        let rows = self.visible_rows(cx);
        let anchor_offset = if anchor.block == head.block {
            anchor.offset
        } else if position(&rows, anchor.block)? < position(&rows, head.block)? {
            0
        } else {
            text.len()
        };
        let low = anchor_offset.min(head.offset);
        let high = anchor_offset.max(head.offset);
        Some(UTF16Selection {
            range: byte_to_utf16(&text, low)..byte_to_utf16(&text, high),
            reversed: head.offset < anchor_offset,
        })
    }

    fn marked_text_range(&self, _: &mut Window, cx: &mut Context<Self>) -> Option<Range<usize>> {
        let marked = self.marked.as_ref()?;
        let text = self.displayed_text(marked.block, cx)?;
        let range = marked.display_range();
        Some(byte_to_utf16(&text, range.start)..byte_to_utf16(&text, range.end))
    }

    fn unmark_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_marked(window, cx);
    }

    fn paste(&mut self, item: ClipboardItem, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = item.text() {
            self.commit_marked(window, cx);
            self.paste_text(&text, window, cx);
        }
    }

    fn replace_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(marked) = self.marked.take() {
            self.replace_in_block(marked.block, marked.replaced, text, window, cx);
            return;
        }
        let target = range.and_then(|range| {
            let block = self.ime_block(cx)?;
            Some((block, self.utf16_range(block, range, cx)?))
        });
        match target {
            Some((block, bytes)) => self.replace_in_block(block, bytes, text, window, cx),
            None => self.type_text(text, window, cx),
        }
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if new_text.is_empty() {
            self.marked = None;
            cx.notify();
            return;
        }
        let (block, replaced) = match &self.marked {
            Some(marked) => (marked.block, marked.replaced.clone()),
            None => {
                let Some(at) = self.insertion_point(window, cx) else {
                    return;
                };
                let replaced = range
                    .and_then(|range| self.utf16_range(at.block, range, cx))
                    .unwrap_or(at.offset..at.offset);
                (at.block, replaced)
            }
        };
        let atomic = self
            .kind_of(block, cx)
            .is_none_or(|kind| is_atomic_row(&kind));
        if atomic {
            return;
        }
        let cursor = new_selected_range.map_or(new_text.len()..new_text.len(), |selected| {
            utf16_to_byte(new_text, selected.start)..utf16_to_byte(new_text, selected.end)
        });
        let start = replaced.start;
        self.marked = Some(Marked {
            block,
            replaced,
            text: new_text.to_owned(),
            cursor,
        });
        self.set_caret(
            TextPoint {
                block,
                offset: start,
            },
            cx,
        );
    }

    fn bounds_for_range(
        &mut self,
        range: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let block = self.ime_block(cx)?;
        let bytes = self.utf16_range(block, range, cx)?;
        let frame = self.frame.borrow();
        let first = frame.caret_bounds(block, bytes.start, CARET_WIDTH)?;
        let last = frame.caret_bounds(block, bytes.end, CARET_WIDTH)?;
        Some(Bounds::from_corners(
            first.origin,
            point(
                last.origin.x.max(first.origin.x + CARET_WIDTH),
                first.origin.y + first.size.height,
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        at: Point<Pixels>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<usize> {
        let block = self.ime_block(cx)?;
        let Hit::Text(hit) = self.frame.borrow().hit(at)? else {
            return None;
        };
        if hit.block != block {
            return None;
        }
        let text = self.displayed_text(block, cx)?;
        Some(byte_to_utf16(&text, hit.offset))
    }
}
