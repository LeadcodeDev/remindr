use std::time::Duration;

use gpui_kit::test::{TestInput, TestWindowExt};
use gpui_kit::{
    ClipboardItem, Context, ElementInputHandler, IntoElement, Render, TestAppContext,
    TestSupportExt, Window, div, point, prelude::*, px,
};
use remindr_core::application::{Command, Created, TextPoint};
use remindr_core::domain::{BlockKind, HeadingLevel, Parent};

use super::caret::Selection;
use super::frame::{CARET_WIDTH, leaf_key};
use super::palette::Palette;
use super::render::{checkbox_id, chevron_id, row_id};
use super::testing::{Doc, Fixture};
use crate::app::{Navigator, NavigatorHandle, Route};
use crate::embed::EmbedRegistry;

fn at(block: remindr_core::domain::BlockId, offset: usize) -> TextPoint {
    TextPoint { block, offset }
}

#[gpui_kit::test]
fn renders_one_row_per_visible_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "alpha");
    let title = doc.child(BlockKind::Heading(HeadingLevel::One), "Title");
    let toggle = doc.child(BlockKind::Toggle, "more");
    let hidden = doc.under(toggle, BlockKind::Text, "hidden");
    let divider = doc.child(BlockKind::Divider, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, _| {
        let tops: Vec<_> = [first, title, toggle, divider]
            .into_iter()
            .map(|block| window.find(row_id(block)).bounds().top())
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(window.try_find(row_id(hidden)).is_none());
    });
}

#[gpui_kit::test]
fn chevron_click_shows_the_children_of_a_toggle(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let toggle = doc.child(BlockKind::Toggle, "more");
    let hidden = doc.under(toggle, BlockKind::Text, "hidden");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| {
        assert!(window.try_find(row_id(hidden)).is_none());
        window.click(chevron_id(toggle), cx);
        assert!(window.try_find(row_id(hidden)).is_some());
        window.click(chevron_id(toggle), cx);
        assert!(window.try_find(row_id(hidden)).is_none());
    });
}

#[gpui_kit::test]
fn click_places_the_caret_in_the_clicked_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "alpha");
    let second = doc.child(BlockKind::Text, "beta");
    let fixture = Fixture::open(cx, &doc);
    fixture.click_block(cx, second);
    assert_eq!(fixture.caret(cx), Some(at(second, 4)));
    fixture.with(cx, |window, cx| {
        window.click_at(row_id(first), point(px(54.), px(10.)), cx);
    });
    let caret = fixture.caret(cx).expect("caret");
    assert_eq!(caret.block, first);
    assert!(caret.offset < 5);
}

#[gpui_kit::test]
fn click_on_empty_page_creates_a_first_block(cx: &mut TestAppContext) {
    let doc = Doc::new();
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| window.click("block-editor", cx));
    let texts = fixture.texts(cx);
    assert_eq!(texts, vec![String::new()]);
    let caret = fixture.caret(cx).expect("caret in the new block");
    assert_eq!(caret.offset, 0);
    fixture.input(cx, "hi");
    assert_eq!(fixture.texts(cx), vec!["hi".to_owned()]);
}

#[gpui_kit::test]
fn typing_on_an_empty_page_creates_the_first_block(cx: &mut TestAppContext) {
    let doc = Doc::new();
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| {
        let focus = fixture.editor.read(cx).focus.clone();
        window.focus(&focus, cx);
        window.render_frame(cx);
    });
    fixture.input(cx, "Hello");
    assert_eq!(fixture.texts(cx), vec!["Hello".to_owned()]);
    assert_eq!(
        fixture.caret(cx).map(|point| point.offset),
        Some("Hello".len())
    );
}

#[gpui_kit::test]
fn drag_selects_across_two_blocks(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "alpha");
    let second = doc.child(BlockKind::Text, "beta");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| {
        let from = window.find(row_id(first)).bounds();
        let to = window.find(row_id(second)).bounds();
        window.drag(
            point(from.left() + px(60.), from.center().y),
            point(to.left() + px(60.), to.center().y),
            cx,
        );
    });
    let selection = fixture.with(cx, |_, cx| fixture.editor.read(cx).selection());
    let selection = selection.expect("selection");
    assert_eq!(selection.anchor.block, first);
    assert_eq!(selection.head.block, second);
    assert!(!selection.is_collapsed());
}

#[gpui_kit::test]
fn typing_inserts_text_in_the_caret_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "hello");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 5));
    fixture.input(cx, "!!");
    assert_eq!(fixture.text(cx, block), "hello!!");
    assert_eq!(fixture.caret(cx), Some(at(block, 7)));
}

#[gpui_kit::test]
fn typing_over_a_selection_replaces_it(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "alpha");
    let second = doc.child(BlockKind::Text, "beta");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 2));
    fixture.press(cx, "shift-down");
    assert_eq!(fixture.caret(cx).map(|point| point.block), Some(second));
    fixture.input(cx, "X");
    assert_eq!(fixture.texts(cx), vec!["alXta".to_owned()]);
}

#[gpui_kit::test]
fn enter_splits_the_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "Hello world");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 5));
    fixture.press(cx, "enter");
    assert_eq!(
        fixture.texts(cx),
        vec!["Hello".to_owned(), " world".to_owned()]
    );
    let caret = fixture.caret(cx).expect("caret");
    assert_ne!(caret.block, block);
    assert_eq!(caret.offset, 0);
}

#[gpui_kit::test]
fn enter_on_an_empty_list_item_turns_it_into_text(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let item = doc.child(BlockKind::BulletedListItem, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(item, 0));
    fixture.press(cx, "enter");
    let kind = fixture.with(cx, |_, cx| {
        fixture
            .model
            .read(cx)
            .workspace()
            .block(item)
            .map(|block| block.kind.clone())
    });
    assert_eq!(kind, Some(BlockKind::Text));
    assert_eq!(fixture.texts(cx).len(), 1);
}

#[gpui_kit::test]
fn shift_enter_inserts_a_soft_break(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "ab");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 1));
    fixture.press(cx, "shift-enter");
    assert_eq!(fixture.text(cx, block), "a\nb");
    assert_eq!(fixture.caret(cx), Some(at(block, 2)));
}

#[gpui_kit::test]
fn backspace_at_start_merges(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "ab");
    let second = doc.child(BlockKind::Text, "cd");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(second, 0));
    fixture.press(cx, "backspace");
    assert_eq!(fixture.texts(cx), vec!["abcd".to_owned()]);
    assert_eq!(fixture.caret(cx), Some(at(first, 2)));
}

#[gpui_kit::test]
fn backspace_over_a_range_across_blocks_leaves_one_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "Hello");
    let second = doc.child(BlockKind::Text, "world");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| {
        fixture.editor.update(cx, |editor, cx| {
            window.focus(&editor.focus, cx);
            editor.set_selection(
                Some(Selection {
                    anchor: at(first, 2),
                    head: at(second, 3),
                }),
                cx,
            );
        });
        window.render_frame(cx);
    });
    fixture.press(cx, "backspace");
    assert_eq!(fixture.texts(cx), vec!["Held".to_owned()]);
    assert_eq!(fixture.caret(cx), Some(at(first, 2)));
}

#[gpui_kit::test]
fn backspace_removes_the_previous_grapheme(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "a👍🏽");
    let fixture = Fixture::open(cx, &doc);
    let end = "a👍🏽".len();
    fixture.focus_at(cx, at(block, end));
    fixture.press(cx, "backspace");
    assert_eq!(fixture.text(cx, block), "a");
    assert_eq!(fixture.caret(cx), Some(at(block, 1)));
}

#[gpui_kit::test]
fn backspace_at_start_of_a_list_item_turns_it_into_text(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let item = doc.child(BlockKind::BulletedListItem, "item");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(item, 0));
    fixture.press(cx, "backspace");
    let kind = fixture.with(cx, |_, cx| {
        fixture
            .model
            .read(cx)
            .workspace()
            .block(item)
            .map(|block| block.kind.clone())
    });
    assert_eq!(kind, Some(BlockKind::Text));
}

#[gpui_kit::test]
fn delete_at_the_end_pulls_the_next_block_up(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "ab");
    doc.child(BlockKind::Text, "cd");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 2));
    fixture.press(cx, "delete");
    assert_eq!(fixture.texts(cx), vec!["abcd".to_owned()]);
    assert_eq!(fixture.caret(cx), Some(at(first, 2)));
}

#[gpui_kit::test]
fn tab_and_shift_tab_nest_and_unnest(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "one");
    let second = doc.child(BlockKind::Text, "two");
    let fixture = Fixture::open(cx, &doc);
    let parent_of = |cx: &mut TestAppContext| {
        fixture.with(cx, |_, cx| {
            fixture
                .model
                .read(cx)
                .workspace()
                .block(second)
                .map(|block| block.parent)
        })
    };
    fixture.focus_at(cx, at(second, 1));
    fixture.press(cx, "tab");
    assert_eq!(parent_of(cx), Some(Parent::Block(first)));
    assert_eq!(fixture.caret(cx), Some(at(second, 1)));
    fixture.press(cx, "shift-tab");
    assert_eq!(parent_of(cx), Some(Parent::Block(fixture.page)));
}

#[gpui_kit::test]
fn tab_on_a_first_child_changes_nothing(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "one");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 1));
    fixture.press(cx, "tab");
    let parent = fixture.with(cx, |_, cx| {
        fixture
            .model
            .read(cx)
            .workspace()
            .block(first)
            .map(|block| block.parent)
    });
    assert_eq!(parent, Some(Parent::Block(fixture.page)));
    assert_eq!(fixture.caret(cx), Some(at(first, 1)));
}

#[gpui_kit::test]
fn cmd_z_undoes_typing_in_one_step(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 0));
    fixture.input(cx, "abc");
    assert_eq!(fixture.text(cx, block), "abc");
    fixture.press(cx, "secondary-z");
    assert_eq!(fixture.text(cx, block), "");
    fixture.press(cx, "secondary-shift-z");
    assert_eq!(fixture.text(cx, block), "abc");
}

#[gpui_kit::test]
fn undo_of_a_split_moves_the_caret_back(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "hello");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 2));
    fixture.press(cx, "enter");
    fixture.press(cx, "secondary-z");
    assert_eq!(fixture.texts(cx), vec!["hello".to_owned()]);
    let caret = fixture.caret(cx).expect("caret");
    assert_eq!(caret.block, block);
}

#[gpui_kit::test]
fn arrow_down_moves_to_the_next_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "one");
    let second = doc.child(BlockKind::Text, "two");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 1));
    fixture.press(cx, "down");
    assert_eq!(fixture.caret(cx).map(|point| point.block), Some(second));
    fixture.press(cx, "up");
    assert_eq!(fixture.caret(cx).map(|point| point.block), Some(first));
}

#[gpui_kit::test]
fn left_and_right_cross_block_boundaries(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "ab");
    let second = doc.child(BlockKind::Text, "cd");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 2));
    fixture.press(cx, "right");
    assert_eq!(fixture.caret(cx), Some(at(second, 0)));
    fixture.press(cx, "left");
    assert_eq!(fixture.caret(cx), Some(at(first, 2)));
    fixture.press(cx, "left");
    assert_eq!(fixture.caret(cx), Some(at(first, 1)));
}

#[gpui_kit::test]
fn select_all_covers_the_page(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "one");
    let second = doc.child(BlockKind::Text, "two");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 1));
    fixture.press(cx, "secondary-a");
    let selection = fixture
        .with(cx, |_, cx| fixture.editor.read(cx).selection())
        .expect("selection");
    assert_eq!(selection.anchor, at(first, 0));
    assert_eq!(selection.head, at(second, 3));
}

#[gpui_kit::test]
fn ime_marked_text_is_not_committed_until_confirmed(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 0));
    fixture.with(cx, |window, cx| {
        let bounds = window.find("block-editor").bounds();
        let mut input = TestInput::new(
            ElementInputHandler::new(bounds, fixture.editor.clone()),
            window,
        );
        input.compose_text(None, "ni", Some(2..2), cx);
        assert_eq!(input.marked_text_range(cx), Some(0..2));
    });
    assert_eq!(fixture.text(cx, block), "");
    fixture.with(cx, |window, cx| {
        let bounds = window.find("block-editor").bounds();
        let mut input = TestInput::new(
            ElementInputHandler::new(bounds, fixture.editor.clone()),
            window,
        );
        input.compose_text(None, "你", Some(1..1), cx);
        input.commit_text("你", cx);
        assert_eq!(input.marked_text_range(cx), None);
    });
    assert_eq!(fixture.text(cx, block), "你");
    assert_eq!(fixture.caret(cx), Some(at(block, "你".len())));
}

#[gpui_kit::test]
fn cancelled_composition_leaves_the_text_untouched(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "ab");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 1));
    fixture.with(cx, |window, cx| {
        let bounds = window.find("block-editor").bounds();
        let mut input = TestInput::new(
            ElementInputHandler::new(bounds, fixture.editor.clone()),
            window,
        );
        input.compose_text(None, "zz", Some(2..2), cx);
        input.compose_text(None, "", None, cx);
        assert_eq!(input.marked_text_range(cx), None);
    });
    assert_eq!(fixture.text(cx, block), "ab");
}

#[gpui_kit::test]
fn checkbox_click_toggles_a_todo(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let todo = doc.child(BlockKind::ToDo { checked: false }, "task");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| window.click(checkbox_id(todo), cx));
    let kind = fixture.with(cx, |_, cx| {
        fixture
            .model
            .read(cx)
            .workspace()
            .block(todo)
            .map(|block| block.kind.clone())
    });
    assert_eq!(kind, Some(BlockKind::ToDo { checked: true }));
}

#[gpui_kit::test]
fn a_selected_divider_is_deleted_by_backspace(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "above");
    let divider = doc.child(BlockKind::Divider, "");
    doc.child(BlockKind::Text, "below");
    let fixture = Fixture::open(cx, &doc);
    fixture.click_block(cx, divider);
    assert_eq!(fixture.caret(cx), Some(at(divider, 0)));
    fixture.press(cx, "backspace");
    assert_eq!(
        fixture.texts(cx),
        vec!["above".to_owned(), "below".to_owned()]
    );
    assert_eq!(fixture.caret(cx), Some(at(first, 5)));
}

#[gpui_kit::test]
fn enter_on_a_selected_divider_adds_a_text_block_after_it(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let divider = doc.child(BlockKind::Divider, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.click_block(cx, divider);
    fixture.press(cx, "enter");
    assert_eq!(fixture.texts(cx), vec![String::new(), String::new()]);
    let caret = fixture.caret(cx).expect("caret");
    assert_ne!(caret.block, divider);
}

#[gpui_kit::test]
fn paste_of_three_lines_creates_two_new_blocks(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 0));
    cx.write_to_clipboard(ClipboardItem::new_string("one\ntwo\nthree".to_owned()));
    fixture.press(cx, "secondary-v");
    assert_eq!(
        fixture.texts(cx),
        vec!["one".to_owned(), "two".to_owned(), "three".to_owned()]
    );
    fixture.press(cx, "secondary-z");
    assert_eq!(fixture.texts(cx), vec![String::new()]);
}

#[gpui_kit::test]
fn cut_copies_markdown_and_deletes_the_selection(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let title = doc.child(BlockKind::Heading(HeadingLevel::One), "Title");
    let item = doc.child(BlockKind::BulletedListItem, "item");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(title, 0));
    fixture.press(cx, "secondary-a");
    fixture.press(cx, "secondary-x");
    let copied = cx.read_from_clipboard().and_then(|item| item.text());
    assert_eq!(copied.as_deref(), Some("# Title\n- item"));
    assert_eq!(fixture.texts(cx), vec![String::new()]);
    let _ = item;
}

fn caret_y(
    fixture: &Fixture,
    cx: &mut TestAppContext,
    block: remindr_core::domain::BlockId,
    offset: usize,
) -> f32 {
    fixture.with(cx, |_, cx| {
        fixture
            .editor
            .read(cx)
            .frame
            .borrow()
            .caret_bounds(block, offset, CARET_WIDTH)
            .map(|bounds| bounds.origin.y.as_f32())
            .expect("caret geometry")
    })
}

#[gpui_kit::test]
fn a_soft_break_starts_a_new_visual_line(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "a\nb");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 0));
    assert!(caret_y(&fixture, cx, block, 2) > caret_y(&fixture, cx, block, 0));
}

#[gpui_kit::test]
fn a_click_below_the_last_block_lands_in_it(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    doc.child(BlockKind::Text, "first");
    let last = doc.child(BlockKind::Text, "last");
    let fixture = Fixture::open(cx, &doc);
    fixture.with(cx, |window, cx| {
        window.click_at("block-editor", point(px(100.), px(560.)), cx);
    });
    assert_eq!(fixture.caret(cx).map(|point| point.block), Some(last));
}

#[gpui_kit::test]
fn home_and_end_stay_on_the_visual_line(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, &"abcd ".repeat(60));
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 150));
    let line = caret_y(&fixture, cx, block, 150);
    fixture.press(cx, "home");
    let start = fixture.caret(cx).expect("caret").offset;
    assert!(start > 0 && start <= 150);
    assert_eq!(caret_y(&fixture, cx, block, start), line);
    fixture.press(cx, "end");
    let end = fixture.caret(cx).expect("caret").offset;
    assert!((150..300).contains(&end));
    assert_eq!(caret_y(&fixture, cx, block, end), line);
}

#[gpui_kit::test]
fn arrow_down_moves_between_wrapped_lines_of_one_block(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, &"abcd ".repeat(60));
    doc.child(BlockKind::Text, "next");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 10));
    let first_line = caret_y(&fixture, cx, block, 10);
    fixture.press(cx, "down");
    let caret = fixture.caret(cx).expect("caret");
    assert_eq!(caret.block, block);
    assert!(caret.offset > 10);
    assert!(caret_y(&fixture, cx, block, caret.offset) > first_line);
    fixture.press(cx, "up");
    let back = fixture.caret(cx).expect("caret");
    assert_eq!(back.block, block);
    assert_eq!(caret_y(&fixture, cx, block, back.offset), first_line);
}

#[gpui_kit::test]
fn word_motion_jumps_over_words(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "hello big world");
    let fixture = Fixture::open(cx, &doc);
    let modifier = if cfg!(target_os = "macos") {
        "alt"
    } else {
        "ctrl"
    };
    fixture.focus_at(cx, at(block, 0));
    fixture.press(cx, &format!("{modifier}-right"));
    assert_eq!(fixture.caret(cx), Some(at(block, 5)));
    fixture.press(cx, &format!("{modifier}-right"));
    assert_eq!(fixture.caret(cx), Some(at(block, 9)));
    fixture.press(cx, &format!("{modifier}-left"));
    assert_eq!(fixture.caret(cx), Some(at(block, 6)));
}

#[gpui_kit::test]
fn selection_paints_decorations_per_leaf(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "alpha");
    let divider = doc.child(BlockKind::Divider, "");
    let second = doc.child(BlockKind::Text, "beta");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(first, 2));
    fixture.press(cx, "shift-down");
    fixture.press(cx, "shift-down");
    let (decorations, selected) = fixture.with(cx, |_, cx| {
        let snapshot = fixture.editor.read(cx).snapshot(&Palette::of(cx), cx);
        let selected: Vec<_> = snapshot
            .specs
            .iter()
            .filter(|spec| spec.selected)
            .map(|spec| spec.row.block)
            .collect();
        (snapshot.decorations, selected)
    });
    assert_eq!(
        decorations
            .get(&leaf_key(first))
            .map(|ranges| ranges[0].0.clone()),
        Some(2..5)
    );
    assert_eq!(selected, vec![divider]);
    let tail = decorations
        .get(&leaf_key(second))
        .map(|ranges| ranges[0].0.start);
    assert_eq!(tail, Some(0));
}

#[gpui_kit::test]
fn caret_blinks_every_500_ms_and_resets_on_edit(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "a");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 1));
    fixture.with(cx, |window, cx| window.render_frame(cx));
    let visible = |cx: &mut TestAppContext| {
        fixture.with(cx, |_, cx| fixture.editor.read(cx).blink.read(cx).visible())
    };
    assert!(visible(cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!visible(cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(visible(cx));
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    assert!(!visible(cx));
    fixture.input(cx, "b");
    assert!(visible(cx));
}

#[gpui_kit::test]
fn a_failed_commit_keeps_the_editor_state(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let block = doc.child(BlockKind::Text, "a");
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(block, 1));
    fixture.store.fail_next_commit();
    fixture.input(cx, "x");
    assert_eq!(fixture.text(cx, block), "a");
    assert_eq!(fixture.caret(cx), Some(at(block, 1)));
    fixture.input(cx, "y");
    assert_eq!(fixture.text(cx, block), "ay");
}

#[gpui_kit::test]
fn clicking_a_page_link_opens_the_page(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let sub = doc.child(BlockKind::Page { icon: None }, "Notes");
    let fixture = Fixture::open(cx, &doc);
    let navigator = cx.new(|_| Navigator::new());
    cx.update(|cx| cx.set_global(NavigatorHandle(navigator.clone())));
    fixture.with(cx, |window, cx| {
        window.click(("page-link", leaf_key(sub)), cx);
    });
    let route = cx.read(|cx| navigator.read(cx).route().copied());
    assert_eq!(route, Some(Route::Page(sub)));
    assert_eq!(fixture.caret(cx), None);
}

struct Embedded;

impl Render for Embedded {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("embedded").h(px(80.)).test_support()
    }
}

fn database_block(
    fixture: &Fixture,
    cx: &mut TestAppContext,
    after: remindr_core::domain::BlockId,
) -> remindr_core::domain::BlockId {
    let changes = fixture.with(cx, |_, cx| {
        fixture
            .model
            .update(cx, |model, cx| {
                model.execute(Command::CreateDatabaseWithView { after }, cx)
            })
            .expect("database block")
    });
    changes
        .created
        .iter()
        .find_map(|created| match created {
            Created::Block(block) => Some(*block),
            _ => None,
        })
        .expect("created block")
}

#[gpui_kit::test]
fn database_view_blocks_embed_the_registered_view(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "x");
    let fixture = Fixture::open(cx, &doc);
    cx.update(|cx| {
        let mut registry = EmbedRegistry::default();
        registry.register(|_, _, _, cx| cx.new(|_| Embedded).into());
        cx.set_global(registry);
    });
    let block = database_block(&fixture, cx, first);
    fixture.with(cx, |window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("embedded").is_some());
        assert!(window.try_find(row_id(block)).is_some());
    });
}

#[gpui_kit::test]
fn database_view_blocks_without_a_factory_show_a_placeholder(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let first = doc.child(BlockKind::Text, "x");
    let fixture = Fixture::open(cx, &doc);
    let block = database_block(&fixture, cx, first);
    fixture.with(cx, |window, cx| {
        window.render_frame(cx);
        assert!(window.find(row_id(block)).bounds().size.height >= px(120.));
    });
}

#[gpui_kit::test]
fn the_caret_is_scrolled_into_view(cx: &mut TestAppContext) {
    let mut doc = Doc::new();
    let mut last = doc.child(BlockKind::Text, "line 0");
    for number in 1..80 {
        last = doc.child(BlockKind::Text, &format!("line {number}"));
    }
    let fixture = Fixture::open(cx, &doc);
    fixture.focus_at(cx, at(last, 3));
    fixture.with(cx, |window, cx| {
        for _ in 0..3 {
            window.render_frame(cx);
        }
    });
    let bottom = fixture.with(cx, |_, cx| {
        fixture
            .editor
            .read(cx)
            .frame
            .borrow()
            .caret_bounds(last, 3, CARET_WIDTH)
            .map(|bounds| bounds.bottom())
    });
    let bottom = bottom.expect("caret geometry");
    assert!(bottom <= px(600.), "caret bottom {bottom:?}");
    assert!(bottom > px(300.));
}

#[gpui_kit::test]
fn code_blocks_take_the_caret_and_newlines(cx: &mut TestAppContext) {
    use remindr_core::domain::CodeLanguage;

    let mut doc = Doc::new();
    let empty = doc.child(
        BlockKind::Code {
            language: CodeLanguage::plain_text(),
        },
        "",
    );
    let filled = doc.child(
        BlockKind::Code {
            language: CodeLanguage::new("rust"),
        },
        "let x = 1;",
    );
    let fixture = Fixture::open(cx, &doc);
    fixture.click_block(cx, empty);
    assert_eq!(fixture.caret(cx), Some(at(empty, 0)));
    fixture.input(cx, "ab");
    fixture.press(cx, "enter");
    fixture.input(cx, "c");
    assert_eq!(fixture.text(cx, empty), "ab\nc");
    fixture.click_block(cx, filled);
    let caret = fixture.caret(cx).expect("caret");
    assert_eq!(caret.block, filled);
    assert!(caret.offset <= "let x = 1;".len());
    fixture.with(cx, |window, cx| {
        let has_leaf = fixture
            .editor
            .read(cx)
            .frame
            .borrow()
            .leaf(filled)
            .is_some();
        assert!(has_leaf);
        window.render_frame(cx);
    });
}
