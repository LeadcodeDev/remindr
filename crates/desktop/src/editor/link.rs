use gpui_kit::{
    AnyElement, App, Context, IntoElement, ParentElement, Pixels, Point, TestSupportExt, Window,
    div, prelude::*, px,
};
use remindr_core::application::{Command, TextPoint};
use remindr_core::domain::{BlockKind, Link, MarkChange};
use unicode_segmentation::UnicodeSegmentation;

use super::caret::grapheme_before;
use super::overlay::{Overlay, ROW_HEIGHT, surface};
use super::palette::Palette;
use super::rows::{Expansion, extent, rows};
use super::slash::single_line;
use super::{BlockEditor, is_atomic_row};
use crate::theme::UI_TEXT_SMALL;

pub struct LinkEntry {
    pub start: TextPoint,
    pub end: TextPoint,
    pub head: TextPoint,
    pub text: String,
    pub error: bool,
}

pub const ERROR_HEIGHT: Pixels = px(22.);
const INVALID: &str = "This is not a valid link";
const PLACEHOLDER: &str = "Paste or type a link";

impl BlockEditor {
    pub(super) fn open_link(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.selection.filter(|selection| !selection.is_collapsed()) else {
            return;
        };
        let workspace = self.model.read(cx).workspace();
        let flow = rows(workspace, self.page, Expansion::All);
        let Some(span) = extent(&flow, selection.anchor, selection.head) else {
            return;
        };
        let covered = flow.get(span.first..=span.last).unwrap_or_default();
        let mut shared: Option<Option<&Link>> = None;
        let mut agreed = true;
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
            let mut at = 0;
            for run in block.text.spans() {
                let (from, to) = (at, at + run.text.len());
                at = to;
                if from >= finish || to <= begin {
                    continue;
                }
                let link = run.marks.link.as_ref();
                match shared {
                    None => shared = Some(link),
                    Some(known) if known == link => {}
                    Some(_) => agreed = false,
                }
            }
        }
        let text = shared
            .flatten()
            .filter(|_| agreed)
            .map(|link| link.as_str().to_owned())
            .unwrap_or_default();
        self.overlay = Some(Overlay::Link(LinkEntry {
            start: span.start,
            end: span.end,
            head: selection.head,
            text,
            error: false,
        }));
        cx.notify();
    }

    pub(super) fn link_commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(Overlay::Link(entry)) = &mut self.overlay else {
            return;
        };
        let trimmed = entry.text.trim();
        let change = if trimmed.is_empty() {
            MarkChange::Link(None)
        } else {
            match Link::try_from(trimmed) {
                Ok(link) => MarkChange::Link(Some(link)),
                Err(_) => {
                    entry.error = true;
                    cx.notify();
                    return;
                }
            }
        };
        let (start, end) = (entry.start, entry.end);
        self.overlay = None;
        cx.notify();
        self.dispatch(Command::SetMarks { start, end, change }, window, cx);
    }

    pub(super) fn link_type(&mut self, text: &str, cx: &mut Context<Self>) {
        if let Some(Overlay::Link(entry)) = &mut self.overlay {
            entry.text.push_str(&single_line(text));
            entry.error = false;
            cx.notify();
        }
    }

    pub(super) fn link_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(Overlay::Link(entry)) = &mut self.overlay {
            if let Some((last, _)) = entry.text.grapheme_indices(true).next_back() {
                entry.text.truncate(last);
            }
            entry.error = false;
            cx.notify();
        }
    }

    pub(super) fn open_link_under(
        &self,
        at: TextPoint,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(url) = self.link_under(at, position, cx) else {
            return false;
        };
        cx.open_url(&url);
        true
    }

    fn link_under(&self, at: TextPoint, position: Point<Pixels>, cx: &App) -> Option<String> {
        let block = self.block(at.block, cx)?;
        let plain = block.text.to_plain();
        let frame = self.frame.borrow();
        let leaf = frame.leaf(at.block)?;
        let boundary = leaf.position_for_index(at.offset.min(leaf.len()));
        let first = if position.x < boundary.x {
            grapheme_before(&plain, at.offset)?
        } else {
            at.offset
        };
        let next = plain.get(first..)?.graphemes(true).next()?;
        let last = first + next.len();
        let from = leaf.position_for_index(first);
        let to = leaf.position_for_index(last);
        let same_line = (from.y - to.y).abs() < leaf.line_height() / 2.;
        if !same_line || position.x < from.x || position.x > to.x {
            return None;
        }
        let mut start = 0;
        for run in block.text.spans() {
            let end = start + run.text.len();
            if (start..end).contains(&first) {
                return run.marks.link.as_ref().map(|link| link.as_str().to_owned());
            }
            start = end;
        }
        None
    }
}

pub fn link_box(entry: &LinkEntry, palette: &Palette) -> AnyElement {
    let shown = if entry.text.is_empty() {
        div()
            .text_color(palette.muted_foreground)
            .child(PLACEHOLDER)
            .into_any_element()
    } else {
        div().child(entry.text.clone()).into_any_element()
    };
    let field = div()
        .id("link-text")
        .aria_label(entry.text.clone())
        .test_support()
        .h(ROW_HEIGHT)
        .px(px(8.))
        .flex()
        .flex_row()
        .items_center()
        .child(shown)
        .child(
            div()
                .flex_none()
                .ml(px(1.))
                .w(px(1.5))
                .h(px(16.))
                .bg(palette.caret),
        );
    let error = entry.error.then(|| {
        div()
            .id("link-error")
            .test_support()
            .h(ERROR_HEIGHT)
            .px(px(8.))
            .flex()
            .items_center()
            .text_size(UI_TEXT_SMALL)
            .text_color(palette.danger)
            .child(INVALID)
    });
    surface("link-input", palette)
        .child(field)
        .children(error)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{Modifiers, TestAppContext, point, px};
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockId, BlockKind, Link, Marks, RichText, Span};

    use crate::editor::render::row_id;
    use crate::editor::testing::{Doc, Fixture};

    fn at(block: BlockId, offset: usize) -> TextPoint {
        TextPoint { block, offset }
    }

    fn linked(url: &str) -> Marks {
        Marks {
            link: Link::try_from(url).ok(),
            ..Marks::default()
        }
    }

    fn url_of(fixture: &Fixture, cx: &mut TestAppContext, block: BlockId) -> Vec<Option<String>> {
        fixture
            .spans(cx, block)
            .into_iter()
            .map(|(_, marks)| marks.link.map(|link| link.as_str().to_owned()))
            .collect()
    }

    #[gpui_kit::test]
    fn cmd_k_sets_a_link(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "Hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 6), at(block, 11));
        fixture.press(cx, "secondary-k");
        assert!(fixture.shown(cx, "link-input"));
        fixture.input(cx, "https://example.com");
        assert_eq!(
            fixture.label_of(cx, "link-text").as_deref(),
            Some("https://example.com")
        );
        fixture.press(cx, "enter");
        assert!(!fixture.shown(cx, "link-input"));
        assert_eq!(
            url_of(&fixture, cx, block),
            vec![None, Some("https://example.com".to_owned())]
        );
        assert_eq!(fixture.text(cx, block), "Hello world");
        let selection = fixture.selection(cx).expect("selection kept");
        assert_eq!(
            (selection.anchor, selection.head),
            (at(block, 6), at(block, 11))
        );
    }

    #[gpui_kit::test]
    fn cmd_k_is_prefilled_with_the_current_link(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |_, cx| {
            let text = RichText::new([
                Span {
                    text: "see ".to_owned(),
                    marks: Marks::default(),
                },
                Span {
                    text: "docs".to_owned(),
                    marks: linked("https://a.dev"),
                },
            ]);
            fixture
                .model
                .update(cx, |model, cx| {
                    model.execute(
                        remindr_core::application::Command::EditText { block, text },
                        cx,
                    )
                })
                .expect("edit");
        });
        fixture.select(cx, at(block, 4), at(block, 8));
        fixture.press(cx, "secondary-k");
        assert_eq!(
            fixture.label_of(cx, "link-text").as_deref(),
            Some("https://a.dev")
        );
        fixture.press(cx, "backspace");
        fixture.input(cx, "m");
        fixture.press(cx, "enter");
        assert_eq!(
            url_of(&fixture, cx, block),
            vec![None, Some("https://a.dem".to_owned())]
        );
    }

    #[gpui_kit::test]
    fn invalid_link_is_refused(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "Hello world");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 6), at(block, 11));
        fixture.press(cx, "secondary-k");
        fixture.input(cx, "not a link");
        fixture.press(cx, "enter");
        assert!(fixture.shown(cx, "link-input"));
        assert!(fixture.shown(cx, "link-error"));
        assert_eq!(url_of(&fixture, cx, block), vec![None]);
        fixture.press(cx, "escape");
        assert!(!fixture.shown(cx, "link-input"));
        assert_eq!(url_of(&fixture, cx, block), vec![None]);
    }

    #[gpui_kit::test]
    fn typing_after_an_error_clears_it_and_a_valid_link_is_accepted(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 0), at(block, 4));
        fixture.press(cx, "secondary-k");
        fixture.input(cx, "a b");
        fixture.press(cx, "enter");
        assert!(fixture.shown(cx, "link-error"));
        fixture.press(cx, "backspace");
        fixture.press(cx, "backspace");
        assert!(!fixture.shown(cx, "link-error"));
        fixture.input(cx, "c");
        assert_eq!(fixture.label_of(cx, "link-text").as_deref(), Some("ac"));
        fixture.press(cx, "enter");
        assert_eq!(url_of(&fixture, cx, block), vec![Some("ac".to_owned())]);
    }

    #[gpui_kit::test]
    fn an_empty_link_input_removes_the_link(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 0), at(block, 4));
        fixture.press(cx, "secondary-k");
        fixture.input(cx, "https://x.dev");
        fixture.press(cx, "enter");
        assert_eq!(
            url_of(&fixture, cx, block),
            vec![Some("https://x.dev".to_owned())]
        );
        fixture.press(cx, "secondary-k");
        for _ in 0..13 {
            fixture.press(cx, "backspace");
        }
        fixture.press(cx, "enter");
        assert_eq!(url_of(&fixture, cx, block), vec![None]);
    }

    #[gpui_kit::test]
    fn escape_closes_the_link_input_without_changes(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 0), at(block, 4));
        fixture.press(cx, "secondary-k");
        fixture.input(cx, "https://x.dev");
        fixture.press(cx, "escape");
        assert!(!fixture.shown(cx, "link-input"));
        assert_eq!(url_of(&fixture, cx, block), vec![None]);
        assert_eq!(fixture.text(cx, block), "word");
    }

    #[gpui_kit::test]
    fn clicking_in_the_page_closes_the_link_input(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let other = doc.child(BlockKind::Text, "other");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 0), at(block, 4));
        fixture.press(cx, "secondary-k");
        assert!(fixture.shown(cx, "link-input"));
        fixture.click_block(cx, other);
        assert!(!fixture.shown(cx, "link-input"));
        assert_eq!(url_of(&fixture, cx, block), vec![None]);
    }

    #[gpui_kit::test]
    fn cmd_k_without_a_selection_does_nothing(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        fixture.focus_at(cx, at(block, 2));
        fixture.press(cx, "secondary-k");
        assert!(!fixture.shown(cx, "link-input"));
    }

    #[gpui_kit::test]
    fn navigation_keys_are_ignored_while_the_link_input_is_open(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "word");
        let fixture = Fixture::open(cx, &doc);
        fixture.select(cx, at(block, 0), at(block, 4));
        fixture.press(cx, "secondary-k");
        fixture.press(cx, "left");
        fixture.press(cx, "up");
        fixture.press(cx, "secondary-a");
        assert!(fixture.shown(cx, "link-input"));
        let selection = fixture.selection(cx).expect("selection");
        assert_eq!(
            (selection.anchor, selection.head),
            (at(block, 0), at(block, 4))
        );
        assert_eq!(fixture.text(cx, block), "word");
    }

    fn doc_with_link() -> (Doc, BlockId) {
        let mut doc = Doc::new();
        let block = doc.child(BlockKind::Text, "");
        (doc, block)
    }

    fn link_fixture(cx: &mut TestAppContext) -> (Fixture, BlockId) {
        let (doc, block) = doc_with_link();
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |_, cx| {
            let text = RichText::new([
                Span {
                    text: "xx".to_owned(),
                    marks: Marks::default(),
                },
                Span {
                    text: "link".to_owned(),
                    marks: linked("https://a.dev"),
                },
            ]);
            fixture
                .model
                .update(cx, |model, cx| {
                    model.execute(
                        remindr_core::application::Command::EditText { block, text },
                        cx,
                    )
                })
                .expect("edit");
        });
        (fixture, block)
    }

    fn click_text_at(
        fixture: &Fixture,
        cx: &mut TestAppContext,
        block: BlockId,
        offset: usize,
        modifiers: Modifiers,
    ) {
        use crate::editor::caret::Selection;
        let _: Option<Selection> = None;
        fixture.focus_at(cx, at(block, 0));
        let x = fixture.with(cx, |_, cx| {
            let editor = fixture.editor.read(cx);
            let frame = editor.frame.borrow();
            let leaf = frame.leaf(block).expect("leaf");
            let left = leaf.position_for_index(offset.saturating_sub(1)).x;
            let right = leaf.position_for_index(offset).x;
            (left + right) / 2.
        });
        let origin = fixture.with(cx, |window, _| window.find(row_id(block)).bounds().origin);
        fixture.with(cx, |window, cx| {
            window.click_with_options(
                row_id(block),
                gpui_kit::test::ClickOptions::new()
                    .with_modifiers(modifiers)
                    .with_offset(point(x - origin.x, px(8.))),
                cx,
            );
        });
    }

    #[gpui_kit::test]
    fn cmd_click_on_a_link_opens_it(cx: &mut TestAppContext) {
        let (fixture, block) = link_fixture(cx);
        click_text_at(&fixture, cx, block, 4, Modifiers::command());
        assert_eq!(cx.opened_url().as_deref(), Some("https://a.dev"));
    }

    #[gpui_kit::test]
    fn a_plain_click_on_a_link_places_the_caret(cx: &mut TestAppContext) {
        let (fixture, block) = link_fixture(cx);
        click_text_at(&fixture, cx, block, 4, Modifiers::default());
        assert_eq!(cx.opened_url(), None);
        let caret = fixture.caret(cx).expect("caret");
        assert_eq!(caret.block, block);
        assert!((3..=5).contains(&caret.offset));
    }

    #[gpui_kit::test]
    fn cmd_click_outside_a_link_places_the_caret(cx: &mut TestAppContext) {
        let (fixture, block) = link_fixture(cx);
        click_text_at(&fixture, cx, block, 1, Modifiers::command());
        assert_eq!(cx.opened_url(), None);
        assert!(fixture.caret(cx).is_some());
    }
}
