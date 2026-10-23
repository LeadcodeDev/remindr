use std::collections::{HashMap, HashSet};

use gpui_kit::{Bounds, Pixels, px};
use remindr_core::domain::{BlockId, BlockKind};

pub const TEXT_ESTIMATE: Pixels = px(28.);
pub const HEADING_ESTIMATE: Pixels = px(40.);
pub const DATABASE_ESTIMATE: Pixels = px(120.);
const SETTLED: Pixels = px(0.5);

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
    Gap { height: Pixels, rows: usize },
}

#[derive(Clone, Copy, Debug)]
pub struct View {
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
    view: View,
    pinned: &[BlockId],
) -> Vec<Slot> {
    let from = view.top - view.height;
    let to = view.top + view.height * 2.;
    let mut slots = Vec::new();
    let mut gap = (px(0.), 0usize);
    let mut y = lead;
    for (index, entry) in entries.iter().enumerate() {
        let height = heights.of(entry);
        let bottom = y + height;
        let keep = (bottom >= from && y <= to) || pinned.contains(&entry.block);
        if keep {
            if gap.1 > 0 {
                slots.push(Slot::Gap {
                    height: gap.0,
                    rows: gap.1,
                });
                gap = (px(0.), 0);
            }
            slots.push(Slot::Row(index));
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
    slots
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Bounds, point, px, size};
    use remindr_core::domain::{BlockId, BlockKind, HeadingLevel, Uuid};

    use super::{Class, Entry, Heights, Slot, View, plan};

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

    fn view(top: f32, height: f32) -> View {
        View {
            top: px(top),
            height: px(height),
        }
    }

    fn rows_of(slots: &[Slot]) -> Vec<usize> {
        slots
            .iter()
            .filter_map(|slot| match slot {
                Slot::Row(index) => Some(*index),
                Slot::Gap { .. } => None,
            })
            .collect()
    }

    #[test]
    fn first_frame_estimates_depend_on_the_block_kind() {
        let heights = Heights::default();
        assert_eq!(heights.estimate(Class::of(&BlockKind::Text)), px(28.));
        assert_eq!(
            heights.estimate(Class::of(&BlockKind::Heading(HeadingLevel::One))),
            px(40.)
        );
        assert_eq!(heights.estimate(Class::of(&BlockKind::Quote)), px(28.));
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
        assert_eq!(heights.estimate(Class::Heading), px(40.));
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
        );
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
        );
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
        assert_eq!(height, px(28. * culled as f32));
    }

    #[test]
    fn scrolled_views_keep_the_band_around_them() {
        let slots = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(3000., 600.),
            &[],
        );
        let rows = rows_of(&slots);
        assert!(rows.first().is_some_and(|first| (60..90).contains(first)));
        assert!(rows.last().is_some_and(|last| (140..160).contains(last)));
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
        let slots = plan(&entries(300), &heights, px(8.), view(0., 600.), &[]);
        let rows = rows_of(&slots);
        assert!(rows.last().is_some_and(|last| *last < 40));
        let total: f32 = slots
            .iter()
            .map(|slot| match slot {
                Slot::Gap { height, .. } => height.as_f32(),
                Slot::Row(index) => heights.of(&entries(300)[*index]).as_f32(),
            })
            .sum();
        assert_eq!(total, 600. + 298. * 28.);
    }

    #[test]
    fn pinned_rows_are_always_kept_and_split_the_gap() {
        let slots = plan(
            &entries(300),
            &Heights::default(),
            px(8.),
            view(0., 600.),
            &[id(251), id(1)],
        );
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
        let all = plan(&entries(300), &heights, px(8.), view(0., 100_000.), &[]);
        assert_eq!(rows_of(&all).len(), 300);
        let culled = plan(&entries(300), &heights, px(8.), view(0., 600.), &[]);
        let sum = |slots: &[Slot]| -> f32 {
            slots
                .iter()
                .map(|slot| match slot {
                    Slot::Gap { height, .. } => height.as_f32(),
                    Slot::Row(_) => 28.,
                })
                .sum()
        };
        assert_eq!(sum(&all), sum(&culled));
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
        assert_eq!(heights.of(&entry(5)), px(40.));
    }

    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{ScrollDelta, TestAppContext};
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
    fn the_scroll_extent_matches_the_uncuiled_page(cx: &mut TestAppContext) {
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
        scroll_to(&fixture, cx, 3000.);
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
}
