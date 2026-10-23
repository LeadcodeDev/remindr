use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::base::text::editor_host::{BlockNode, FlowPosition, RenderContext};
use gpui_kit::component::{Icon, IconName};
use gpui_kit::{
    AnyElement, AnyView, App, Bounds, Entity, MouseButton, Pixels, SharedString, StyleRefinement,
    TestSupportExt, Window, canvas, div, prelude::*, px,
};
use remindr_core::domain::{BlockId, BlockKind, DatabaseId};

use super::drag::{DragPreview, DraggedBlock};
use super::frame::leaf_key;
use super::palette::Palette;
use super::rows::Row;
use super::{BlockEditor, open_page};
use crate::theme::{
    BLOCK_GAP, BODY_TEXT, CODE_TEXT, H1, H2, H3, RADIUS, RADIUS_LG, UI_TEXT, UI_TEXT_SMALL,
};

pub const GUTTER: Pixels = px(24.);
pub const INDENT: Pixels = px(24.);
pub const LINE: Pixels = px(24.);
const HANDLE_WIDTH: Pixels = px(18.);
const HANDLE_HEIGHT: Pixels = px(22.);

fn heading_style(level: u8) -> StyleRefinement {
    let size = match level {
        1 => H1,
        2 => H2,
        3 => H3,
        _ => return StyleRefinement::default(),
    };
    StyleRefinement::default().text_size(size)
}

pub fn render_context(cx: &App) -> RenderContext {
    let context = RenderContext::from_app(cx).with_body_font_size(BODY_TEXT);
    let style = context
        .style()
        .clone()
        .with_heading(heading_style)
        .with_code_block(StyleRefinement::default().text_size(CODE_TEXT));
    context.with_style(style)
}

pub type RowBounds = Rc<RefCell<Vec<(BlockId, Bounds<Pixels>)>>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowKind {
    Text,
    Bulleted,
    Numbered,
    ToDo { checked: bool },
    Toggle,
    Quote,
    Callout,
    Divider,
    Page,
    Database(DatabaseId),
}

impl RowKind {
    pub fn of(kind: &BlockKind) -> Self {
        match kind {
            BlockKind::Text | BlockKind::Heading(_) | BlockKind::Code { .. } => Self::Text,
            BlockKind::BulletedListItem => Self::Bulleted,
            BlockKind::NumberedListItem => Self::Numbered,
            BlockKind::ToDo { checked } => Self::ToDo { checked: *checked },
            BlockKind::Toggle => Self::Toggle,
            BlockKind::Quote => Self::Quote,
            BlockKind::Callout { .. } => Self::Callout,
            BlockKind::Divider => Self::Divider,
            BlockKind::Page { .. } => Self::Page,
            BlockKind::DatabaseView { database } => Self::Database(*database),
        }
    }
}

pub fn icon_of(kind: &BlockKind) -> Option<SharedString> {
    match kind {
        BlockKind::Callout { icon } => Some(SharedString::from(icon.as_str().to_owned())),
        BlockKind::Page { icon: Some(icon) } => Some(SharedString::from(icon.as_str().to_owned())),
        _ => None,
    }
}

pub struct RowSpec {
    pub row: Row,
    pub kind: RowKind,
    pub icon: Option<SharedString>,
    pub node: Option<BlockNode>,
    pub title: SharedString,
    pub label: SharedString,
    pub selected: bool,
    pub open: bool,
}

pub struct RowEnv<'a> {
    pub editor: &'a Entity<BlockEditor>,
    pub sink: &'a RowBounds,
    pub palette: &'a Palette,
    pub context: &'a RenderContext,
}

pub fn row_id(block: BlockId) -> String {
    format!("block-{}", block.as_uuid())
}

pub fn handle_id(block: BlockId) -> String {
    format!("handle-{}", block.as_uuid())
}

fn group_id(block: BlockId) -> String {
    format!("row-{}", block.as_uuid())
}

pub fn checkbox_id(block: BlockId) -> String {
    format!("checkbox-{}", block.as_uuid())
}

pub fn chevron_id(block: BlockId) -> String {
    format!("chevron-{}", block.as_uuid())
}

pub fn row(
    spec: RowSpec,
    embed: Option<AnyView>,
    env: &RowEnv,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let block = spec.row.block;
    let palette = env.palette;
    let boxed = matches!(spec.kind, RowKind::Callout);
    let sink = env.sink.clone();
    let prefix = prefix(&spec, env);
    let content = content(&spec, embed, env, window, cx);
    let body = div()
        .flex()
        .flex_row()
        .items_start()
        .flex_1()
        .min_w_0()
        .when(boxed, |this| {
            this.bg(palette.muted)
                .rounded(RADIUS_LG)
                .px(px(8.))
                .py(px(4.))
        })
        .children(prefix)
        .child(content);
    div()
        .id(row_id(block))
        .test_support()
        .group(group_id(block))
        .relative()
        .w_full()
        .flex()
        .flex_row()
        .items_start()
        .py(BLOCK_GAP / 2.)
        .rounded(RADIUS)
        .when(spec.selected, |this| this.bg(palette.selection))
        .child(gutter(block, spec.label.clone(), env))
        .child(div().w(INDENT * spec.row.depth as f32).flex_none())
        .child(body)
        .child(
            canvas(
                move |bounds, _, _| sink.borrow_mut().push((block, bounds)),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .into_any_element()
}

fn gutter(block: BlockId, label: SharedString, env: &RowEnv) -> AnyElement {
    let editor = env.editor.clone();
    let palette = *env.palette;
    let group = group_id(block);
    let handle = div()
        .id(handle_id(block))
        .opacity(0.)
        .group_hover(group, |style| style.opacity(1.))
        .on_drag(DraggedBlock { block }, move |_, _, _, cx| {
            editor.update(cx, |this, _| this.begin_block_drag(block));
            cx.new(|_| DragPreview::new(label.clone(), palette.muted, palette.foreground))
        })
        .test_support()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .rounded(RADIUS)
        .cursor_grab()
        .text_size(UI_TEXT_SMALL)
        .text_color(palette.muted_foreground)
        .hover(|style| style.bg(palette.muted))
        .child("\u{22EE}\u{22EE}");
    div()
        .w(GUTTER)
        .h(LINE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .w(HANDLE_WIDTH)
                .h(HANDLE_HEIGHT)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(handle),
        )
        .into_any_element()
}

fn marker(label: impl Into<SharedString>) -> gpui_kit::Div {
    div()
        .w(px(24.))
        .h(LINE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(label.into())
}

fn prefix(spec: &RowSpec, env: &RowEnv) -> Option<AnyElement> {
    let block = spec.row.block;
    match spec.kind {
        RowKind::Bulleted => Some(marker("•").into_any_element()),
        RowKind::Numbered => Some(
            marker(format!("{}.", spec.row.number.unwrap_or(1)))
                .justify_end()
                .pr(px(6.))
                .into_any_element(),
        ),
        RowKind::Callout => spec
            .icon
            .clone()
            .map(|icon| marker(icon).into_any_element()),
        RowKind::ToDo { checked } => Some(checkbox(block, checked, env)),
        RowKind::Toggle => Some(chevron(block, spec.open, env)),
        _ => None,
    }
}

fn checkbox(block: BlockId, checked: bool, env: &RowEnv) -> AnyElement {
    let editor = env.editor.clone();
    let palette = env.palette;
    div()
        .w(px(24.))
        .h(LINE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .id(checkbox_id(block))
                .test_support()
                .size(px(16.))
                .rounded(px(3.))
                .border_1()
                .border_color(palette.border)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .when(checked, |this| {
                    this.bg(palette.primary)
                        .border_color(palette.primary)
                        .child(
                            Icon::new(IconName::Check)
                                .size_3()
                                .text_color(palette.primary_foreground),
                        )
                })
                .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    cx.stop_propagation();
                    editor.update(cx, |this, cx| this.toggle_todo(block, window, cx));
                }),
        )
        .into_any_element()
}

fn chevron(block: BlockId, open: bool, env: &RowEnv) -> AnyElement {
    let editor = env.editor.clone();
    let icon = if open {
        IconName::ChevronDown
    } else {
        IconName::ChevronRight
    };
    div()
        .id(chevron_id(block))
        .test_support()
        .w(px(24.))
        .h(LINE)
        .flex_none()
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(
            Icon::new(icon)
                .size_4()
                .text_color(env.palette.muted_foreground),
        )
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            editor.update(cx, |this, cx| this.toggle_open(block, cx));
        })
        .into_any_element()
}

fn content(
    spec: &RowSpec,
    embed: Option<AnyView>,
    env: &RowEnv,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let palette = env.palette;
    let block = spec.row.block;
    match spec.kind {
        RowKind::Divider => div()
            .flex_1()
            .h(LINE)
            .flex()
            .items_center()
            .child(div().w_full().h(px(1.)).bg(palette.border))
            .into_any_element(),
        RowKind::Page => {
            let glyph = match &spec.icon {
                Some(icon) => div().child(icon.clone()).into_any_element(),
                None => Icon::new(IconName::File)
                    .size_4()
                    .text_color(palette.muted_foreground)
                    .into_any_element(),
            };
            div()
                .id(("page-link", leaf_key(block)))
                .test_support()
                .h(LINE)
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .cursor_pointer()
                .child(glyph)
                .child(div().flex_none().underline().child(spec.title.clone()))
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    cx.stop_propagation();
                    open_page(block, cx);
                })
                .into_any_element()
        }
        RowKind::Database(_) => match embed {
            Some(view) => div()
                .flex_1()
                .min_w_0()
                .text_size(UI_TEXT)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(view)
                .into_any_element(),
            None => div()
                .flex_1()
                .h(px(120.))
                .rounded(RADIUS_LG)
                .bg(palette.muted)
                .into_any_element(),
        },
        kind => {
            let text = match &spec.node {
                Some(node) => {
                    let alone = [node];
                    let flow = FlowPosition::at(&alone, 0, true);
                    node.render_with_id(("text", leaf_key(block)), flow, env.context, window, cx)
                }
                None => div().into_any_element(),
            };
            let struck = matches!(kind, RowKind::ToDo { checked: true });
            let quoted = matches!(kind, RowKind::Quote);
            div()
                .flex_1()
                .min_w_0()
                .when(struck, |this| this.text_color(palette.muted_foreground))
                .when(quoted, |this| {
                    this.border_l(px(3.))
                        .border_color(palette.border)
                        .pl(px(12.))
                })
                .child(text)
                .into_any_element()
        }
    }
}
