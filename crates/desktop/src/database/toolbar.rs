use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, h_flex};
use gpui_kit::{
    Anchor, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, div, prelude::FluentBuilder as _,
};
use remindr_core::application::Command;
use remindr_core::domain::{DatabaseId, ViewId};

use super::filter::FilterPanel;
use super::sort::SortPanel;
use super::ui::icon_label;
use crate::app::WorkspaceModel;
use crate::app::commands;
use crate::theme::{UI_TEXT_SMALL, labeled};

pub struct Dismiss;

pub struct Toolbar {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    view: ViewId,
    filters: Entity<FilterPanel>,
    sorts: Entity<SortPanel>,
    filter_open: bool,
    sort_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl Toolbar {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        view: ViewId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let filters = {
            let model = model.clone();
            cx.new(|cx| FilterPanel::new(model, database, view, window, cx))
        };
        let sorts = {
            let model = model.clone();
            cx.new(|cx| SortPanel::new(model, database, view, window, cx))
        };
        let subscriptions = vec![
            cx.subscribe(&filters, |this, _, _: &Dismiss, cx| {
                this.filter_open = false;
                cx.notify();
            }),
            cx.subscribe(&sorts, |this, _, _: &Dismiss, cx| {
                this.sort_open = false;
                cx.notify();
            }),
            cx.observe(&model, |_, _, cx| cx.notify()),
        ];
        Self {
            model,
            database,
            view,
            filters,
            sorts,
            filter_open: false,
            sort_open: false,
            _subscriptions: subscriptions,
        }
    }

    fn counts(&self, cx: &Context<Self>) -> (usize, usize) {
        self.model
            .read(cx)
            .workspace()
            .view(self.view)
            .map_or((0, 0), |view| (view.filters.len(), view.sorts.len()))
    }

    fn add_row(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(
            &self.model,
            Command::AddRow {
                database: self.database,
                values: Vec::new(),
            },
            window,
            cx,
        );
    }
}

fn counted(
    button: Button,
    id: &'static str,
    label: &'static str,
    count: usize,
    cx: &Context<Toolbar>,
) -> Button {
    let pill = cx.theme().primary;
    let ink = cx.theme().primary_foreground;
    labeled(button, label).when(count > 0, |this| {
        this.child(
            div()
                .id(id)
                .test_support()
                .aria_label(count.to_string())
                .px_1()
                .rounded_full()
                .bg(pill)
                .text_color(ink)
                .text_size(UI_TEXT_SMALL)
                .child(count.to_string()),
        )
    })
}

impl Render for Toolbar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (filters, sorts) = self.counts(cx);
        let filter_button = counted(
            Button::new("toolbar-filter").ghost().small(),
            "toolbar-filter-count",
            "Filter",
            filters,
            cx,
        );
        let sort_button = counted(
            Button::new("toolbar-sort").ghost().small(),
            "toolbar-sort-count",
            "Sort",
            sorts,
            cx,
        );
        let (filter_owner, sort_owner) = (cx.entity(), cx.entity());
        let (filter_panel, sort_panel) = (self.filters.clone(), self.sorts.clone());
        h_flex()
            .id("database-toolbar")
            .gap_1()
            .items_center()
            .child(
                Popover::new("toolbar-filter-popover")
                    .anchor(Anchor::TopRight)
                    .open(self.filter_open)
                    .overlay_closable(false)
                    .on_open_change(move |open, _, cx| {
                        let open = *open;
                        filter_owner.update(cx, |this, cx| {
                            this.filter_open = open;
                            cx.notify();
                        });
                    })
                    .trigger(filter_button)
                    .content(move |_, _, _| filter_panel.clone()),
            )
            .child(
                Popover::new("toolbar-sort-popover")
                    .anchor(Anchor::TopRight)
                    .open(self.sort_open)
                    .overlay_closable(false)
                    .on_open_change(move |open, _, cx| {
                        let open = *open;
                        sort_owner.update(cx, |this, cx| {
                            this.sort_open = open;
                            cx.notify();
                        });
                    })
                    .trigger(sort_button)
                    .content(move |_, _, _| sort_panel.clone()),
            )
            .child(
                Button::new("toolbar-new")
                    .primary()
                    .small()
                    .child(icon_label(IconName::Plus, "New"))
                    .on_click(cx.listener(|this, _, window, cx| this.add_row(window, cx))),
            )
    }
}
