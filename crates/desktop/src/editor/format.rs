use gpui_kit::{Context, Window};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::{BlockKind, MarkChange, Marks};

use super::rows::{Expansion, extent, rows};
use super::{BlockEditor, is_atomic_row};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strikethrough,
    Code,
}

impl Mark {
    fn change(self, on: bool) -> MarkChange {
        match self {
            Self::Bold => MarkChange::Bold(on),
            Self::Italic => MarkChange::Italic(on),
            Self::Underline => MarkChange::Underline(on),
            Self::Strikethrough => MarkChange::Strikethrough(on),
            Self::Code => MarkChange::Code(on),
        }
    }

    fn is_on(self, marks: &Marks) -> bool {
        match self {
            Self::Bold => marks.bold,
            Self::Italic => marks.italic,
            Self::Underline => marks.underline,
            Self::Strikethrough => marks.strikethrough,
            Self::Code => marks.code,
        }
    }

    fn set(self, marks: &mut Marks, on: bool) {
        match self {
            Self::Bold => marks.bold = on,
            Self::Italic => marks.italic = on,
            Self::Underline => marks.underline = on,
            Self::Strikethrough => marks.strikethrough = on,
            Self::Code => marks.code = on,
        }
    }
}

pub struct Pending {
    pub at: TextPoint,
    pub marks: Marks,
}

impl BlockEditor {
    pub(super) fn toggle_mark(&mut self, mark: Mark, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        if selection.is_collapsed() {
            self.toggle_pending(mark, selection.head, cx);
            return;
        }
        let workspace = self.model.read(cx).workspace();
        let flow = rows(workspace, self.page, Expansion::All);
        let Some(span) = extent(&flow, selection.anchor, selection.head) else {
            return;
        };
        let covered = flow.get(span.first..=span.last).unwrap_or_default();
        let mut marked = 0;
        let mut unmarked = 0;
        for (index, row) in covered.iter().enumerate() {
            let Some(block) = workspace.block(row.block) else {
                continue;
            };
            if is_atomic_row(&block.kind) || matches!(block.kind, BlockKind::Page { .. }) {
                continue;
            }
            let begin = if index == 0 { span.start.offset } else { 0 };
            let finish = if index + 1 == covered.len() {
                span.end.offset
            } else {
                block.text.len()
            };
            if begin >= finish {
                continue;
            }
            if block.text.has_mark(begin..finish, &mark.change(true)) {
                marked += 1;
            } else {
                unmarked += 1;
            }
        }
        if marked + unmarked == 0 {
            return;
        }
        let command = Command::SetMarks {
            start: span.start,
            end: span.end,
            change: mark.change(unmarked > 0),
        };
        self.dispatch(command, window, cx);
    }

    fn toggle_pending(&mut self, mark: Mark, at: TextPoint, cx: &mut Context<Self>) {
        let Some(block) = self
            .block(at.block, cx)
            .filter(|block| !is_atomic_row(&block.kind))
        else {
            return;
        };
        let mut marks = match &self.pending {
            Some(pending) if pending.at == at => pending.marks.clone(),
            _ => block.text.marks_at(at.offset),
        };
        let flipped = !mark.is_on(&marks);
        mark.set(&mut marks, flipped);
        self.pending = Some(Pending { at, marks });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockId, BlockKind, Marks, RichText, Span};

    use crate::editor::testing::{Doc, Fixture};

    fn at(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn bold() -> Marks {
        Marks {
            bold: true,
            ..Marks::default()
        }
    }

    fn plain() -> Marks {
        Marks::default()
    }

    fn spans(items: &[(&str, Marks)]) -> Vec<(String, Marks)> {
        items
            .iter()
            .map(|(text, marks)| ((*text).to_owned(), marks.clone()))
            .collect()
    }

    #[gpui_kit::test]
    fn cmd_b_on_world_stores_a_bold_span(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "Hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 6), at(block, 11));
        fixture.press(cx, "secondary-b");
        assert_eq!(
            fixture.spans(cx, block),
            spans(&[("Hello ", plain()), ("world", bold())])
        );
        let selection = fixture.selection(cx).expect("selection kept");
        assert_eq!(
            (selection.anchor, selection.head),
            (at(block, 6), at(block, 11))
        );
    }

    #[gpui_kit::test]
    fn cmd_b_twice_removes_bold(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "Hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 6), at(block, 11));
        fixture.press(cx, "secondary-b");
        fixture.press(cx, "secondary-b");
        assert_eq!(fixture.spans(cx, block), spans(&[("Hello world", plain())]));
    }

    #[gpui_kit::test]
    fn cmd_b_over_partly_bold_text_bolds_everything(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "ab");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 1), at(block, 2));
        fixture.press(cx, "secondary-b");
        fixture.select(cx, at(block, 0), at(block, 2));
        fixture.press(cx, "secondary-b");
        assert_eq!(fixture.spans(cx, block), spans(&[("ab", bold())]));
    }

    #[gpui_kit::test]
    fn cmd_b_across_two_blocks_bolds_both_ranges(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "Hello");
        let second = doc.child(BlockKind::Text, "world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(first, 2), at(second, 3));
        fixture.press(cx, "secondary-b");
        assert_eq!(
            fixture.spans(cx, first),
            spans(&[("He", plain()), ("llo", bold())])
        );
        assert_eq!(
            fixture.spans(cx, second),
            spans(&[("wor", bold()), ("ld", plain())])
        );
        fixture.press(cx, "secondary-b");
        assert_eq!(fixture.spans(cx, first), spans(&[("Hello", plain())]));
        assert_eq!(fixture.spans(cx, second), spans(&[("world", plain())]));
    }

    #[gpui_kit::test]
    fn an_empty_block_in_the_range_does_not_block_the_toggle(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "ab");
        doc.child(BlockKind::Text, "");
        let third = doc.child(BlockKind::Text, "cd");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(first, 0), at(third, 2));
        fixture.press(cx, "secondary-b");
        fixture.press(cx, "secondary-b");
        assert_eq!(fixture.spans(cx, first), spans(&[("ab", plain())]));
        assert_eq!(fixture.spans(cx, third), spans(&[("cd", plain())]));
    }

    #[gpui_kit::test]
    fn each_formatting_shortcut_toggles_its_mark(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        type Probe = fn(&Marks) -> bool;
        let cases: [(&str, Probe); 4] = [
            ("secondary-i", |marks| marks.italic),
            ("secondary-u", |marks| marks.underline),
            ("secondary-shift-s", |marks| marks.strikethrough),
            ("secondary-e", |marks| marks.code),
        ];
        for (key, is_set) in cases {
            fixture.select(cx, at(block, 0), at(block, 4));
            fixture.press(cx, key);
            let marked = fixture.spans(cx, block);
            assert_eq!(marked.len(), 1, "{key}");
            assert!(is_set(&marked[0].1), "{key} sets its mark");
            fixture.press(cx, key);
            assert!(!is_set(&fixture.spans(cx, block)[0].1), "{key} clears it");
        }
    }

    #[gpui_kit::test]
    fn pending_bold_applies_to_typed_text(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "ab");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 2));
        fixture.press(cx, "secondary-b");
        fixture.input(cx, "cd");
        assert_eq!(
            fixture.spans(cx, block),
            spans(&[("ab", plain()), ("cd", bold())])
        );
        fixture.press(cx, "secondary-b");
        fixture.input(cx, "e");
        assert_eq!(
            fixture.spans(cx, block),
            spans(&[("ab", plain()), ("cd", bold()), ("e", plain())])
        );
    }

    #[gpui_kit::test]
    fn pending_bold_is_cleared_when_the_caret_moves(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "ab");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 2));
        fixture.press(cx, "secondary-b");
        fixture.press(cx, "left");
        fixture.press(cx, "right");
        fixture.input(cx, "c");
        assert_eq!(fixture.spans(cx, block), spans(&[("abc", plain())]));
    }

    #[gpui_kit::test]
    fn pending_bold_in_an_empty_block(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 0));
        fixture.press(cx, "secondary-b");
        fixture.input(cx, "x");
        assert_eq!(fixture.spans(cx, block), spans(&[("x", bold())]));
    }

    #[gpui_kit::test]
    fn typing_after_bold_text_continues_bold(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 0));
        fixture.press(cx, "secondary-b");
        fixture.input(cx, "x");
        fixture.input(cx, "y");
        assert_eq!(fixture.spans(cx, block), spans(&[("xy", bold())]));
    }

    #[gpui_kit::test]
    fn formatting_is_one_undo_step(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "Hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 6), at(block, 11));
        fixture.press(cx, "secondary-b");
        fixture.press(cx, "secondary-z");
        assert_eq!(fixture.spans(cx, block), spans(&[("Hello world", plain())]));
    }

    #[gpui_kit::test]
    fn formatting_skips_atomic_rows_in_the_range(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "ab");
        let divider = doc.child(BlockKind::Divider, "");
        let last = doc.child(BlockKind::Text, "cd");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(first, 0), at(last, 2));
        fixture.press(cx, "secondary-b");
        assert_eq!(fixture.spans(cx, first), spans(&[("ab", bold())]));
        assert_eq!(fixture.spans(cx, last), spans(&[("cd", bold())]));
        assert!(fixture.alive(cx, divider));
        assert!(!fixture.has_notification(cx));
    }

    #[test]
    fn rich_text_reports_marks_per_range() {
        let text = RichText::new([
            Span {
                text: "ab".to_owned(),
                marks: plain(),
            },
            Span {
                text: "cd".to_owned(),
                marks: bold(),
            },
        ]);
        assert!(text.has_mark(2..4, &remindr_core::domain::MarkChange::Bold(true)));
        assert!(!text.has_mark(1..4, &remindr_core::domain::MarkChange::Bold(true)));
    }
}
