use std::collections::{HashMap, HashSet};

use gpui_kit::{Bounds, Pixels, px};
use remindr_core::domain::{BlockId, BlockKind};

pub const TEXT_ESTIMATE: Pixels = px(26.);
pub const HEADING_ESTIMATE: Pixels = px(32.);
pub const DATABASE_ESTIMATE: Pixels = px(120.);
pub const SETTLED: Pixels = px(0.5);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Text,
    Heading,
    Database,
}

impl Class {
    pub fn of(kind: &BlockKind) -> Self {
        match kind {
            BlockKind::Heading(_) => Self::Heading,
            BlockKind::DatabaseView { .. } => Self::Database,
            _ => Self::Text,
        }
    }

    fn slot(self) -> usize {
        match self {
            Self::Text => 0,
            Self::Heading => 1,
            Self::Database => 2,
        }
    }

    fn first_frame(self) -> Pixels {
        match self {
            Self::Text => TEXT_ESTIMATE,
            Self::Heading => HEADING_ESTIMATE,
            Self::Database => DATABASE_ESTIMATE,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub block: BlockId,
    pub class: Class,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Slot {
    Row(usize),
    Held { index: usize, height: Pixels },
    Gap { height: Pixels, rows: usize },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub index: usize,
    pub top: Pixels,
}

#[derive(Debug)]
pub struct Plan {
    pub slots: Vec<Slot>,
    pub anchor: Option<Anchor>,
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub top: Pixels,
    pub height: Pixels,
}

#[derive(Default)]
pub struct Heights {
    measured: HashMap<BlockId, Pixels>,
    typical: [Option<Pixels>; 3],
}

impl Heights {
    pub fn estimate(&self, class: Class) -> Pixels {
        self.typical[class.slot()].unwrap_or_else(|| class.first_frame())
    }

    pub fn of(&self, entry: &Entry) -> Pixels {
        self.measured
            .get(&entry.block)
            .copied()
            .unwrap_or_else(|| self.estimate(entry.class))
    }

    pub fn observe(
        &mut self,
        rows: &[(BlockId, Bounds<Pixels>)],
        class_of: impl Fn(BlockId) -> Option<Class>,
    ) -> bool {
        let mut changed = false;
        for (block, bounds) in rows {
            let height = bounds.size.height;
            let class = class_of(*block);
            let before = self
                .measured
                .get(block)
                .copied()
                .or_else(|| class.map(|class| self.estimate(class)));
            if before.is_none_or(|before| (before - height).abs() > SETTLED) {
                changed = true;
            }
            self.measured.insert(*block, height);
            if let Some(class) = class {
                let slot = &mut self.typical[class.slot()];
                if slot.is_none_or(|typical| height < typical) {
                    *slot = Some(height);
                }
            }
        }
        changed
    }

    pub fn forget_except(&mut self, alive: &HashSet<BlockId>) {
        if self.measured.len() > alive.len() * 2 {
            self.measured.retain(|block, _| alive.contains(block));
        }
    }
}

pub fn plan(
    entries: &[Entry],
    heights: &Heights,
    lead: Pixels,
    viewport: Viewport,
    pinned: &[BlockId],
) -> Plan {
    let from = viewport.top - viewport.height;
    let to = viewport.top + viewport.height * 2.;
    let mut slots = Vec::new();
    let mut anchor = None;
    let mut gap = (px(0.), 0usize);
    let mut y = lead;
    for (index, entry) in entries.iter().enumerate() {
        let height = heights.of(entry);
        let bottom = y + height;
        if anchor.is_none() && bottom > viewport.top {
            anchor = Some(Anchor { index, top: y });
        }
        let keep = (bottom >= from && y <= to) || pinned.contains(&entry.block);
        if keep {
            if gap.1 > 0 {
                slots.push(Slot::Gap {
                    height: gap.0,
                    rows: gap.1,
                });
                gap = (px(0.), 0);
            }
            slots.push(if bottom <= viewport.top {
                Slot::Held { index, height }
            } else {
                Slot::Row(index)
            });
        } else {
            gap.0 += height;
            gap.1 += 1;
        }
        y = bottom;
    }
    if gap.1 > 0 {
        slots.push(Slot::Gap {
            height: gap.0,
            rows: gap.1,
        });
    }
    Plan { slots, anchor }
}

pub fn top_of(
    entries: &[Entry],
    heights: &Heights,
    lead: Pixels,
    block: BlockId,
) -> Option<(usize, Pixels)> {
    let mut y = lead;
    for (index, entry) in entries.iter().enumerate() {
        if entry.block == block {
            return Some((index, y));
        }
        y += heights.of(entry);
    }
    None
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, point, px, size};
    use remindr_core::domain::{BlockId, BlockKind, HeadingLevel, Uuid};

    use super::{
        Class, Entry, HEADING_ESTIMATE, Heights, Slot, TEXT_ESTIMATE, Viewport, plan, top_of,
    };

    fn id(n: u128) -> BlockId {
        BlockId::from_uuid(Uuid::from_u128(n))
    }

    fn entries(count: u128) -> Vec<Entry> {
        (1..=count)
            .map(|n| Entry {
                block: id(n),
                class: Class::Text,
            })
            .collect()
    }

    fn view(top: f32, height: f32) -> Viewport {
        Viewport {
            top: px(top),
            height: px(height),
        }
    }

    fn rows_of(slots: &[Slot]) -> Vec<usize> {
        slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Row(index) | Slot::Held { index, .. } => Some(*index),
                Slot::Gap { .. } => None,
            })
            .collect()
    }

    #[test]
    fn first_frame_estimates_depend_on_the_block_kind() {
        let heights = Heights::default();
        assert_eq!(heights.estimate(Class::of(&BlockKind::Text)), TEXT_ESTIMATE);
        assert_eq!(
            heights.estimate(Class::of(&BlockKind::Heading(HeadingLevel::One))),
            HEADING_ESTIMATE
        );
        assert_eq!(
            heights.estimate(Class::of(&BlockKind::Quote)),
            TEXT_ESTIMATE
        );
    }

    #[test]
    fn measured_rows_calibrate_the_estimate_of_their_class() {
        let mut heights = Heights::default();
        let one_line = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(30.)));
        let wrapped = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(90.)));
        let changed = heights.observe(&[(id(1), wrapped), (id(2), one_line)], |_| {
            Some(Class::Text)
        });
        assert!(changed);
        assert_eq!(heights.estimate(Class::Text), px(30.));
        assert_eq!(heights.estimate(Class::Heading), HEADING_ESTIMATE);
        let settled = heights.observe(&[(id(1), wrapped), (id(2), one_line)], |_| {
            Some(Class::Text)
        });
        assert!(!settled);
    }

    #[test]
    fn a_short_page_is_never_culled() {
        let slots = plan(
            &entries(10),
            &Heights::default(),
            px(8.),
            view(0., 600.),
            &[],
        )
        .slots;
        assert_eq!(rows_of(&slots), (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn rows_beyond_one_viewport_on_each_side_become_one_gap_each() {
        let slots = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(0., 600.),
            &[],
        )
        .slots;
        let rows = rows_of(&slots);
        assert_eq!(rows.first(), Some(&0));
        let last = *rows.last().expect("rows");
        assert!((42..50).contains(&last), "last rendered row {last}");
        assert_eq!(slots.len(), rows.len() + 1);
        let Some(Slot::Gap {
            height,
            rows: culled,
        }) = slots.last().copied()
        else {
            panic!("trailing gap");
        };
        assert_eq!(culled, 300 - rows.len());
        assert_eq!(height, TEXT_ESTIMATE * culled as f32);
    }

    #[test]
    fn scrolled_views_keep_the_band_around_them() {
        let slots = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(3000., 600.),
            &[],
        )
        .slots;
        let rows = rows_of(&slots);
        let row = |offset: f32| (offset / TEXT_ESTIMATE.as_f32()) as usize;
        let first = *rows.first().expect("rows");
        let last = *rows.last().expect("rows");
        assert!(
            first.abs_diff(row(2400.)) < 10,
            "first rendered row {first}"
        );
        assert!(last.abs_diff(row(4200.)) < 10, "last rendered row {last}");
        assert!(matches!(slots.first(), Some(Slot::Gap { .. })));
        assert!(matches!(slots.last(), Some(Slot::Gap { .. })));
        let kept = rows.len();
        assert!(rows.windows(2).all(|pair| pair[1] == pair[0] + 1));
        assert!(kept < 120);
    }

    #[test]
    fn measured_heights_replace_estimates() {
        let mut heights = Heights::default();
        let tall = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(300.)));
        heights.observe(&[(id(1), tall), (id(2), tall)], |_| None);
        let slots = plan(&entries(300), &heights, px(8.), view(0., 600.), &[]).slots;
        let rows = rows_of(&slots);
        assert!(rows.last().is_some_and(|last| *last < 40));
        let total: f32 = slots
            .iter()
            .map(|slot| match slot {
                Slot::Gap { height, .. } | Slot::Held { height, .. } => height.as_f32(),
                Slot::Row(index) => heights.of(&entries(300)[*index]).as_f32(),
            })
            .sum();
        assert_eq!(total, 600. + 298. * TEXT_ESTIMATE.as_f32());
    }

    #[test]
    fn pinned_rows_are_always_kept_and_split_the_gap() {
        let slots = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(0., 600.),
            &[id(251), id(1)],
        )
        .slots;
        let rows = rows_of(&slots);
        assert!(rows.contains(&250));
        let position = slots
            .iter()
            .position(|slot| *slot == Slot::Row(250))
            .expect("pinned");
        assert!(matches!(slots[position - 1], Slot::Gap { .. }));
        assert!(matches!(slots[position + 1], Slot::Gap { .. }));
    }

    #[test]
    fn the_total_height_is_the_same_with_and_without_culling() {
        let heights = Heights::default();
        let all = plan(&entries(300), &heights, px(8.), view(0., 100_000.), &[]).slots;
        assert_eq!(rows_of(&all).len(), 300);
        let culled = plan(&entries(300), &heights, px(8.), view(0., 600.), &[]).slots;
        let sum = |slots: &[Slot]| -> f32 {
            slots
                .iter()
                .map(|slot| match slot {
                    Slot::Gap { height, .. } | Slot::Held { height, .. } => height.as_f32(),
                    Slot::Row(_) => TEXT_ESTIMATE.as_f32(),
                })
                .sum()
        };
        assert_eq!(sum(&all), sum(&culled));
    }

    #[test]
    fn rows_entirely_above_the_viewport_are_held_at_their_model_height() {
        let plan = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(3000., 600.),
            &[id(1)],
        );
        let held: Vec<_> = plan
            .slots
            .iter()
            .filter(|slot| matches!(slot, Slot::Held { .. }))
            .collect();
        assert!(held.len() > 15);
        assert!(plan.slots.iter().any(|slot| matches!(
            slot,
            Slot::Held { index: 0, height } if *height == TEXT_ESTIMATE
        )));
        let rows: Vec<_> = plan
            .slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Row(index) => Some(*index),
                _ => None,
            })
            .collect();
        let anchor = plan.anchor.expect("an anchor");
        assert_eq!(rows.first(), Some(&anchor.index));
        let top = 8. + anchor.index as f32 * TEXT_ESTIMATE.as_f32();
        assert_eq!(anchor.top, px(top));
        assert!(anchor.top <= px(3000.));
        assert!(anchor.top + TEXT_ESTIMATE > px(3000.));
    }

    #[test]
    fn the_anchor_is_the_first_row_that_ends_below_the_top_edge() {
        let at_top = plan(
            &entries(10),
            &Heights::default(),
            px(8.),
            view(0., 600.),
            &[],
        );
        assert_eq!(
            at_top.anchor.map(|anchor| (anchor.index, anchor.top)),
            Some((0, px(8.)))
        );
        let none = plan(
            &entries(3),
            &Heights::default(),
            px(8.),
            view(5000., 600.),
            &[],
        );
        assert_eq!(none.anchor, None);
    }

    #[test]
    fn top_of_adds_up_the_heights_of_the_rows_before_the_block() {
        let mut heights = Heights::default();
        let tall = Bounds::new(point(px(0.), px(0.)), size(px(100.), px(60.)));
        heights.observe(&[(id(2), tall)], |_| None);
        let all = entries(5);
        assert_eq!(top_of(&all, &heights, px(8.), id(1)), Some((0, px(8.))));
        assert_eq!(
            top_of(&all, &heights, px(8.), id(4)),
            Some((3, px(8.) + TEXT_ESTIMATE + px(60.) + TEXT_ESTIMATE))
        );
        assert_eq!(top_of(&all, &heights, px(8.), id(99)), None);
    }

    #[test]
    fn forgetting_keeps_the_heights_of_living_blocks() {
        use std::collections::HashSet;

        let mut heights = Heights::default();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(10.), px(33.)));
        let rows: Vec<_> = (1..=10).map(|n| (id(n), bounds)).collect();
        heights.observe(&rows, |_| None);
        heights.forget_except(&HashSet::from([id(1)]));
        let entry = |n| Entry {
            block: id(n),
            class: Class::Heading,
        };
        assert_eq!(heights.of(&entry(1)), px(33.));
        assert_eq!(heights.of(&entry(5)), HEADING_ESTIMATE);
    }

    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{Pixels, ScrollDelta, TestAppContext};
    use remindr_core::application::TextPoint;

    use crate::editor::render::row_id;
    use crate::editor::testing::{Doc, Fixture};

    fn many(cx: &mut TestAppContext, count: usize) -> (Fixture, Vec<BlockId>) {
        let mut doc = Doc::new();
        let blocks = (0..count)
            .map(|number| doc.child(BlockKind::Text, &format!("line {number}")))
            .collect();
        (Fixture::open(cx, &doc), blocks)
    }

    fn rendered(fixture: &Fixture, cx: &mut TestAppContext, blocks: &[BlockId]) -> Vec<usize> {
        fixture.with(cx, |window, cx| {
            window.render_frame(cx);
            blocks
                .iter()
                .enumerate()
                .filter(|(_, block)| window.try_find(row_id(**block)).is_some())
                .map(|(index, _)| index)
                .collect()
        })
    }

    fn settle(fixture: &Fixture, cx: &mut TestAppContext) {
        fixture.with(cx, |window, cx| {
            for _ in 0..4 {
                window.render_frame(cx);
            }
        });
    }

    fn scroll_to(fixture: &Fixture, cx: &mut TestAppContext, y: f32) {
        fixture.with(cx, |_, cx| {
            fixture
                .editor
                .read(cx)
                .scroll
                .set_offset(point(px(0.), px(-y)));
        });
        settle(fixture, cx);
    }

    #[gpui_kit::test]
    fn rows_outside_the_viewport_are_spacers(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        let shown = rendered(&fixture, cx, &blocks);
        assert!(shown.len() < 120, "{} rows rendered", shown.len());
        assert!(shown.len() > 15);
        assert_eq!(shown.first(), Some(&0));
        assert!(!shown.contains(&250));
        assert!(!shown.contains(&299));
        let texts = fixture.with(cx, |window, _| {
            window.try_find(row_id(blocks[250])).is_none()
        });
        assert!(texts);
    }

    #[gpui_kit::test]
    fn the_scroll_extent_matches_the_unculled_page(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        settle(&fixture, cx);
        let row = fixture.with(cx, |window, _| {
            window.find(row_id(blocks[0])).bounds().size.height
        });
        let extent = fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.max_offset().y);
        let expected = row * 300. + px(8. + 160.) - px(600.);
        assert!(
            (extent - expected).abs() < row * 12.,
            "extent {extent:?} expected {expected:?}"
        );
    }

    #[gpui_kit::test]
    fn caret_row_is_never_culled(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        fixture.focus_at(
            cx,
            TextPoint {
                block: blocks[250],
                offset: 2,
            },
        );
        settle(&fixture, cx);
        scroll_to(&fixture, cx, 0.);
        let shown = rendered(&fixture, cx, &blocks);
        assert!(shown.contains(&250), "caret row rendered at the top");
        assert!(!shown.contains(&249));
        assert!(!shown.contains(&251));
        assert!(shown.contains(&0));
        fixture.input(cx, "x");
        assert_eq!(fixture.text(cx, blocks[250]), "lixne 250");
    }

    #[gpui_kit::test]
    fn selection_ends_are_never_culled(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        fixture.select(
            cx,
            TextPoint {
                block: blocks[0],
                offset: 1,
            },
            TextPoint {
                block: blocks[250],
                offset: 3,
            },
        );
        settle(&fixture, cx);
        scroll_to(&fixture, cx, 0.);
        let shown = rendered(&fixture, cx, &blocks);
        assert!(shown.contains(&0));
        assert!(shown.contains(&250));
        assert!(!shown.contains(&125));
    }

    #[gpui_kit::test]
    fn scrolling_renders_newly_visible_rows(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        assert!(!rendered(&fixture, cx, &blocks).contains(&120));
        fixture.with(cx, |window, cx| {
            window.scroll(
                "block-editor",
                ScrollDelta::Pixels(point(px(0.), px(-3000.))),
                cx,
            );
        });
        settle(&fixture, cx);
        let shown = rendered(&fixture, cx, &blocks);
        assert!(shown.contains(&110), "{shown:?}");
        assert!(shown.contains(&120));
        assert!(!shown.contains(&0));
        assert!(!shown.contains(&299));
        let visible = fixture.with(cx, |window, _| {
            let top = window.find(row_id(blocks[110])).bounds().top();
            top > px(-1500.) && top < px(2500.)
        });
        assert!(visible);
    }

    #[gpui_kit::test]
    fn scrolling_back_up_renders_the_first_rows_again(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        scroll_to(&fixture, cx, 4000.);
        assert!(!rendered(&fixture, cx, &blocks).contains(&0));
        scroll_to(&fixture, cx, 0.);
        let shown = rendered(&fixture, cx, &blocks);
        assert!(shown.contains(&0));
        assert!(!shown.contains(&200));
    }

    #[gpui_kit::test]
    fn clicking_a_row_after_scrolling_places_the_caret_in_it(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        let row: f32 = fixture
            .with(cx, |window, _| {
                window.find(row_id(blocks[0])).bounds().size.height
            })
            .into();
        scroll_to(&fixture, cx, row * 100.);
        fixture.click_block(cx, blocks[112]);
        let caret = fixture.caret(cx).expect("caret");
        assert_eq!(caret.block, blocks[112]);
    }

    #[gpui_kit::test]
    fn arrow_down_into_a_row_that_was_culled_lands_on_it(cx: &mut TestAppContext) {
        let (fixture, blocks) = many(cx, 300);
        fixture.focus_at(
            cx,
            TextPoint {
                block: blocks[60],
                offset: 0,
            },
        );
        settle(&fixture, cx);
        for _ in 0..3 {
            fixture.press(cx, "down");
        }
        assert_eq!(fixture.caret(cx).map(|caret| caret.block), Some(blocks[63]));
    }

    fn wrapping_page(cx: &mut TestAppContext, count: usize) -> (Fixture, Vec<BlockId>) {
        let mut doc = Doc::new();
        let long = "word ".repeat(40);
        let blocks = (0..count)
            .map(|number| {
                let text = if number % 5 == 4 {
                    format!("{number} {long}")
                } else {
                    format!("line {number}")
                };
                doc.child(BlockKind::Text, &text)
            })
            .collect();
        (Fixture::open(cx, &doc), blocks)
    }

    fn settle_for(fixture: &Fixture, cx: &mut TestAppContext, frames: usize) {
        fixture.with(cx, |window, cx| {
            for _ in 0..frames {
                window.render_frame(cx);
            }
        });
    }

    fn view_top(fixture: &Fixture, cx: &mut TestAppContext) -> Pixels {
        fixture.with(cx, |_, cx| fixture.editor.read(cx).scroll.bounds().top())
    }

    fn first_visible(
        fixture: &Fixture,
        cx: &mut TestAppContext,
        blocks: &[BlockId],
        below: Pixels,
    ) -> usize {
        let top = view_top(fixture, cx);
        fixture.with(cx, |window, _| {
            blocks
                .iter()
                .position(|block| {
                    window
                        .try_find(row_id(*block))
                        .is_some_and(|found| found.bounds().top() >= top + below)
                })
                .expect("a visible row")
        })
    }

    fn row_top(fixture: &Fixture, cx: &mut TestAppContext, block: BlockId) -> Pixels {
        fixture.with(cx, |window, _| window.find(row_id(block)).bounds().top())
    }

    #[gpui_kit::test]
    fn wrapped_rows_are_taller_than_single_lines(cx: &mut TestAppContext) {
        let (fixture, blocks) = wrapping_page(cx, 10);
        let single = row_top(&fixture, cx, blocks[1]) - row_top(&fixture, cx, blocks[0]);
        let wrapped = row_top(&fixture, cx, blocks[5]) - row_top(&fixture, cx, blocks[4]);
        assert!(wrapped > single * 1.5, "{wrapped:?} against {single:?}");
    }

    #[gpui_kit::test]
    fn scrolling_up_into_unmeasured_rows_does_not_move_the_visible_content(
        cx: &mut TestAppContext,
    ) {
        let (fixture, blocks) = wrapping_page(cx, 300);
        scroll_to(&fixture, cx, 6000.);
        settle_for(&fixture, cx, 6);
        let tracked = first_visible(&fixture, cx, &blocks, px(40.));
        let mut previous = row_top(&fixture, cx, blocks[tracked]);
        for step in 0..45 {
            fixture.with(cx, |window, cx| {
                let editor = fixture.editor.read(cx);
                let mut offset = editor.scroll.offset();
                offset.y += px(20.);
                editor.scroll.set_offset(offset);
                window.render_frame(cx);
            });
            let now = row_top(&fixture, cx, blocks[tracked]);
            assert!(
                (now - previous - px(20.)).abs() < px(1.),
                "step {step}: the tracked row moved {:?} instead of 20px",
                now - previous
            );
            previous = now;
        }
    }

    #[gpui_kit::test]
    fn resizing_the_window_does_not_move_the_first_visible_row(cx: &mut TestAppContext) {
        let (fixture, blocks) = wrapping_page(cx, 300);
        scroll_to(&fixture, cx, 6000.);
        settle_for(&fixture, cx, 6);
        let top = view_top(&fixture, cx);
        let anchor = fixture.with(cx, |window, _| {
            blocks
                .iter()
                .position(|block| {
                    window
                        .try_find(row_id(*block))
                        .is_some_and(|found| found.bounds().bottom() > top)
                })
                .expect("the row under the top edge")
        });
        let before = row_top(&fixture, cx, blocks[anchor]);
        cx.simulate_window_resize(fixture.handle, size(px(480.), px(600.)));
        for frame in 0..8 {
            settle_for(&fixture, cx, 1);
            let now = row_top(&fixture, cx, blocks[anchor]);
            assert!(
                (now - before).abs() < px(1.),
                "frame {frame}: the anchor row moved {:?}",
                now - before
            );
        }
    }

    #[gpui_kit::test]
    fn editing_inside_the_view_does_not_scroll_it(cx: &mut TestAppContext) {
        let (fixture, blocks) = wrapping_page(cx, 300);
        scroll_to(&fixture, cx, 6000.);
        settle_for(&fixture, cx, 6);
        let tracked = first_visible(&fixture, cx, &blocks, px(100.));
        fixture.focus_at(
            cx,
            TextPoint {
                block: blocks[tracked],
                offset: 2,
            },
        );
        settle_for(&fixture, cx, 4);
        let before = row_top(&fixture, cx, blocks[tracked]);
        fixture.input(cx, "xyz");
        settle_for(&fixture, cx, 2);
        assert_eq!(row_top(&fixture, cx, blocks[tracked]), before);
    }
}
