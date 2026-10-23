use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, IconName, Sizable as _, h_flex};
use gpui_kit::{
    Anchor, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, div, prelude::FluentBuilder as _,
};
use remindr_core::application::Command;
use remindr_core::domain::{DatabaseId, PropertyId, ViewId};

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

    fn toggle_property(
        &mut self,
        property: PropertyId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = self.model.read(cx).workspace().view(self.view) else {
            return;
        };
        let mut properties = view.visible.clone();
        match properties.iter().position(|shown| *shown == property) {
            Some(position) => {
                properties.remove(position);
            }
            None => properties.push(property),
        }
        commands::run(
            &self.model,
            Command::SetVisibleProperties {
                view: self.view,
                properties,
            },
            window,
            cx,
        );
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
        let properties_button = {
            let toolbar = cx.entity();
            let (model, database, view) = (self.model.clone(), self.database, self.view);
            labeled(
                Button::new("toolbar-properties").ghost().small(),
                "Properties",
            )
            .dropdown_menu(move |menu, _, cx| {
                let workspace = model.read(cx).workspace();
                let Some(database) = workspace.database(database) else {
                    return menu;
                };
                let shown = workspace
                    .view(view)
                    .map_or(&[][..], |view| view.visible.as_slice());
                database
                    .schema
                    .properties
                    .iter()
                    .fold(menu, |menu, property| {
                        let id = property.id;
                        let toolbar = toolbar.clone();
                        menu.item(
                            PopupMenuItem::new(property.name.clone())
                                .checked(shown.contains(&id))
                                .on_click(move |_, window, cx| {
                                    toolbar.update(cx, |this, cx| {
                                        this.toggle_property(id, window, cx);
                                    });
                                }),
                        )
                    })
            })
        };
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
            .child(properties_button)
            .child(
                Button::new("toolbar-new")
                    .primary()
                    .small()
                    .child(icon_label(IconName::Plus, "New"))
                    .on_click(cx.listener(|this, _, window, cx| this.add_row(window, cx))),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Entity, TestAppContext};
    use remindr_core::domain::{PropertyId, ViewId};

    use crate::app::WorkspaceModel;
    use crate::app::testing::model_with;
    use crate::database::ids::header_id;
    use crate::database::testing::{Tasks, UiExt as _, open_block, tasks};

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn visible(model: &Model, cx: &mut TestAppContext, view: ViewId) -> Vec<PropertyId> {
        model.read_with(cx, |model, _| {
            model.workspace().view(view).unwrap().visible.clone()
        })
    }

    #[gpui_kit::test]
    fn hidden_properties_come_back_from_the_properties_menu(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, header_id(tasks.status));
        ui.pick_item(cx, "popup-menu", 2usize);
        assert!(!ui.exists(cx, header_id(tasks.status)));
        assert_eq!(
            visible(&model, cx, tasks.view),
            vec![tasks.title, tasks.priority]
        );

        ui.click(cx, "toolbar-properties");
        ui.pick_item(cx, "popup-menu", 1usize);
        assert_eq!(
            visible(&model, cx, tasks.view),
            vec![tasks.title, tasks.priority, tasks.status]
        );
        let priority = ui.find(cx, header_id(tasks.priority)).unwrap().bounds();
        let status = ui.find(cx, header_id(tasks.status)).unwrap().bounds();
        assert!(status.left() > priority.left());
    }

    #[gpui_kit::test]
    fn the_properties_menu_hides_and_shows_the_title(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-properties");
        ui.with(cx, |window, _| {
            use gpui_kit::test::TestWindowExt as _;
            let menu = window.within("popup-menu");
            assert_eq!(menu.find(0usize).label(), Some("Name"));
            assert_eq!(menu.find(1usize).label(), Some("Status"));
            assert_eq!(menu.find(2usize).label(), Some("Priority"));
        });
        ui.pick_item(cx, "popup-menu", 0usize);
        assert_eq!(
            visible(&model, cx, tasks.view),
            vec![tasks.status, tasks.priority]
        );
        assert!(!ui.exists(cx, header_id(tasks.title)));
        ui.click(cx, "toolbar-properties");
        ui.pick_item(cx, "popup-menu", 0usize);
        assert_eq!(
            visible(&model, cx, tasks.view),
            vec![tasks.status, tasks.priority, tasks.title]
        );
        assert!(ui.exists(cx, header_id(tasks.title)));
    }
}
