use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::Range;
use std::rc::Rc;

use gpui_kit::base::text::editor_host::{BlockNode, RenderContext};
use gpui_kit::{
    AnyView, App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, Hsla, IntoElement,
    Pixels, Render, ScrollHandle, SharedString, Subscription, TestSupportExt, Window, div,
    prelude::*, px,
};
use remindr_core::application::{Changes, TextPoint};
use remindr_core::domain::{Block, BlockId, BlockKind, HeadingLevel, Parent};

use crate::app::{NavigatorHandle, Route, WorkspaceModel};
use crate::embed::EmbedRegistry;

mod actions;
mod caret;
mod clipboard;
mod editing;
mod element;
mod frame;
mod input;
mod navigation;
mod palette;
mod render;
mod rows;
#[cfg(test)]
mod testing;
mod text;
#[cfg(test)]
mod ui_tests;

use caret::{Blink, Marked, Selection, floor_boundary};
use element::EditorRoot;
use frame::{CARET_WIDTH, Frame, leaf_key};
use palette::Palette;
use render::{RowBounds, RowEnv, RowKind, RowSpec, icon_of};
use rows::{Expansion, Extent, Row, extent, position, rows};

const KEY_CONTEXT: &str = "BlockEditor";

pub struct BlockEditor {
    model: Entity<WorkspaceModel>,
    page: BlockId,
    focus: FocusHandle,
    scroll: ScrollHandle,
    selection: Option<Selection>,
    marked: Option<Marked>,
    goal_x: Option<Pixels>,
    expanded: HashSet<BlockId>,
    dragging: bool,
    order: Vec<BlockId>,
    frame: Rc<RefCell<Frame>>,
    row_bounds: RowBounds,
    blink: Entity<Blink>,
    embeds: HashMap<BlockId, AnyView>,
    reveal: Cell<bool>,
    own_events: usize,
    _subscriptions: Vec<Subscription>,
}

struct Snapshot {
    specs: Vec<RowSpec>,
    decorations: HashMap<u64, Vec<(Range<usize>, Hsla)>>,
    blocks: Vec<BlockId>,
}

pub fn init(cx: &mut App) {
    actions::bind(cx);
}

fn is_atomic_row(kind: &BlockKind) -> bool {
    matches!(
        kind,
        BlockKind::Divider | BlockKind::DatabaseView { .. } | BlockKind::Page { .. }
    )
}

fn open_page(block: BlockId, cx: &mut App) {
    let Some(handle) = cx.try_global::<NavigatorHandle>() else {
        return;
    };
    let navigator = handle.0.clone();
    navigator.update(cx, |navigator, cx| navigator.open(Route::Page(block), cx));
}

impl BlockEditor {
    pub fn new(
        model: Entity<WorkspaceModel>,
        page: BlockId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus = cx.focus_handle();
        let blink = cx.new(|_| Blink::new());
        let subscriptions = vec![
            cx.observe(&blink, |_, _, cx| cx.notify()),
            cx.observe(&model, |_, _, cx| cx.notify()),
            cx.subscribe_in(&model, window, Self::on_changes),
            cx.on_blur(&focus, window, |this, window, cx| {
                this.commit_marked(window, cx);
                this.blink.update(cx, |blink, cx| blink.stop(cx));
            }),
        ];
        let order = rows(model.read(cx).workspace(), page, Expansion::All)
            .into_iter()
            .map(|row| row.block)
            .collect();
        Self {
            model,
            page,
            focus,
            scroll: ScrollHandle::new(),
            selection: None,
            marked: None,
            goal_x: None,
            expanded: HashSet::new(),
            dragging: false,
            order,
            frame: Rc::default(),
            row_bounds: Rc::default(),
            blink,
            embeds: HashMap::new(),
            reveal: Cell::new(false),
            own_events: 0,
            _subscriptions: subscriptions,
        }
    }

    #[cfg(test)]
    fn selection(&self) -> Option<Selection> {
        self.selection
    }

    #[cfg(test)]
    fn place_caret(&mut self, point: TextPoint, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        self.set_selection(Some(Selection::collapsed(point)), cx);
    }

    fn visible_rows(&self, cx: &App) -> Vec<Row> {
        rows(
            self.model.read(cx).workspace(),
            self.page,
            Expansion::Only(&self.expanded),
        )
    }

    fn block<'a>(&self, block: BlockId, cx: &'a App) -> Option<&'a Block> {
        self.model
            .read(cx)
            .workspace()
            .block(block)
            .filter(|found| found.alive)
    }

    fn kind_of(&self, block: BlockId, cx: &App) -> Option<BlockKind> {
        self.block(block, cx).map(|found| found.kind.clone())
    }

    fn plain_of(&self, block: BlockId, cx: &App) -> Option<String> {
        self.block(block, cx).map(|found| found.text.to_plain())
    }

    fn text_len(&self, point_block: BlockId, cx: &App) -> usize {
        self.block(point_block, cx)
            .filter(|found| !is_atomic_row(&found.kind))
            .map_or(0, |found| found.text.len())
    }

    fn owns(&self, block: BlockId, cx: &App) -> bool {
        let workspace = self.model.read(cx).workspace();
        let mut current = workspace.block(block).map(|found| found.parent);
        while let Some(Parent::Block(parent)) = current {
            if parent == self.page {
                return true;
            }
            current = workspace.block(parent).map(|found| found.parent);
        }
        false
    }

    fn set_selection(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        self.selection = selection;
        self.goal_x = None;
        self.reveal.set(true);
        self.blink.update(cx, |blink, cx| blink.restart(cx));
        cx.notify();
    }

    fn set_caret(&mut self, point: TextPoint, cx: &mut Context<Self>) {
        self.set_selection(Some(Selection::collapsed(point)), cx);
    }

    fn on_changes(
        &mut self,
        _: &Entity<WorkspaceModel>,
        changes: &Changes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.own_events > 0 {
            self.own_events -= 1;
            return;
        }
        let moved = changes.caret.filter(|point| self.owns(point.block, cx));
        if moved.is_some() {
            window.focus(&self.focus, cx);
        }
        self.settle(changes, cx);
    }

    fn settle(&mut self, changes: &Changes, cx: &mut Context<Self>) {
        match changes.caret.filter(|point| self.owns(point.block, cx)) {
            Some(point) => self.set_caret(point, cx),
            None => self.clamp_selection(cx),
        }
        self.order = self
            .visible_rows(cx)
            .into_iter()
            .map(|row| row.block)
            .collect();
    }

    fn clamp_selection(&mut self, cx: &mut Context<Self>) {
        let Some(selection) = self.selection else {
            return;
        };
        let rows = self.visible_rows(cx);
        let anchor = self.clamp_point(selection.anchor, &rows, cx);
        let head = self.clamp_point(selection.head, &rows, cx);
        let clamped = anchor
            .zip(head)
            .map(|(anchor, head)| Selection { anchor, head });
        if clamped != self.selection {
            self.set_selection(clamped, cx);
        }
    }

    fn valid_point(&self, point: TextPoint, cx: &App) -> TextPoint {
        let offset = match self.block(point.block, cx) {
            Some(block) if is_atomic_row(&block.kind) => 0,
            Some(block) => floor_boundary(&block.text.to_plain(), point.offset),
            None => 0,
        };
        TextPoint {
            block: point.block,
            offset,
        }
    }

    fn clamp_point(&self, point: TextPoint, rows: &[Row], cx: &App) -> Option<TextPoint> {
        if position(rows, point.block).is_some() {
            return Some(self.valid_point(point, cx));
        }
        let old = self.order.iter().position(|block| *block == point.block)?;
        let alive = |block: &&BlockId| position(rows, **block).is_some();
        if let Some(before) = self.order[..old].iter().rev().find(alive) {
            let offset = self.text_len(*before, cx);
            return Some(TextPoint {
                block: *before,
                offset,
            });
        }
        self.order[old + 1..]
            .iter()
            .find(alive)
            .map(|after| TextPoint {
                block: *after,
                offset: 0,
            })
    }

    fn reveal_caret(&self, window: &mut Window) {
        if !self.reveal.get() {
            return;
        }
        let Some(selection) = self.selection else {
            self.reveal.set(false);
            return;
        };
        let caret = self.frame.borrow().caret_bounds(
            selection.head.block,
            selection.head.offset,
            CARET_WIDTH,
        );
        let Some(caret) = caret else {
            return;
        };
        self.reveal.set(false);
        let view: Bounds<Pixels> = self.scroll.bounds();
        let mut offset = self.scroll.offset();
        if caret.top() < view.top() {
            offset.y += view.top() - caret.top();
        } else if caret.bottom() > view.bottom() {
            offset.y -= caret.bottom() - view.bottom();
        } else {
            return;
        }
        self.scroll.set_offset(offset);
        window.refresh();
    }

    fn snapshot(&self, palette: &Palette, cx: &App) -> Snapshot {
        let workspace = self.model.read(cx).workspace();
        let rows = self.visible_rows(cx);
        let range = self
            .selection
            .filter(|_| self.marked.is_none())
            .and_then(|selection| extent(&rows, selection.anchor, selection.head));
        let mut specs = Vec::with_capacity(rows.len());
        let mut decorations: HashMap<u64, Vec<(Range<usize>, Hsla)>> = HashMap::new();
        for (index, row) in rows.iter().enumerate() {
            let Some(block) = workspace.block(row.block) else {
                continue;
            };
            let atomic = is_atomic_row(&block.kind);
            let inside = range.is_some_and(|span| (span.first..=span.last).contains(&index));
            let key = leaf_key(block.id);
            if !atomic && let Some(part) = selected_part(range, index, block.text.len()) {
                decorations
                    .entry(key)
                    .or_default()
                    .push((part, palette.selection));
            }
            specs.push(RowSpec {
                row: *row,
                kind: RowKind::of(&block.kind),
                icon: icon_of(&block.kind),
                node: self.node_of(block, palette, key),
                title: title_of(block),
                selected: atomic && inside,
                open: self.expanded.contains(&block.id),
            });
        }
        Snapshot {
            specs,
            decorations,
            blocks: rows.iter().map(|row| row.block).collect(),
        }
    }

    fn node_of(&self, block: &Block, palette: &Palette, key: u64) -> Option<BlockNode> {
        if is_atomic_row(&block.kind) {
            return None;
        }
        let composed = self
            .marked
            .as_ref()
            .filter(|marked| marked.block == block.id)
            .map(|marked| text::composed(&block.text, marked));
        let shown = composed.as_ref().unwrap_or(&block.text);
        Some(match &block.kind {
            BlockKind::Code { language } => {
                BlockNode::CodeBlock(text::code_block(shown, language, key))
            }
            BlockKind::Heading(level) => BlockNode::Heading {
                level: match level {
                    HeadingLevel::One => 1,
                    HeadingLevel::Two => 2,
                    HeadingLevel::Three => 3,
                },
                children: text::paragraph(shown, palette, key, false),
                span: None,
            },
            kind => BlockNode::Paragraph(text::paragraph(
                shown,
                palette,
                key,
                matches!(kind, BlockKind::ToDo { checked: true }),
            )),
        })
    }

    fn embed(&mut self, spec: &RowSpec, window: &mut Window, cx: &mut App) -> Option<AnyView> {
        let RowKind::Database(database) = spec.kind else {
            return None;
        };
        let block = spec.row.block;
        if let Some(view) = self.embeds.get(&block) {
            return Some(view.clone());
        }
        if !cx.has_global::<EmbedRegistry>() {
            return None;
        }
        let view = cx.update_global::<EmbedRegistry, _>(|registry, cx| {
            registry.build(block, database, window, cx)
        })?;
        self.embeds.insert(block, view.clone());
        Some(view)
    }
}

fn selected_part(span: Option<Extent>, index: usize, length: usize) -> Option<Range<usize>> {
    let span = span.filter(|span| (span.first..=span.last).contains(&index))?;
    let from = if index == span.first {
        span.start.offset
    } else {
        0
    };
    let to = if index == span.last {
        span.end.offset
    } else {
        length
    };
    (from < to).then_some(from..to)
}

fn title_of(block: &Block) -> SharedString {
    match block.kind {
        BlockKind::Page { .. } if block.text.is_empty() => SharedString::from("Untitled"),
        BlockKind::Page { .. } => SharedString::from(block.text.to_plain()),
        _ => SharedString::default(),
    }
}

impl Focusable for BlockEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BlockEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = Palette::of(cx);
        let context = RenderContext::from_app(cx)
            .with_body_font_size(px(16.))
            .on_link_click(|_, _, _, _| {})
            .with_code_block_highlighter(|_| Vec::new());
        let Snapshot {
            specs,
            decorations,
            blocks,
        } = self.snapshot(&palette, cx);
        let alive: HashSet<BlockId> = blocks.iter().copied().collect();
        self.embeds.retain(|block, _| alive.contains(block));
        self.order = blocks;
        self.row_bounds.borrow_mut().clear();
        let editor = cx.entity();
        let sink = self.row_bounds.clone();
        let env = RowEnv {
            editor: &editor,
            sink: &sink,
            palette: &palette,
            context: &context,
        };
        let mut elements = Vec::with_capacity(specs.len());
        for spec in specs {
            let embed = self.embed(&spec, window, cx);
            elements.push(render::row(spec, embed, &env, window, cx));
        }
        let content = div()
            .w_full()
            .min_h_full()
            .pt(px(8.))
            .pb(px(160.))
            .children(elements)
            .into_any_element();
        let root = div()
            .id("block-editor")
            .test_support()
            .key_context(KEY_CONTEXT)
            .track_focus(&self.focus)
            .track_scroll(&self.scroll)
            .size_full()
            .cursor_text()
            .overflow_y_scroll()
            .text_size(px(16.))
            .text_color(palette.foreground)
            .child(EditorRoot {
                editor: editor.clone(),
                child: Some(content),
                decorations,
                palette,
            });
        actions::handle(root, cx)
    }
}
