use std::ops::Range;

use gpui_kit::{Context, Window};

use super::caret::{word_after, word_before};
use super::{BlockEditor, is_atomic_row};

#[derive(Clone, Copy)]
enum Reach {
    WordBackward,
    WordForward,
    LineBackward,
}

impl BlockEditor {
    pub(super) fn delete_word_backward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_reaching(Reach::WordBackward, window, cx);
    }

    pub(super) fn delete_word_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_reaching(Reach::WordForward, window, cx);
    }

    pub(super) fn delete_line_backward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.delete_reaching(Reach::LineBackward, window, cx);
    }

    fn delete_reaching(&mut self, reach: Reach, window: &mut Window, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        if !selection.is_collapsed() {
            self.delete_selection(selection, window, cx);
            return;
        }
        let at = selection.head;
        let editable = self
            .kind_of(at.block, cx)
            .is_some_and(|kind| !is_atomic_row(&kind));
        let plain = self.plain_of(at.block, cx).unwrap_or_default();
        let range: Option<Range<usize>> = editable
            .then(|| match reach {
                Reach::WordBackward => word_before(&plain, at.offset)..at.offset,
                Reach::WordForward => at.offset..word_after(&plain, at.offset),
                Reach::LineBackward => {
                    let edges = self.frame.borrow().line_edges(at.block, at.offset, &plain);
                    edges.map_or(0, |(start, _)| start)..at.offset
                }
            })
            .filter(|range| !range.is_empty());
        match (range, reach) {
            (Some(range), _) => self.delete_text(at.block, range, window, cx),
            (None, Reach::WordForward) => self.delete_forward(window, cx),
            (None, _) => self.backspace(window, cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockId, BlockKind};

    use crate::editor::testing::{Doc, Fixture};

    fn at(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn word() -> &'static str {
        if cfg!(target_os = "macos") {
            "alt"
        } else {
            "ctrl"
        }
    }

    #[gpui_kit::test]
    fn option_backspace_deletes_a_word(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "hello big world");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 15));
        fixture.press(cx, &format!("{}-backspace", word()));
        assert_eq!(fixture.text(cx, block), "hello big ");
        assert_eq!(fixture.caret(cx), Some(at(block, 10)));
        fixture.press(cx, &format!("{}-backspace", word()));
        assert_eq!(fixture.text(cx, block), "hello ");
    }

    #[gpui_kit::test]
    fn option_backspace_in_the_middle_of_a_word_deletes_to_its_start(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 9));
        fixture.press(cx, &format!("{}-backspace", word()));
        assert_eq!(fixture.text(cx, block), "hello ld");
        assert_eq!(fixture.caret(cx), Some(at(block, 6)));
    }

    #[gpui_kit::test]
    fn option_backspace_at_the_start_merges_like_backspace(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "ab");
        let second = doc.child(BlockKind::Text, "cd");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(second, 0));
        fixture.press(cx, &format!("{}-backspace", word()));
        assert_eq!(fixture.texts(cx), vec!["abcd".to_owned()]);
        assert_eq!(fixture.caret(cx), Some(at(first, 2)));
    }

    #[gpui_kit::test]
    fn option_backspace_with_a_selection_deletes_the_selection(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 2), at(block, 7));
        fixture.press(cx, &format!("{}-backspace", word()));
        assert_eq!(fixture.text(cx, block), "heorld");
    }

    #[gpui_kit::test]
    fn option_delete_deletes_the_next_word(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "hello big world");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 0));
        fixture.press(cx, &format!("{}-delete", word()));
        assert_eq!(fixture.text(cx, block), " big world");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[gpui_kit::test]
    fn option_delete_at_the_end_pulls_the_next_block_up(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "ab");
        doc.child(BlockKind::Text, "cd");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(first, 2));
        fixture.press(cx, &format!("{}-delete", word()));
        assert_eq!(fixture.texts(cx), vec!["abcd".to_owned()]);
    }

    #[cfg(target_os = "macos")]
    #[gpui_kit::test]
    fn cmd_backspace_deletes_to_the_start_of_the_visual_line(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let long = "abcd ".repeat(60);
        let block = doc.child(BlockKind::Text, &long);
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 150));
        fixture.press(cx, "home");
        let start = fixture.caret(cx).expect("caret").offset;
        assert!(start > 0 && start < 150);
        fixture.focus_at(cx, at(block, 150));
        fixture.press(cx, "cmd-backspace");
        let expected = format!("{}{}", &long[..start], &long[150..]);
        assert_eq!(fixture.text(cx, block), expected);
        assert_eq!(fixture.caret(cx), Some(at(block, start)));
    }

    #[cfg(target_os = "macos")]
    #[gpui_kit::test]
    fn cmd_backspace_on_a_short_line_deletes_the_text_before_the_caret(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 5));
        fixture.press(cx, "cmd-backspace");
        assert_eq!(fixture.text(cx, block), " world");
        assert_eq!(fixture.caret(cx), Some(at(block, 0)));
    }

    #[cfg(target_os = "macos")]
    #[gpui_kit::test]
    fn cmd_backspace_at_the_line_start_behaves_like_backspace(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "ab");
        let second = doc.child(BlockKind::Text, "cd");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(second, 0));
        fixture.press(cx, "cmd-backspace");
        assert_eq!(fixture.texts(cx), vec!["abcd".to_owned()]);
        assert_eq!(fixture.caret(cx), Some(at(first, 2)));
    }
}
