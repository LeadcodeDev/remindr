use gpui_kit::{
    Anchor, AnyElement, App, Bounds, Context, Entity, MouseButton, Pixels, Point, ScrollHandle,
    SharedString, Size, TestSupportExt, Window, anchored, deferred, div, point, prelude::*, px,
};

use super::BlockEditor;
use super::link::{ERROR_HEIGHT, LinkEntry, link_box};
use super::palette::Palette;
use super::slash::{Picker, Slash};

pub const MENU_WIDTH: Pixels = px(320.);
pub const ROW_HEIGHT: Pixels = px(28.);
pub const MAX_ROWS: usize = 8;
const GAP: Pixels = px(4.);
const PAD: Pixels = px(4.);
const EDGE: Pixels = px(8.);

pub enum Overlay {
    Slash(Slash),
    Picker(Picker),
    Link(LinkEntry),
}

pub struct MenuRow {
    pub label: SharedString,
    pub detail: Option<SharedString>,
    pub icon: Option<SharedString>,
    pub enabled: bool,
}

impl MenuRow {
    fn description(&self) -> SharedString {
        match &self.detail {
            Some(detail) if !self.enabled => {
                SharedString::from(format!("{}, {detail}", self.label))
            }
            _ => self.label.clone(),
        }
    }
}

pub struct ListIds {
    pub menu: &'static str,
    pub list: &'static str,
    pub item: &'static str,
    pub empty: &'static str,
}

pub const SLASH_IDS: ListIds = ListIds {
    menu: "slash-menu",
    list: "slash-list",
    item: "slash-item",
    empty: "slash-empty",
};

pub const PICKER_IDS: ListIds = ListIds {
    menu: "database-picker",
    list: "picker-list",
    item: "picker-item",
    empty: "picker-empty",
};

pub struct ListMenu<'a> {
    pub ids: ListIds,
    pub rows: Vec<MenuRow>,
    pub selected: usize,
    pub empty_text: &'static str,
    pub scroll: &'a ScrollHandle,
}

pub struct Placement {
    pub position: Point<Pixels>,
    pub anchor: Anchor,
}

pub fn menu_height(rows: usize) -> Pixels {
    ROW_HEIGHT * rows.clamp(1, MAX_ROWS) as f32 + PAD * 2. + px(2.)
}

pub fn place(line: Bounds<Pixels>, height: Pixels, window: Size<Pixels>) -> Placement {
    let below = line.bottom() + GAP;
    let above = line.top() - GAP;
    if below + height <= window.height || above - height < px(0.) {
        Placement {
            position: point(line.left(), below),
            anchor: Anchor::TopLeft,
        }
    } else {
        Placement {
            position: point(line.left(), above),
            anchor: Anchor::BottomLeft,
        }
    }
}

pub fn floating(placement: Placement, content: AnyElement) -> AnyElement {
    deferred(
        anchored()
            .anchor(placement.anchor)
            .position(placement.position)
            .snap_to_window_with_margin(EDGE)
            .child(content),
    )
    .with_priority(1)
    .into_any_element()
}

pub fn list_menu(
    menu: ListMenu<'_>,
    palette: &Palette,
    pick: impl Fn(usize, &mut Window, &mut App) + Clone + 'static,
) -> AnyElement {
    let ListMenu {
        ids,
        rows,
        selected,
        empty_text,
        scroll,
    } = menu;
    let palette = *palette;
    let empty = rows.is_empty();
    let items = rows.into_iter().enumerate().map(|(index, row)| {
        let pick = pick.clone();
        let enabled = row.enabled;
        let chosen = index == selected;
        div()
            .id((ids.item, index as u64))
            .aria_label(row.description())
            .test_support()
            .flex_none()
            .h(ROW_HEIGHT)
            .px(px(8.))
            .gap(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .rounded(px(4.))
            .when(chosen && enabled, |this| this.bg(palette.muted))
            .when(enabled, |this| {
                this.cursor_pointer().hover(|style| style.bg(palette.muted))
            })
            .when(!enabled, |this| this.text_color(palette.muted_foreground))
            .children(
                row.icon
                    .map(|icon| div().flex_none().w(px(20.)).child(icon).into_any_element()),
            )
            .child(div().flex_1().min_w_0().truncate().child(row.label))
            .children(row.detail.map(|detail| {
                div()
                    .flex_none()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child(detail)
                    .into_any_element()
            }))
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                cx.stop_propagation();
                if enabled {
                    pick(index, window, cx);
                }
            })
    });
    let body = if empty {
        div()
            .id(ids.empty)
            .test_support()
            .h(ROW_HEIGHT)
            .px(px(8.))
            .flex()
            .items_center()
            .text_color(palette.muted_foreground)
            .child(empty_text)
            .into_any_element()
    } else {
        div()
            .id(ids.list)
            .max_h(ROW_HEIGHT * MAX_ROWS as f32)
            .overflow_y_scroll()
            .track_scroll(scroll)
            .flex()
            .flex_col()
            .children(items)
            .into_any_element()
    };
    surface(ids.menu, &palette).child(body).into_any_element()
}

pub fn surface(id: &'static str, palette: &Palette) -> impl ParentElement + IntoElement {
    div()
        .id(id)
        .test_support()
        .occlude()
        .w(MENU_WIDTH)
        .p(PAD)
        .rounded(px(8.))
        .border_1()
        .border_color(palette.border)
        .bg(palette.popover)
        .text_color(palette.popover_foreground)
        .text_size(px(14.))
        .shadow_md()
}

impl BlockEditor {
    pub(super) fn overlay_element(
        &self,
        editor: &Entity<BlockEditor>,
        palette: &Palette,
        window: &Window,
        cx: &App,
    ) -> Option<AnyElement> {
        match self.overlay.as_ref()? {
            Overlay::Slash(slash) => {
                let rows = self.slash_rows(slash, cx);
                let line = self.frame.borrow().line_box(slash.block, slash.start)?;
                let placement = place(line, menu_height(rows.len()), window.viewport_size());
                let editor = editor.clone();
                let menu = list_menu(
                    ListMenu {
                        ids: SLASH_IDS,
                        rows,
                        selected: slash.selected,
                        empty_text: "No results",
                        scroll: &slash.scroll,
                    },
                    palette,
                    move |index, window, cx| {
                        editor.update(cx, |this, cx| this.slash_pick(index, window, cx));
                    },
                );
                Some(floating(placement, menu))
            }
            Overlay::Picker(picker) => {
                let rows = self.picker_rows(picker, cx);
                let line = self.frame.borrow().line_box(picker.block, picker.start)?;
                let placement = place(line, menu_height(rows.len()), window.viewport_size());
                let editor = editor.clone();
                let menu = list_menu(
                    ListMenu {
                        ids: PICKER_IDS,
                        rows,
                        selected: picker.selected,
                        empty_text: "No databases found",
                        scroll: &picker.scroll,
                    },
                    palette,
                    move |index, window, cx| {
                        editor.update(cx, |this, cx| this.picker_pick(index, window, cx));
                    },
                );
                Some(floating(placement, menu))
            }
            Overlay::Link(entry) => {
                let line = self
                    .frame
                    .borrow()
                    .line_box(entry.head.block, entry.head.offset)?;
                let extra = if entry.error { ERROR_HEIGHT } else { px(0.) };
                let placement = place(line, menu_height(1) + extra, window.viewport_size());
                Some(floating(placement, link_box(entry, palette)))
            }
        }
    }

    pub(super) fn overlay_blocks(&self) -> Vec<remindr_core::domain::BlockId> {
        match &self.overlay {
            Some(Overlay::Slash(slash)) => vec![slash.block],
            Some(Overlay::Picker(picker)) => vec![picker.block],
            Some(Overlay::Link(entry)) => vec![entry.head.block],
            None => Vec::new(),
        }
    }

    pub(super) fn modal(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Picker(_) | Overlay::Link(_)))
    }

    pub(super) fn dismiss_overlay(&mut self, cx: &mut Context<Self>) -> bool {
        if self.overlay.take().is_some() {
            cx.notify();
            return true;
        }
        false
    }

    pub(super) fn overlay_step(&mut self, down: bool, cx: &mut Context<Self>) -> bool {
        match self.overlay {
            Some(Overlay::Slash(_)) => self.slash_step(down, cx),
            Some(Overlay::Picker(_)) => self.picker_step(down, cx),
            Some(Overlay::Link(_)) => {}
            None => return false,
        }
        true
    }

    pub(super) fn overlay_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        match self.overlay {
            Some(Overlay::Slash(_)) => return self.slash_confirm(window, cx),
            Some(Overlay::Picker(_)) => self.picker_confirm(window, cx),
            Some(Overlay::Link(_)) => self.link_commit(window, cx),
            None => return false,
        }
        true
    }

    pub(super) fn modal_text(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        match self.overlay {
            Some(Overlay::Picker(_)) => {
                self.picker_type(text, cx);
                true
            }
            Some(Overlay::Link(_)) => {
                self.link_type(text, cx);
                true
            }
            _ => false,
        }
    }

    pub(super) fn modal_backspace(&mut self, cx: &mut Context<Self>) -> bool {
        match self.overlay {
            Some(Overlay::Picker(_)) => {
                self.picker_backspace(cx);
                true
            }
            Some(Overlay::Link(_)) => {
                self.link_backspace(cx);
                true
            }
            _ => false,
        }
    }
}
