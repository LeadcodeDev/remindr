use std::rc::Rc;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::sidebar::SidebarItem;
use gpui_kit::component::{
    ActiveTheme as _, Collapsible, Icon, IconName, Sizable as _, StyledExt as _, h_flex, v_flex,
};
use gpui_kit::{
    AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, SharedString, StatefulInteractiveElement as _, Styled as _,
    TestSupportExt as _, Window, div, prelude::FluentBuilder as _, px,
};
use remindr_core::domain::{BlockId, DatabaseId};

use super::Handles;
use super::entries::{
    Entry, Target, database_entry_id, database_menu_id, database_open_id, page_add_id,
    page_entry_id, page_icon_id, page_menu_id, page_open_id, page_toggle_id,
};
use super::rename::{self, RenameTarget};
use crate::app::Route;
use crate::theme::{SIDEBAR_ICON, SIDEBAR_INDENT, SIDEBAR_ROW, UI_TEXT, UI_TEXT_SMALL};

#[derive(Clone)]
pub struct Section {
    id: &'static str,
    label: &'static str,
    entries: Rc<Vec<Entry>>,
    handles: Handles,
    collapsed: bool,
}

impl Section {
    pub fn new(
        id: &'static str,
        label: &'static str,
        entries: Vec<Entry>,
        handles: &Handles,
    ) -> Self {
        Self {
            id,
            label,
            entries: Rc::new(entries),
            handles: handles.clone(),
            collapsed: false,
        }
    }
}

impl Collapsible for Section {
    fn is_collapsed(&self) -> bool {
        self.collapsed
    }

    fn collapsed(mut self, collapsed: bool) -> Self {
        self.collapsed = collapsed;
        self
    }
}

impl SidebarItem for Section {
    fn render(self, _: impl Into<ElementId>, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let heading = cx.theme().sidebar_foreground.opacity(0.7);
        let entries: Vec<AnyElement> = self
            .entries
            .iter()
            .map(|entry| render_entry(entry, 0, &self.handles, cx))
            .collect();
        v_flex()
            .id(self.id)
            .test_support()
            .w_full()
            .child(
                h_flex()
                    .h(SIDEBAR_ROW)
                    .px_2()
                    .items_center()
                    .text_size(UI_TEXT_SMALL)
                    .text_color(heading)
                    .child(self.label),
            )
            .child(v_flex().gap_0p5().children(entries))
    }
}

fn render_entry(entry: &Entry, depth: usize, handles: &Handles, cx: &App) -> AnyElement {
    let theme = cx.theme();
    let accent = theme.sidebar_accent;
    let foreground = theme.foreground;
    let muted = theme.muted_foreground;
    let radius = theme.radius;
    let group: SharedString = format!("sidebar-entry-{}", entry.key()).into();
    let (entry_id, open_id, route) = match entry.target {
        Target::Page(page) => (page_entry_id(page), page_open_id(page), Route::Page(page)),
        Target::Database(database) => (
            database_entry_id(database),
            database_open_id(database),
            Route::Database(database),
        ),
    };

    let leading = match entry.target {
        Target::Page(page) if !entry.children.is_empty() => {
            toggle_button(page, entry.expanded, handles).into_any_element()
        }
        _ => div().w(SIDEBAR_ICON).flex_none().into_any_element(),
    };

    let icon = match (&entry.emoji, entry.target) {
        (Some(emoji), Target::Page(page)) => div()
            .id(page_icon_id(page))
            .aria_label(emoji.clone())
            .test_support()
            .w(SIDEBAR_ICON)
            .flex_none()
            .text_center()
            .child(emoji.clone())
            .into_any_element(),
        (Some(emoji), Target::Database(_)) => div()
            .w(SIDEBAR_ICON)
            .flex_none()
            .text_center()
            .child(emoji.clone())
            .into_any_element(),
        (None, _) => div()
            .w(SIDEBAR_ICON)
            .flex_none()
            .flex()
            .justify_center()
            .child(
                Icon::new(match entry.target {
                    Target::Page(_) => IconName::FileText,
                    Target::Database(_) => IconName::LayoutDashboard,
                })
                .size(SIDEBAR_ICON)
                .text_color(muted),
            )
            .into_any_element(),
    };

    let open = {
        let handles = handles.clone();
        move |_: &ClickEvent, _: &mut Window, cx: &mut App| handles.open(route, cx)
    };

    let label_region = h_flex()
        .id(open_id)
        .flex_1()
        .min_w_0()
        .h_full()
        .gap_2()
        .items_center()
        .cursor_pointer()
        .role(Role::TreeItem)
        .aria_label(entry.label.clone())
        .aria_selected(entry.active)
        .on_click(open)
        .test_support()
        .child(icon)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .child(entry.label.clone()),
        );

    let line = h_flex()
        .id(entry_id)
        .test_support()
        .group(group.clone())
        .w_full()
        .h(SIDEBAR_ROW)
        .items_center()
        .gap_0p5()
        .rounded(radius)
        .text_size(UI_TEXT)
        .pl(SIDEBAR_INDENT * depth as f32 + px(2.))
        .when(entry.active, |this| {
            this.bg(accent).font_medium().text_color(foreground)
        })
        .when(!entry.active, |this| this.hover(|style| style.bg(accent)))
        .child(leading)
        .child(label_region)
        .child(actions(entry, group, handles));

    if entry.expanded {
        v_flex()
            .child(line)
            .children(
                entry
                    .children
                    .iter()
                    .map(|child| render_entry(child, depth + 1, handles, cx)),
            )
            .into_any_element()
    } else {
        line.into_any_element()
    }
}

fn toggle_button(page: BlockId, expanded: bool, handles: &Handles) -> Button {
    let handles = handles.clone();
    Button::new(page_toggle_id(page))
        .ghost()
        .xsmall()
        .size(SIDEBAR_ICON)
        .icon(if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .accessibility_label(if expanded {
            "Collapse sub-pages"
        } else {
            "Expand sub-pages"
        })
        .on_click(move |_, _, cx| handles.toggle(page, cx))
}

fn actions(entry: &Entry, group: SharedString, handles: &Handles) -> AnyElement {
    let container = h_flex()
        .flex_none()
        .gap_0p5()
        .pr_1()
        .opacity(0.)
        .group_hover(group, |style| style.opacity(1.));
    match entry.target {
        Target::Page(page) => container
            .child(add_button(page, handles))
            .child(page_menu(page, handles))
            .into_any_element(),
        Target::Database(database) => container
            .child(database_menu(database, handles))
            .into_any_element(),
    }
}

fn add_button(page: BlockId, handles: &Handles) -> Button {
    let handles = handles.clone();
    Button::new(page_add_id(page))
        .ghost()
        .xsmall()
        .icon(IconName::Plus)
        .accessibility_label("Add a sub-page")
        .on_click(move |_, window, cx| handles.add_child_page(page, window, cx))
}

fn page_menu(page: BlockId, handles: &Handles) -> impl IntoElement {
    let handles = handles.clone();
    Button::new(page_menu_id(page))
        .ghost()
        .xsmall()
        .icon(IconName::Ellipsis)
        .accessibility_label("Page options")
        .dropdown_menu(move |menu, _, _| {
            let rename_handles = handles.clone();
            let delete_handles = handles.clone();
            menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                rename::open(RenameTarget::Page(page), &rename_handles.model, window, cx);
            }))
            .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                delete_handles.delete_page(page, window, cx);
            }))
        })
}

fn database_menu(database: DatabaseId, handles: &Handles) -> impl IntoElement {
    let handles = handles.clone();
    Button::new(database_menu_id(database))
        .ghost()
        .xsmall()
        .icon(IconName::Ellipsis)
        .accessibility_label("Database options")
        .dropdown_menu(move |menu, _, _| {
            let rename_handles = handles.clone();
            let delete_handles = handles.clone();
            menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                rename::open(
                    RenameTarget::Database(database),
                    &rename_handles.model,
                    window,
                    cx,
                );
            }))
            .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                delete_handles.delete_database(database, window, cx);
            }))
        })
}
