use gpui_kit::{
    App, Context, DragMoveEvent, Hsla, InteractiveElement, IntoElement, Pixels, Render,
    SharedString, Window, div, prelude::*, px,
};
use remindr_core::application::{Command, DropTarget};
use remindr_core::domain::{BlockId, BlockKind, Parent};

use super::BlockEditor;
use super::render::{GUTTER, INDENT};
use super::rows::position;
use crate::theme::{RADIUS_LG, UI_TEXT};

pub const NEST_THRESHOLD: Pixels = px(24.);
pub const PREVIEW_CHARS: usize = 40;
const EDGE: Pixels = px(32.);
const STEP: Pixels = px(12.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropRow {
    pub block: BlockId,
    pub parent: Parent,
    pub depth: usize,
    pub accepts_children: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Drop {
    pub target: DropTarget,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DropMark {
    pub target: DropTarget,
    pub x: Pixels,
    pub y: Pixels,
    pub right: Pixels,
}

#[derive(Clone)]
pub struct DraggedBlock {
    pub block: BlockId,
}

pub struct DragPreview {
    label: SharedString,
    background: Hsla,
    color: Hsla,
}

impl DragPreview {
    pub fn new(label: SharedString, background: Hsla, color: Hsla) -> Self {
        Self {
            label,
            background,
            color,
        }
    }
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(10.))
            .py(px(5.))
            .rounded(RADIUS_LG)
            .bg(self.background)
            .text_color(self.color)
            .text_size(UI_TEXT)
            .child(self.label.clone())
    }
}

pub fn drop_target_for_position(
    rows: &[DropRow],
    dragged: BlockId,
    slot: usize,
    offset: Pixels,
) -> Option<Drop> {
    if slot > rows.len() {
        return None;
    }
    let candidate = if slot == 0 {
        let first = rows.first()?;
        Drop {
            target: DropTarget {
                parent: first.parent,
                after: None,
            },
            depth: first.depth,
        }
    } else {
        let above = rows.get(slot - 1)?;
        let below = rows.get(slot);
        let opens = below.is_some_and(|below| below.depth > above.depth);
        if opens || (offset > NEST_THRESHOLD && above.accepts_children) {
            Drop {
                target: DropTarget {
                    parent: Parent::Block(above.block),
                    after: None,
                },
                depth: above.depth + 1,
            }
        } else {
            let floor = below.map_or(0, |below| below.depth);
            let out = if offset < -NEST_THRESHOLD {
                ((-offset) / INDENT) as usize
            } else {
                0
            };
            let depth = above.depth.saturating_sub(out).max(floor);
            let anchor = rows[..slot].iter().rev().find(|row| row.depth == depth)?;
            Drop {
                target: DropTarget {
                    parent: anchor.parent,
                    after: Some(anchor.block),
                },
                depth,
            }
        }
    };
    let Some(index) = rows.iter().position(|row| row.block == dragged) else {
        return Some(candidate);
    };
    let depth = rows[index].depth;
    let end = index
        + 1
        + rows[index + 1..]
            .iter()
            .take_while(|row| row.depth > depth)
            .count();
    let inside = match candidate.target.parent {
        Parent::Block(parent) => rows[index..end].iter().any(|row| row.block == parent),
        _ => false,
    };
    if inside || candidate.target.after == Some(dragged) {
        return None;
    }
    let previous = rows[..index]
        .iter()
        .rev()
        .take_while(|row| row.depth >= depth)
        .find(|row| row.depth == depth)
        .map(|row| row.block);
    let current = DropTarget {
        parent: rows[index].parent,
        after: previous,
    };
    (candidate.target != current).then_some(candidate)
}

pub fn slot_for(centers: impl Iterator<Item = Pixels>, y: Pixels) -> usize {
    centers.take_while(|center| *center < y).count()
}

pub fn preview_label(text: &str, fallback: &str) -> String {
    let first_line = text.lines().next().unwrap_or_default().trim();
    if first_line.is_empty() {
        return fallback.to_owned();
    }
    if first_line.chars().count() <= PREVIEW_CHARS {
        return first_line.to_owned();
    }
    let mut label: String = first_line.chars().take(PREVIEW_CHARS - 1).collect();
    label.push('\u{2026}');
    label
}

impl BlockEditor {
    pub(super) fn begin_block_drag(&self, block: BlockId) {
        self.drag_block.set(Some(block));
    }

    pub(super) fn refresh_drop(&self, cx: &App) {
        let Some(dragged) = self.drag_block.get() else {
            return;
        };
        if !cx.has_active_drag() {
            self.drag_block.set(None);
            self.drop.set(None);
            return;
        }
        let mark = self
            .pointer
            .get()
            .and_then(|pointer| self.drop_for(dragged, pointer, cx));
        self.drop.set(mark);
    }

    fn drop_for(
        &self,
        dragged: BlockId,
        pointer: gpui_kit::Point<Pixels>,
        cx: &App,
    ) -> Option<DropMark> {
        if !self.scroll.bounds().contains(&pointer) {
            return None;
        }
        let workspace = self.model.read(cx).workspace();
        let flow = self.visible_rows(cx);
        let frame = self.frame.borrow();
        let mut rows = Vec::new();
        let mut areas = Vec::new();
        for (block, area) in frame.rows() {
            let Some(index) = position(&flow, *block) else {
                continue;
            };
            let Some(found) = workspace.block(*block) else {
                continue;
            };
            rows.push(DropRow {
                block: *block,
                parent: found.parent,
                depth: flow[index].depth,
                accepts_children: found.kind.accepts_children()
                    && !matches!(found.kind, BlockKind::Page { .. }),
            });
            areas.push(*area);
        }
        let first = rows.first()?;
        let over_itself = rows.iter().zip(&areas).any(|(row, area)| {
            row.block == dragged && area.top() < pointer.y && pointer.y < area.bottom()
        });
        if over_itself {
            return None;
        }
        let slot = slot_for(areas.iter().map(|area| area.center().y), pointer.y);
        if slot == 0 && flow.first().map(|row| row.block) != Some(first.block) {
            return None;
        }
        let offset = match slot.checked_sub(1) {
            Some(above) => {
                pointer.x - (areas[above].left() + GUTTER + INDENT * rows[above].depth as f32)
            }
            None => px(0.),
        };
        let found = drop_target_for_position(&rows, dragged, slot, offset)?;
        let y = match slot.checked_sub(1) {
            Some(above) => areas[above].bottom(),
            None => areas[0].top(),
        };
        let left = areas[0].left();
        Some(DropMark {
            target: found.target,
            x: left + GUTTER + INDENT * found.depth as f32,
            y,
            right: areas[0].right(),
        })
    }

    pub(super) fn drop_block(
        &mut self,
        block: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mark = self.drop_for(block, window.mouse_position(), cx);
        self.drop.set(None);
        self.drag_block.set(None);
        self.pointer.set(None);
        let Some(mark) = mark else {
            return;
        };
        let command = Command::MoveBlock {
            block,
            target: mark.target,
        };
        if self.dispatch(command, window, cx).is_some() {
            self.open_parent_toggle(block, cx);
        }
        cx.notify();
    }

    pub(super) fn auto_scroll(&self, window: &mut Window) -> bool {
        let active = self.dragging || self.drag_block.get().is_some();
        let Some(pointer) = self.pointer.get().filter(|_| active) else {
            return false;
        };
        let view = self.scroll.bounds();
        let step = if pointer.y < view.top() + EDGE {
            STEP
        } else if pointer.y > view.bottom() - EDGE {
            -STEP
        } else {
            return false;
        };
        let mut offset = self.scroll.offset();
        let limit = self.scroll.max_offset().y;
        let target = (offset.y + step).clamp(-limit, px(0.));
        if target == offset.y {
            return false;
        }
        offset.y = target;
        self.scroll.set_offset(offset);
        window.refresh();
        true
    }
}

pub fn handle<E: InteractiveElement>(root: E, cx: &mut Context<BlockEditor>) -> E {
    root.on_drag_move::<DraggedBlock>(cx.listener(
        |this, event: &DragMoveEvent<DraggedBlock>, window, cx| {
            this.pointer.set(Some(event.event.position));
            window.refresh();
            cx.notify();
        },
    ))
    .on_drop::<DraggedBlock>(cx.listener(|this, dragged: &DraggedBlock, window, cx| {
        this.drop_block(dragged.block, window, cx);
    }))
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;
    use remindr_core::application::DropTarget;
    use remindr_core::domain::{BlockId, Parent, Uuid};

    use super::{Drop, DropRow, drop_target_for_position, preview_label, slot_for};

    fn id(n: u128) -> BlockId {
        BlockId::from_uuid(Uuid::from_u128(n))
    }

    fn page() -> Parent {
        Parent::Block(id(100))
    }

    fn row(n: u128, parent: Parent, depth: usize, accepts_children: bool) -> DropRow {
        DropRow {
            block: id(n),
            parent,
            depth,
            accepts_children,
        }
    }

    fn rows() -> Vec<DropRow> {
        vec![
            row(1, page(), 0, true),
            row(2, page(), 0, true),
            row(3, Parent::Block(id(2)), 1, true),
            row(4, page(), 0, false),
            row(5, page(), 0, true),
        ]
    }

    fn target(parent: Parent, after: Option<u128>) -> DropTarget {
        DropTarget {
            parent,
            after: after.map(id),
        }
    }

    fn dropped(rows: &[DropRow], dragged: u128, slot: usize, offset: f32) -> Option<Drop> {
        drop_target_for_position(rows, id(dragged), slot, px(offset))
    }

    #[test]
    fn the_first_slot_drops_at_the_top_of_the_page() {
        let found = dropped(&rows(), 5, 0, 0.).expect("drop");
        assert_eq!(found.target, target(page(), None));
        assert_eq!(found.depth, 0);
    }

    #[test]
    fn a_slot_after_a_row_drops_as_its_next_sibling() {
        let found = dropped(&rows(), 5, 1, 0.).expect("drop");
        assert_eq!(found.target, target(page(), Some(1)));
        assert_eq!(found.depth, 0);
        let found = dropped(&rows(), 1, 4, 10.).expect("drop");
        assert_eq!(found.target, target(page(), Some(4)));
    }

    #[test]
    fn dropping_more_than_24px_right_of_the_text_nests_as_first_child() {
        let found = dropped(&rows(), 5, 1, 30.).expect("drop");
        assert_eq!(found.target, target(Parent::Block(id(1)), None));
        assert_eq!(found.depth, 1);
        let at_the_limit = dropped(&rows(), 5, 1, 24.).expect("drop");
        assert_eq!(at_the_limit.target, target(page(), Some(1)));
    }

    #[test]
    fn rows_that_take_no_children_never_nest() {
        let found = dropped(&rows(), 1, 4, 60.).expect("drop");
        assert_eq!(found.target, target(page(), Some(4)));
    }

    #[test]
    fn a_slot_between_a_row_and_its_visible_child_is_that_rows_first_child() {
        let found = dropped(&rows(), 5, 2, 0.).expect("drop");
        assert_eq!(found.target, target(Parent::Block(id(2)), None));
        assert_eq!(found.depth, 1);
        let far_left = dropped(&rows(), 5, 2, -60.).expect("drop");
        assert_eq!(far_left.target, target(Parent::Block(id(2)), None));
    }

    #[test]
    fn the_end_of_a_subtree_can_drop_inside_it_or_back_out() {
        let inside = dropped(&rows(), 5, 3, 0.).expect("drop");
        assert_eq!(inside.target, target(Parent::Block(id(2)), Some(3)));
        assert_eq!(inside.depth, 1);
        let out = dropped(&rows(), 5, 3, -30.).expect("drop");
        assert_eq!(out.target, target(page(), Some(2)));
        assert_eq!(out.depth, 0);
        let nested = dropped(&rows(), 5, 3, 30.).expect("drop");
        assert_eq!(nested.target, target(Parent::Block(id(3)), None));
        assert_eq!(nested.depth, 2);
    }

    #[test]
    fn a_block_never_drops_inside_its_own_subtree() {
        let rows = rows();
        assert_eq!(dropped(&rows, 2, 2, 0.), None);
        assert_eq!(dropped(&rows, 2, 3, 0.), None);
        assert_eq!(dropped(&rows, 2, 3, 30.), None);
        assert_eq!(dropped(&rows, 2, 3, -30.), None);
    }

    #[test]
    fn a_block_never_drops_where_it_already_is() {
        let rows = rows();
        assert_eq!(dropped(&rows, 1, 0, 0.), None);
        assert_eq!(dropped(&rows, 1, 1, 0.), None);
        assert_eq!(dropped(&rows, 2, 1, 0.), None);
        assert_eq!(dropped(&rows, 5, 4, 0.), None);
        assert_eq!(dropped(&rows, 5, 5, 0.), None);
        assert!(dropped(&rows, 2, 4, 0.).is_some());
        assert!(dropped(&rows, 1, 5, 0.).is_some());
    }

    #[test]
    fn a_child_can_leave_its_parent_by_dropping_after_it() {
        let rows = rows();
        let found = dropped(&rows, 3, 3, -30.).expect("drop");
        assert_eq!(found.target, target(page(), Some(2)));
        assert_eq!(dropped(&rows, 3, 3, 0.), None);
    }

    #[test]
    fn an_unknown_slot_is_refused() {
        assert_eq!(dropped(&rows(), 5, 6, 0.), None);
        assert_eq!(dropped(&[], 5, 0, 0.), None);
    }

    #[test]
    fn the_slot_is_the_number_of_rows_whose_center_is_above_the_pointer() {
        let centers = [px(10.), px(30.), px(50.)];
        assert_eq!(slot_for(centers.iter().copied(), px(5.)), 0);
        assert_eq!(slot_for(centers.iter().copied(), px(31.)), 2);
        assert_eq!(slot_for(centers.iter().copied(), px(500.)), 3);
    }

    #[test]
    fn the_preview_label_is_cut_at_forty_characters() {
        assert_eq!(preview_label("short", "Empty"), "short");
        assert_eq!(preview_label("", "Empty"), "Empty");
        assert_eq!(preview_label("  \n", "Empty"), "Empty");
        let long = "x".repeat(60);
        let label = preview_label(&long, "Empty");
        assert_eq!(label.chars().count(), 40);
        assert!(label.ends_with('\u{2026}'));
        let exact = "y".repeat(40);
        assert_eq!(preview_label(&exact, "Empty"), exact);
        assert_eq!(preview_label("first\nsecond", "Empty"), "first");
    }

    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        App, InputEvent, Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
        Pixels, Point, TestAppContext, Window, point,
    };
    use remindr_core::application::TextPoint;
    use remindr_core::domain::{BlockKind, HeadingLevel};

    use crate::editor::render::{GUTTER, handle_id, row_id};
    use crate::editor::testing::{Doc, Fixture};

    fn press_at(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
        window.dispatch_event(
            MouseDownEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }

    fn move_to(window: &mut Window, position: Point<Pixels>, held: bool, cx: &mut App) {
        window.dispatch_event(
            MouseMoveEvent {
                position,
                pressed_button: held.then_some(MouseButton::Left),
                modifiers: Modifiers::default(),
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }

    fn release_at(window: &mut Window, position: Point<Pixels>, cx: &mut App) {
        window.dispatch_event(
            MouseUpEvent {
                button: MouseButton::Left,
                position,
                modifiers: Modifiers::default(),
                click_count: 1,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
    }

    fn hold_drag(window: &mut Window, from: Point<Pixels>, to: Point<Pixels>, cx: &mut App) {
        move_to(window, from, false, cx);
        press_at(window, from, cx);
        for step in 1..=8 {
            let fraction = step as f32 / 8.;
            move_to(
                window,
                point(
                    from.x + (to.x - from.x) * fraction,
                    from.y + (to.y - from.y) * fraction,
                ),
                true,
                cx,
            );
        }
    }

    fn handle_center(window: &Window, block: BlockId) -> Point<Pixels> {
        window.find(handle_id(block)).bounds().center()
    }

    fn text_left(window: &Window, block: BlockId) -> Pixels {
        window.find(row_id(block)).bounds().left() + GUTTER
    }

    fn three(cx: &mut TestAppContext) -> (Fixture, [BlockId; 3]) {
        let mut doc = Doc::new();
        let first = doc.child(BlockKind::Text, "first");
        let second = doc.child(BlockKind::Text, "second");
        let third = doc.child(BlockKind::Text, "third");
        (Fixture::open(cx, &doc), [first, second, third])
    }

    #[gpui_kit::test]
    fn dragging_a_block_below_another_moves_it(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, first);
            let bounds = window.find(row_id(third)).bounds();
            let to = point(bounds.left() + GUTTER + px(10.), bounds.bottom() - px(2.));
            window.drag(from, to, cx);
        });
        assert_eq!(
            fixture.children_ids(cx, fixture.page),
            vec![second, third, first]
        );
        assert_eq!(
            fixture.texts(cx),
            vec!["second".to_owned(), "third".to_owned(), "first".to_owned()]
        );
        assert!(!fixture.has_notification(cx));
    }

    #[gpui_kit::test]
    fn dragging_a_block_between_two_others_drops_it_there(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, third);
            let boundary = window.find(row_id(first)).bounds().bottom();
            let to = point(text_left(window, first) + px(5.), boundary);
            window.drag(from, to, cx);
        });
        assert_eq!(
            fixture.children_ids(cx, fixture.page),
            vec![first, third, second]
        );
    }

    #[gpui_kit::test]
    fn dropping_far_right_of_a_row_nests_the_block_as_its_first_child(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, third);
            let boundary = window.find(row_id(first)).bounds().bottom();
            let to = point(text_left(window, first) + px(40.), boundary);
            window.drag(from, to, cx);
        });
        assert_eq!(fixture.children_ids(cx, fixture.page), vec![first, second]);
        assert_eq!(fixture.children_ids(cx, first), vec![third]);
        assert_eq!(fixture.parent(cx, third), Some(Parent::Block(first)));
    }

    #[gpui_kit::test]
    fn nesting_into_a_closed_toggle_opens_it(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let toggle = doc.child(BlockKind::Toggle, "more");
        let moved = doc.child(BlockKind::Text, "moved");
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, moved);
            let boundary = window.find(row_id(toggle)).bounds().bottom();
            let to = point(text_left(window, toggle) + px(60.), boundary);
            window.drag(from, to, cx);
        });
        assert_eq!(fixture.children_ids(cx, toggle), vec![moved]);
        fixture.with(cx, |window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(row_id(moved)).is_some());
        });
    }

    #[gpui_kit::test]
    fn dragging_onto_itself_does_nothing(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, second);
            let to = window.find(row_id(second)).bounds().center();
            window.drag(from, to, cx);
        });
        assert_eq!(
            fixture.children_ids(cx, fixture.page),
            vec![first, second, third]
        );
        assert!(!fixture.has_notification(cx));
    }

    #[gpui_kit::test]
    fn the_drop_indicator_follows_the_pointer_and_goes_away_with_the_drag(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        let boundary = fixture.with(cx, |window, _| {
            window.find(row_id(second)).bounds().bottom()
        });
        let to = fixture.with(cx, |window, cx| {
            let from = handle_center(window, first);
            let to = point(text_left(window, second) + px(5.), boundary);
            hold_drag(window, from, to, cx);
            to
        });
        let mark = fixture
            .with(cx, |_, cx| fixture.editor.read(cx).drop.get())
            .expect("indicator while dragging");
        assert!((mark.y - boundary).abs() < px(1.));
        assert!((mark.x - to.x).abs() < px(6.));
        assert_eq!(mark.target.after, Some(second));
        fixture.with(cx, |window, cx| release_at(window, to, cx));
        assert_eq!(
            fixture.children_ids(cx, fixture.page),
            vec![second, first, third]
        );
        assert!(fixture.with(cx, |_, cx| fixture.editor.read(cx).drop.get().is_none()));
    }

    #[gpui_kit::test]
    fn the_indicator_shows_the_nesting_depth_by_its_start(cx: &mut TestAppContext) {
        let (fixture, [first, second, third]) = three(cx);
        let boundary = fixture.with(cx, |window, _| window.find(row_id(first)).bounds().bottom());
        let marks = fixture.with(cx, |window, cx| {
            let from = handle_center(window, third);
            let base = text_left(window, first);
            hold_drag(window, from, point(base + px(5.), boundary), cx);
            let flat = fixture.editor.read(cx).drop.get();
            move_to(window, point(base + px(40.), boundary), true, cx);
            let nested = fixture.editor.read(cx).drop.get();
            release_at(window, point(base + px(40.), boundary), cx);
            (flat, nested)
        });
        let (flat, nested) = marks;
        let (flat, nested) = (flat.expect("flat"), nested.expect("nested"));
        assert!(nested.x > flat.x + px(20.));
        assert_eq!(nested.target.parent, Parent::Block(first));
        assert_eq!(flat.target.after, Some(first));
        assert_eq!(fixture.children_ids(cx, fixture.page), vec![first, second]);
        assert_eq!(fixture.children_ids(cx, first), vec![third]);
    }

    #[gpui_kit::test]
    fn no_indicator_over_the_blocks_own_subtree(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let parent = doc.child(BlockKind::Text, "parent");
        let child = doc.under(parent, BlockKind::Text, "child");
        doc.child(BlockKind::Text, "after");
        let fixture = Fixture::open(cx, &doc);
        let to = fixture.with(cx, |window, cx| {
            let from = handle_center(window, parent);
            let to = window.find(row_id(child)).bounds().center();
            hold_drag(window, from, to, cx);
            to
        });
        assert!(fixture.with(cx, |_, cx| fixture.editor.read(cx).drop.get().is_none()));
        fixture.with(cx, |window, cx| release_at(window, to, cx));
        assert_eq!(fixture.parent(cx, child), Some(Parent::Block(parent)));
        assert!(!fixture.has_notification(cx));
    }

    #[gpui_kit::test]
    fn a_dragged_block_keeps_its_subtree(cx: &mut TestAppContext) {
        let mut doc = Doc::new();
        let parent = doc.child(BlockKind::Text, "parent");
        let child = doc.under(parent, BlockKind::Text, "child");
        let other = doc.child(BlockKind::Text, "other");
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |window, cx| {
            let from = handle_center(window, parent);
            let bounds = window.find(row_id(other)).bounds();
            window.drag(
                from,
                point(bounds.left() + GUTTER + px(5.), bounds.bottom() - px(2.)),
                cx,
            );
        });
        assert_eq!(fixture.children_ids(cx, fixture.page), vec![other, parent]);
        assert_eq!(fixture.children_ids(cx, parent), vec![child]);
    }

    #[gpui_kit::test]
    fn every_block_kind_has_a_handle(cx: &mut TestAppContext) {
        use remindr_core::domain::CodeLanguage;

        let mut doc = Doc::new();
        let blocks = [
            doc.child(BlockKind::Text, "t"),
            doc.child(BlockKind::Heading(HeadingLevel::One), "h"),
            doc.child(BlockKind::BulletedListItem, "b"),
            doc.child(BlockKind::ToDo { checked: false }, "d"),
            doc.child(BlockKind::Toggle, "g"),
            doc.child(BlockKind::Quote, "q"),
            doc.child(
                BlockKind::Code {
                    language: CodeLanguage::plain_text(),
                },
                "c",
            ),
            doc.child(BlockKind::Divider, ""),
            doc.child(BlockKind::Page { icon: None }, "p"),
        ];
        let fixture = Fixture::open(cx, &doc);
        fixture.with(cx, |window, _| {
            for block in blocks {
                assert!(window.try_find(handle_id(block)).is_some(), "{block:?}");
            }
        });
        let database = fixture.create_database(cx, "Data");
        let view = fixture.with(cx, |_, cx| {
            fixture
                .model
                .update(cx, |model, cx| {
                    model.execute(
                        remindr_core::application::Command::InsertDatabaseView {
                            after: blocks[0],
                            database,
                        },
                        cx,
                    )
                })
                .expect("view")
                .created
        });
        let view = view
            .iter()
            .find_map(|created| match created {
                remindr_core::application::Created::Block(block) => Some(*block),
                _ => None,
            })
            .expect("block");
        fixture.with(cx, |window, cx| {
            window.render_frame(cx);
            assert!(window.try_find(handle_id(view)).is_some());
        });
    }

    #[gpui_kit::test]
    fn the_handle_shows_while_the_row_is_hovered(cx: &mut TestAppContext) {
        let (fixture, [first, second, _]) = three(cx);
        fixture.with(cx, |window, cx| {
            window.render_frame(cx);
            assert!(!window.find(handle_id(first)).visible());
            window.hover(row_id(first), cx);
            assert!(window.find(handle_id(first)).visible());
            assert!(!window.find(handle_id(second)).visible());
        });
    }

    #[gpui_kit::test]
    fn clicking_the_gutter_beside_the_handle_still_places_the_caret(cx: &mut TestAppContext) {
        let (fixture, [first, second, _]) = three(cx);
        fixture.click_block(cx, first);
        fixture.with(cx, |window, cx| {
            window.click_at(row_id(second), point(px(1.), px(10.)), cx);
        });
        assert_eq!(fixture.caret(cx).map(|caret| caret.block), Some(second));
    }

    #[gpui_kit::test]
    fn pressing_the_handle_does_not_move_the_caret(cx: &mut TestAppContext) {
        let (fixture, [first, second, _]) = three(cx);
        fixture.focus_at(
            cx,
            TextPoint {
                block: second,
                offset: 2,
            },
        );
        fixture.with(cx, |window, cx| {
            let at = handle_center(window, first);
            move_to(window, at, false, cx);
            press_at(window, at, cx);
            release_at(window, at, cx);
        });
        assert_eq!(
            fixture.caret(cx),
            Some(TextPoint {
                block: second,
                offset: 2
            })
        );
    }

    fn lines(cx: &mut TestAppContext, count: usize) -> (Fixture, Vec<BlockId>) {
        let mut doc = Doc::new();
        let blocks = (0..count)
            .map(|number| doc.child(BlockKind::Text, &format!("line {number}")))
            .collect();
        (Fixture::open(cx, &doc), blocks)
    }

    fn scroll_y(fixture: &Fixture, cx: &mut TestAppContext) -> Pixels {
        fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.offset().y)
    }

    #[gpui_kit::test]
    fn selecting_near_the_bottom_edge_scrolls_down(cx: &mut TestAppContext) {
        let (fixture, blocks) = lines(cx, 80);
        let start = fixture.with(cx, |window, _| {
            let bounds = window.find(row_id(blocks[0])).bounds();
            point(bounds.left() + GUTTER + px(20.), bounds.center().y)
        });
        let view = fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.bounds());
        let near_bottom = point(start.x, view.bottom() - px(10.));
        fixture.with(cx, |window, cx| {
            move_to(window, start, false, cx);
            press_at(window, start, cx);
            move_to(window, near_bottom, true, cx);
            for _ in 0..4 {
                window.render_frame(cx);
            }
        });
        let scrolled = scroll_y(&fixture, cx);
        assert!(scrolled <= px(-48.), "scrolled {scrolled:?}");
        let selection = fixture.selection(cx).expect("selection");
        assert_ne!(selection.head.block, blocks[0]);
        fixture.with(cx, |window, cx| release_at(window, near_bottom, cx));
        let stopped = scroll_y(&fixture, cx);
        fixture.with(cx, |window, cx| {
            for _ in 0..3 {
                window.render_frame(cx);
            }
        });
        assert_eq!(scroll_y(&fixture, cx), stopped);
    }

    #[gpui_kit::test]
    fn selecting_in_the_middle_does_not_scroll(cx: &mut TestAppContext) {
        let (fixture, blocks) = lines(cx, 80);
        let start = fixture.with(cx, |window, _| {
            let bounds = window.find(row_id(blocks[0])).bounds();
            point(bounds.left() + GUTTER + px(20.), bounds.center().y)
        });
        let view = fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.bounds());
        let middle = point(start.x, view.center().y);
        fixture.with(cx, |window, cx| {
            move_to(window, start, false, cx);
            press_at(window, start, cx);
            move_to(window, middle, true, cx);
            for _ in 0..4 {
                window.render_frame(cx);
            }
            release_at(window, middle, cx);
        });
        assert_eq!(scroll_y(&fixture, cx), px(0.));
    }

    #[gpui_kit::test]
    fn dragging_a_block_near_the_top_edge_scrolls_up(cx: &mut TestAppContext) {
        let (fixture, blocks) = lines(cx, 80);
        fixture.with(cx, |window, cx| {
            fixture
                .editor
                .read(cx)
                .scroll
                .set_offset(point(px(0.), px(-300.)));
            window.render_frame(cx);
            window.render_frame(cx);
        });
        let from = fixture.with(cx, |window, _| {
            let visible = blocks
                .iter()
                .find(|block| {
                    window
                        .try_find(handle_id(**block))
                        .is_some_and(|found| found.bounds().top() > px(100.))
                })
                .copied()
                .expect("a visible row");
            handle_center(window, visible)
        });
        let view = fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.bounds());
        let near_top = point(from.x + px(60.), view.top() + px(8.));
        fixture.with(cx, |window, cx| {
            hold_drag(window, from, near_top, cx);
            for _ in 0..4 {
                window.render_frame(cx);
            }
        });
        let scrolled = scroll_y(&fixture, cx);
        assert!(scrolled >= px(-252.), "scrolled {scrolled:?}");
        fixture.with(cx, |window, cx| release_at(window, near_top, cx));
    }

    #[gpui_kit::test]
    fn dragging_a_block_near_the_bottom_edge_scrolls_down(cx: &mut TestAppContext) {
        let (fixture, blocks) = lines(cx, 80);
        let from = fixture.with(cx, |window, _| handle_center(window, blocks[0]));
        let view = fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.bounds());
        let near_bottom = point(from.x + px(60.), view.bottom() - px(8.));
        fixture.with(cx, |window, cx| {
            hold_drag(window, from, near_bottom, cx);
            for _ in 0..4 {
                window.render_frame(cx);
            }
        });
        assert!(scroll_y(&fixture, cx) <= px(-48.));
        fixture.with(cx, |window, cx| release_at(window, near_bottom, cx));
    }
}
