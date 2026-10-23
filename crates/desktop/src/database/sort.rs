use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{
    ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _, h_flex, v_flex,
};
use gpui_kit::{
    Context, Entity, EventEmitter, InteractiveElement as _, IntoElement, ParentElement as _,
    Render, Styled as _, Subscription, TestSupportExt as _, Window, div,
    prelude::FluentBuilder as _, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{DatabaseId, Property, PropertyId, Sort, SortDirection, ViewId};

use super::ids::{sort_direction_id, sort_property_id, sort_remove_id, sort_row_id};
use super::kinds::kind_icon;
use super::toolbar::Dismiss;
use super::ui::icon_label;
use crate::app::{WorkspaceModel, commands};
use crate::theme::UI_TEXT;

pub struct SortPanel {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    view: ViewId,
    menus_open: usize,
    _subscription: Subscription,
}

impl EventEmitter<Dismiss> for SortPanel {}

impl SortPanel {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        view: ViewId,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription = cx.subscribe(&model, |_, _, changes: &Changes, cx| {
            if !changes.views.is_empty() || !changes.databases.is_empty() {
                cx.notify();
            }
        });
        Self {
            model,
            database,
            view,
            menus_open: 0,
            _subscription: subscription,
        }
    }

    fn sorts(&self, cx: &gpui_kit::App) -> Vec<Sort> {
        self.model
            .read(cx)
            .workspace()
            .view(self.view)
            .map(|view| view.sorts.clone())
            .unwrap_or_default()
    }

    fn schema(&self, cx: &gpui_kit::App) -> Vec<Property> {
        self.model
            .read(cx)
            .workspace()
            .database(self.database)
            .map(|database| database.schema.properties.clone())
            .unwrap_or_default()
    }

    fn store(&mut self, sorts: Vec<Sort>, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(
            &self.model,
            Command::SetSorts {
                view: self.view,
                sorts,
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn add_sort(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut sorts = self.sorts(cx);
        let free = self
            .schema(cx)
            .into_iter()
            .find(|property| sorts.iter().all(|sort| sort.property != property.id));
        let Some(property) = free else {
            return;
        };
        sorts.push(Sort {
            property: property.id,
            direction: SortDirection::Ascending,
        });
        self.store(sorts, window, cx);
    }

    fn remove_sort(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut sorts = self.sorts(cx);
        if index < sorts.len() {
            sorts.remove(index);
            self.store(sorts, window, cx);
        }
    }

    fn set_property(
        &mut self,
        index: usize,
        property: PropertyId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut sorts = self.sorts(cx);
        if let Some(sort) = sorts.get_mut(index) {
            sort.property = property;
            self.store(sorts, window, cx);
        }
    }

    fn flip(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut sorts = self.sorts(cx);
        if let Some(sort) = sorts.get_mut(index) {
            sort.direction = match sort.direction {
                SortDirection::Ascending => SortDirection::Descending,
                SortDirection::Descending => SortDirection::Ascending,
            };
            self.store(sorts, window, cx);
        }
    }

    fn menu_toggled(&mut self, open: bool) {
        self.menus_open = if open {
            self.menus_open + 1
        } else {
            self.menus_open.saturating_sub(1)
        };
    }
}

impl Render for SortPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sorts = self.sorts(cx);
        let schema = self.schema(cx);
        let rows: Vec<_> = sorts
            .iter()
            .enumerate()
            .map(|(index, sort)| {
                let panel = cx.entity();
                let tracker = cx.entity();
                let chosen = sort.property;
                let property = schema.iter().find(|candidate| candidate.id == chosen);
                let (name, icon) = property.map_or_else(
                    || ("Unknown".to_owned(), IconName::File),
                    |property| (property.name.clone(), kind_icon(&property.kind)),
                );
                let ascending = sort.direction == SortDirection::Ascending;
                h_flex()
                    .id(sort_row_id(index))
                    .test_support()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new(sort_property_id(index))
                            .ghost()
                            .small()
                            .w(px(170.))
                            .child(
                                h_flex()
                                    .w_full()
                                    .gap_1()
                                    .items_center()
                                    .text_size(UI_TEXT)
                                    .child(Icon::new(icon).size_3())
                                    .child(div().truncate().child(name)),
                            )
                            .dropdown_menu({
                                let schema = schema.clone();
                                move |menu, _, _| {
                                    schema.iter().fold(menu, |menu, property| {
                                        let panel = panel.clone();
                                        let id = property.id;
                                        menu.item(
                                            PopupMenuItem::new(property.name.clone())
                                                .checked(id == chosen)
                                                .on_click(move |_, window, cx| {
                                                    panel.update(cx, |this, cx| {
                                                        this.set_property(index, id, window, cx);
                                                    });
                                                }),
                                        )
                                    })
                                }
                            })
                            .on_open_change(move |open, _, cx| {
                                let open = *open;
                                tracker.update(cx, |this, _| this.menu_toggled(open));
                            }),
                    )
                    .child(
                        Button::new(sort_direction_id(index))
                            .ghost()
                            .small()
                            .child(icon_label(
                                if ascending {
                                    IconName::SortAscending
                                } else {
                                    IconName::SortDescending
                                },
                                if ascending { "Ascending" } else { "Descending" },
                            ))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.flip(index, window, cx);
                            })),
                    )
                    .child(
                        Button::new(sort_remove_id(index))
                            .ghost()
                            .xsmall()
                            .child(Icon::new(IconName::Close).size_3())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_sort(index, window, cx);
                            })),
                    )
            })
            .collect();
        let empty = sorts.is_empty();
        let exhausted = sorts.len() >= schema.len();
        let muted = cx.theme().muted_foreground;
        v_flex()
            .id("sort-panel")
            .test_support()
            .w(px(420.))
            .gap_2()
            .text_size(UI_TEXT)
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.menus_open == 0 {
                    cx.emit(Dismiss);
                }
            }))
            .when(empty, |this| {
                this.child(div().text_color(muted).child("No sorts applied"))
            })
            .children(rows)
            .child(
                div().child(
                    Button::new("sort-add")
                        .ghost()
                        .small()
                        .disabled(exhausted)
                        .child(icon_label(IconName::Plus, "Add sort"))
                        .on_click(cx.listener(|this, _, window, cx| this.add_sort(window, cx))),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Entity, TestAppContext};
    use remindr_core::domain::{Sort, SortDirection, ViewId};

    use crate::app::WorkspaceModel;
    use crate::app::testing::model_with;
    use crate::database::ids::{row_id, sort_direction_id, sort_property_id, sort_remove_id};
    use crate::database::testing::{Tasks, UiExt as _, open_block, tasks, titles};

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn sorts(model: &Model, cx: &mut TestAppContext, view: ViewId) -> Vec<Sort> {
        model.read_with(cx, |model, _| {
            model.workspace().view(view).unwrap().sorts.clone()
        })
    }

    #[gpui_kit::test]
    fn sort_by_number_descending_reorders_rows(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-sort");
        ui.click(cx, "sort-add");
        assert_eq!(
            sorts(&model, cx, tasks.view),
            vec![Sort {
                property: tasks.title,
                direction: SortDirection::Ascending
            }]
        );
        ui.click(cx, sort_property_id(0));
        ui.pick_item(cx, "popup-menu", 2usize);
        ui.click(cx, sort_direction_id(0));
        assert_eq!(
            sorts(&model, cx, tasks.view),
            vec![Sort {
                property: tasks.priority,
                direction: SortDirection::Descending
            }]
        );
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Build core", "Ship UI", "Write spec", "Celebrate"]
        );
        let tops: Vec<f32> = [1usize, 2, 0, 3]
            .iter()
            .map(|index| {
                let found = ui.find(cx, row_id(tasks.rows[*index])).unwrap();
                found.bounds().origin.y.into()
            })
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
        assert_eq!(ui.label(cx, "toolbar-sort-count").as_deref(), Some("1"));

        ui.click(cx, sort_remove_id(0));
        assert!(sorts(&model, cx, tasks.view).is_empty());
        assert_eq!(
            titles(&model, cx, tasks.view),
            ["Write spec", "Build core", "Ship UI", "Celebrate"]
        );
    }

    #[gpui_kit::test]
    fn sorts_stack_in_order(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-sort");
        ui.click(cx, "sort-add");
        ui.click(cx, sort_property_id(0));
        ui.pick_item(cx, "popup-menu", 1usize);
        ui.click(cx, "sort-add");
        assert_eq!(sorts(&model, cx, tasks.view).len(), 2);
        assert_eq!(sorts(&model, cx, tasks.view)[0].property, tasks.status);
        assert!(ui.exists(cx, sort_property_id(1)));
    }
}
