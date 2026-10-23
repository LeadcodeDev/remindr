use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::calendar::{Calendar, CalendarEvent, CalendarState};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, EventEmitter, Focusable as _, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div, prelude::FluentBuilder as _, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{
    CalendarDate, Condition, Filter, OptionId, Property, PropertyKind, ViewId,
};

use super::conditions::{
    Operand, Operator, default_condition, operand_of, operator_of, operators, with_date,
    with_number, with_operator, with_option, with_text,
};
use super::format::{Parsed, date_text, from_naive, parse_number, plain_number, to_naive};
use super::ids::{
    filter_condition_id, filter_property_id, filter_remove_id, filter_row_id, filter_value_id,
};
use super::toolbar::Dismiss;
use super::ui::icon_label;
use crate::app::{WorkspaceModel, commands};

struct RowInput {
    input: Entity<InputState>,
    invalid: bool,
    _subscription: Subscription,
}

struct DateEdit {
    row: usize,
    calendar: Entity<CalendarState>,
    _subscription: Subscription,
}

pub struct FilterPanel {
    model: Entity<WorkspaceModel>,
    database: remindr_core::domain::DatabaseId,
    view: ViewId,
    inputs: Vec<Option<RowInput>>,
    date: Option<DateEdit>,
    menus_open: usize,
    _subscription: Subscription,
}

impl EventEmitter<Dismiss> for FilterPanel {}

fn options_of(kind: &PropertyKind) -> &[remindr_core::domain::SelectOption] {
    match kind {
        PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => options,
        _ => &[],
    }
}

fn operand_text(condition: &Condition) -> Option<String> {
    match operand_of(condition) {
        Operand::Text(text) => Some(text.to_owned()),
        Operand::Number(number) => Some(plain_number(number)),
        _ => None,
    }
}

impl FilterPanel {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: remindr_core::domain::DatabaseId,
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
            inputs: Vec::new(),
            date: None,
            menus_open: 0,
            _subscription: subscription,
        }
    }

    fn filters(&self, cx: &gpui_kit::App) -> Vec<Filter> {
        self.model
            .read(cx)
            .workspace()
            .view(self.view)
            .map(|view| view.filters.clone())
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

    fn store(&mut self, filters: Vec<Filter>, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(
            &self.model,
            Command::SetFilters {
                view: self.view,
                filters,
            },
            window,
            cx,
        );
        cx.notify();
    }

    fn replace(
        &mut self,
        index: usize,
        condition: Option<(remindr_core::domain::PropertyId, Condition)>,
        reset_input: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut filters = self.filters(cx);
        let (Some(slot), Some((property, condition))) = (filters.get_mut(index), condition) else {
            return;
        };
        let next = Filter {
            property,
            condition,
        };
        if *slot == next {
            return;
        }
        *slot = next;
        if reset_input && let Some(input) = self.inputs.get_mut(index) {
            *input = None;
        }
        self.store(filters, window, cx);
    }

    fn add_filter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(title) = self
            .schema(cx)
            .into_iter()
            .find(|property| property.kind == PropertyKind::Title)
        else {
            return;
        };
        let mut filters = self.filters(cx);
        filters.push(Filter {
            property: title.id,
            condition: default_condition(&title.kind),
        });
        self.store(filters, window, cx);
    }

    fn remove_filter(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let mut filters = self.filters(cx);
        if index >= filters.len() {
            return;
        }
        filters.remove(index);
        if index < self.inputs.len() {
            self.inputs.remove(index);
        }
        self.date = None;
        self.store(filters, window, cx);
    }

    fn set_property(
        &mut self,
        index: usize,
        property: remindr_core::domain::PropertyId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(found) = self
            .schema(cx)
            .into_iter()
            .find(|candidate| candidate.id == property)
        else {
            return;
        };
        let condition = Some((property, default_condition(&found.kind)));
        self.replace(index, condition, true, window, cx);
    }

    fn set_operator(
        &mut self,
        index: usize,
        operator: Operator,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filters = self.filters(cx);
        let Some(filter) = filters.get(index) else {
            return;
        };
        let Some(property) = self
            .schema(cx)
            .into_iter()
            .find(|candidate| candidate.id == filter.property)
        else {
            return;
        };
        let condition = with_operator(&property.kind, &filter.condition, operator)
            .map(|condition| (filter.property, condition));
        self.replace(index, condition, true, window, cx);
    }

    fn set_option(
        &mut self,
        index: usize,
        option: OptionId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filters = self.filters(cx);
        let Some(filter) = filters.get(index) else {
            return;
        };
        let condition =
            with_option(&filter.condition, option).map(|condition| (filter.property, condition));
        self.replace(index, condition, false, window, cx);
    }

    fn set_date(
        &mut self,
        index: usize,
        date: CalendarDate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let filters = self.filters(cx);
        let Some(filter) = filters.get(index) else {
            return;
        };
        let condition =
            with_date(&filter.condition, date).map(|condition| (filter.property, condition));
        self.replace(index, condition, false, window, cx);
    }

    fn menu_toggled(&mut self, open: bool) {
        self.menus_open = if open {
            self.menus_open + 1
        } else {
            self.menus_open.saturating_sub(1)
        };
    }

    fn toggle_date(
        &mut self,
        index: usize,
        open: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !open {
            if self.date.as_ref().is_some_and(|edit| edit.row == index) {
                self.date = None;
            }
            cx.notify();
            return;
        }
        let current =
            self.filters(cx)
                .get(index)
                .and_then(|filter| match operand_of(&filter.condition) {
                    Operand::Date(date) => to_naive(date),
                    _ => None,
                });
        let calendar = cx.new(|cx| {
            let mut state = CalendarState::new(window, cx);
            if let Some(date) = current {
                state.set_date(date, window, cx);
            }
            state
        });
        let subscription = cx.subscribe_in(
            &calendar,
            window,
            move |this, _, event: &CalendarEvent, window, cx| {
                let CalendarEvent::Selected(date) = event;
                if let Some(chosen) = date.start().and_then(from_naive) {
                    this.set_date(index, chosen, window, cx);
                    this.date = None;
                    cx.notify();
                }
            },
        );
        self.date = Some(DateEdit {
            row: index,
            calendar,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn on_value_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let Some(index) = self
            .inputs
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|row| &row.input == input))
        else {
            return;
        };
        let typed = input.read(cx).value().to_string();
        let filters = self.filters(cx);
        let Some(filter) = filters.get(index) else {
            return;
        };
        let (condition, invalid) = match operand_of(&filter.condition) {
            Operand::Text(_) => (with_text(&filter.condition, &typed), false),
            Operand::Number(_) => match parse_number(&typed) {
                Parsed::Number(number) => (with_number(&filter.condition, number), false),
                Parsed::Empty | Parsed::Invalid => (None, true),
            },
            _ => return,
        };
        if let Some(Some(row)) = self.inputs.get_mut(index) {
            row.invalid = invalid;
        }
        let condition = condition.map(|condition| (filter.property, condition));
        self.replace(index, condition, false, window, cx);
        cx.notify();
    }

    fn sync_inputs(&mut self, filters: &[Filter], window: &mut Window, cx: &mut Context<Self>) {
        self.inputs.resize_with(filters.len(), || None);
        for (index, filter) in filters.iter().enumerate() {
            let wanted = operand_text(&filter.condition);
            match (wanted, self.inputs[index].as_ref()) {
                (None, Some(_)) => self.inputs[index] = None,
                (Some(text), None) => {
                    let input = cx.new(|cx| InputState::new(window, cx).default_value(text));
                    let subscription = cx.subscribe_in(&input, window, Self::on_value_event);
                    self.inputs[index] = Some(RowInput {
                        input,
                        invalid: false,
                        _subscription: subscription,
                    });
                }
                (Some(text), Some(row)) => {
                    let focused = row.input.read(cx).focus_handle(cx).is_focused(window);
                    if !focused && row.input.read(cx).value() != text {
                        row.input
                            .update(cx, |input, cx| input.set_value(text, window, cx));
                    }
                }
                (None, None) => {}
            }
        }
    }

    fn tracker(
        &self,
        cx: &Context<Self>,
    ) -> impl Fn(&bool, &mut Window, &mut gpui_kit::App) + 'static {
        let panel = cx.entity();
        move |open, _, cx| {
            let open = *open;
            panel.update(cx, |this, _| this.menu_toggled(open));
        }
    }

    fn property_picker(
        &self,
        index: usize,
        current: &Property,
        cx: &Context<Self>,
    ) -> gpui_kit::AnyElement {
        let panel = cx.entity();
        let chosen = current.id;
        Button::new(filter_property_id(index))
            .ghost()
            .small()
            .w(px(150.))
            .child(
                h_flex()
                    .w_full()
                    .gap_1()
                    .items_center()
                    .child(Icon::new(super::kinds::kind_icon(&current.kind)).size_3())
                    .child(div().truncate().child(current.name.clone())),
            )
            .dropdown_menu(move |menu, _, cx| {
                let properties = panel.read(cx).schema(cx);
                properties.into_iter().fold(menu, |menu, property| {
                    let panel = panel.clone();
                    let id = property.id;
                    menu.item(
                        PopupMenuItem::new(property.name.clone())
                            .checked(id == chosen)
                            .on_click(move |_, window, cx| {
                                panel.update(cx, |this, cx| {
                                    this.set_property(index, id, window, cx)
                                });
                            }),
                    )
                })
            })
            .on_open_change(self.tracker(cx))
            .into_any_element()
    }

    fn operator_picker(
        &self,
        index: usize,
        kind: &PropertyKind,
        condition: &Condition,
        cx: &Context<Self>,
    ) -> gpui_kit::AnyElement {
        let panel = cx.entity();
        let current = operator_of(condition);
        let choices = operators(kind);
        Button::new(filter_condition_id(index))
            .ghost()
            .small()
            .w(px(130.))
            .label(current.label())
            .dropdown_menu(move |menu, _, _| {
                choices.iter().fold(menu, |menu, operator| {
                    let panel = panel.clone();
                    let operator = *operator;
                    menu.item(
                        PopupMenuItem::new(operator.label())
                            .checked(operator == current)
                            .on_click(move |_, window, cx| {
                                panel.update(cx, |this, cx| {
                                    this.set_operator(index, operator, window, cx);
                                });
                            }),
                    )
                })
            })
            .on_open_change(self.tracker(cx))
            .into_any_element()
    }

    fn value_editor(
        &self,
        index: usize,
        property: &Property,
        condition: &Condition,
        cx: &Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        match operand_of(condition) {
            Operand::None => None,
            Operand::Text(_) | Operand::Number(_) => {
                let row = self.inputs.get(index)?.as_ref()?;
                let danger = cx.theme().danger;
                Some(
                    div()
                        .id(filter_value_id(index))
                        .test_support()
                        .flex_1()
                        .min_w_0()
                        .rounded(px(4.))
                        .when(row.invalid, |this| {
                            this.aria_value("invalid").border_1().border_color(danger)
                        })
                        .child(Input::new(&row.input).small())
                        .into_any_element(),
                )
            }
            Operand::Option(chosen) => {
                let panel = cx.entity();
                let options = options_of(&property.kind).to_vec();
                let label = options
                    .iter()
                    .find(|option| option.id == chosen)
                    .map_or_else(|| "Select…".to_owned(), |option| option.name.clone());
                Some(
                    Button::new(filter_value_id(index))
                        .ghost()
                        .small()
                        .w(px(140.))
                        .label(label)
                        .dropdown_menu(move |menu, _, _| {
                            options.iter().fold(menu, |menu, option| {
                                let panel = panel.clone();
                                let id = option.id;
                                menu.item(
                                    PopupMenuItem::new(option.name.clone())
                                        .checked(id == chosen)
                                        .on_click(move |_, window, cx| {
                                            panel.update(cx, |this, cx| {
                                                this.set_option(index, id, window, cx);
                                            });
                                        }),
                                )
                            })
                        })
                        .on_open_change(self.tracker(cx))
                        .into_any_element(),
                )
            }
            Operand::Date(date) => {
                let open_panel = cx.entity();
                let calendar = self
                    .date
                    .as_ref()
                    .filter(|edit| edit.row == index)
                    .map(|edit| edit.calendar.clone());
                let open = calendar.is_some();
                let mut popover = Popover::new(("filter-date-popover", index))
                    .open(open)
                    .on_open_change(move |open, window, cx| {
                        let open = *open;
                        open_panel.update(cx, |this, cx| this.toggle_date(index, open, window, cx));
                    })
                    .trigger(
                        Button::new(filter_value_id(index))
                            .ghost()
                            .small()
                            .w(px(140.))
                            .label(date_text(date)),
                    );
                if let Some(calendar) = calendar {
                    popover = popover.content(move |_, _, _| Calendar::new(&calendar));
                }
                Some(popover.into_any_element())
            }
        }
    }
}

impl Render for FilterPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filters = self.filters(cx);
        self.sync_inputs(&filters, window, cx);
        let schema = self.schema(cx);
        let rows: Vec<_> = filters
            .iter()
            .enumerate()
            .map(|(index, filter)| {
                let property = schema
                    .iter()
                    .find(|candidate| candidate.id == filter.property);
                let row = h_flex()
                    .id(filter_row_id(index))
                    .test_support()
                    .gap_2()
                    .items_center();
                let Some(property) = property else {
                    return row.child("Unknown property");
                };
                row.child(self.property_picker(index, property, cx))
                    .child(self.operator_picker(index, &property.kind, &filter.condition, cx))
                    .children(self.value_editor(index, property, &filter.condition, cx))
                    .child(
                        Button::new(filter_remove_id(index))
                            .ghost()
                            .xsmall()
                            .child(Icon::new(IconName::Close).size_3())
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_filter(index, window, cx);
                            })),
                    )
            })
            .collect();
        let empty = filters.is_empty();
        let muted = cx.theme().muted_foreground;
        v_flex()
            .id("filter-panel")
            .test_support()
            .w(px(560.))
            .gap_2()
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                if this.menus_open == 0 && this.date.is_none() {
                    cx.emit(Dismiss);
                }
            }))
            .when(empty, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(muted)
                        .child("No filters applied"),
                )
            })
            .children(rows)
            .child(
                div().child(
                    Button::new("filter-add")
                        .ghost()
                        .small()
                        .child(icon_label(IconName::Plus, "Add filter"))
                        .on_click(cx.listener(|this, _, window, cx| this.add_filter(window, cx))),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{Entity, TestAppContext};
    use remindr_core::domain::{
        CalendarDate, Condition, DateCondition, Filter, MultiSelectCondition, NumberCondition,
        OptionColor, PropertyKind, PropertyValue, SelectCondition, TextCondition, ViewId,
    };

    use crate::app::WorkspaceModel;
    use crate::app::testing::{execute, model_with};
    use crate::database::ids::{
        block_id, filter_condition_id, filter_property_id, filter_remove_id, filter_value_id,
        row_id,
    };
    use crate::database::testing::{
        Tasks, UiExt as _, add_option, add_property, open_block, run, tasks, titles,
    };
    use remindr_core::application::Command;

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn filters(model: &Model, cx: &mut TestAppContext, view: ViewId) -> Vec<Filter> {
        model.read_with(cx, |model, _| {
            model.workspace().view(view).unwrap().filters.clone()
        })
    }

    #[gpui_kit::test]
    fn filter_status_is_done_shows_only_done_rows(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-filter");
        ui.click(cx, "filter-add");
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: tasks.title,
                condition: Condition::Text(TextCondition::Contains(String::new())),
            }]
        );
        ui.click(cx, filter_property_id(0));
        ui.pick_item(cx, "popup-menu", 1usize);
        ui.click(cx, filter_condition_id(0));
        ui.pick_item(cx, "popup-menu", 0usize);
        ui.click(cx, filter_value_id(0));
        ui.pick_item(cx, "popup-menu", 2usize);
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: tasks.status,
                condition: Condition::Select(SelectCondition::Is(tasks.done)),
            }]
        );
        assert_eq!(titles(&model, cx, tasks.view), ["Ship UI", "Celebrate"]);
        assert!(!ui.exists(cx, row_id(tasks.rows[0])));
        assert!(ui.exists(cx, row_id(tasks.rows[2])));
        assert!(ui.exists(cx, row_id(tasks.rows[3])));
        assert_eq!(ui.label(cx, "toolbar-filter-count").as_deref(), Some("1"));

        ui.click(cx, filter_remove_id(0));
        assert!(filters(&model, cx, tasks.view).is_empty());
        assert!(ui.exists(cx, row_id(tasks.rows[0])));
        assert!(!ui.exists(cx, "toolbar-filter-count"));
    }

    #[gpui_kit::test]
    fn a_text_filter_narrows_the_rows_as_you_type(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-filter");
        ui.click(cx, "filter-add");
        ui.click(cx, filter_value_id(0));
        ui.input(cx, "ship");
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: tasks.title,
                condition: Condition::Text(TextCondition::Contains("ship".to_owned())),
            }]
        );
        assert_eq!(titles(&model, cx, tasks.view), ["Ship UI"]);
    }

    #[gpui_kit::test]
    fn an_invalid_number_filter_keeps_the_previous_filter(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-filter");
        ui.click(cx, "filter-add");
        ui.click(cx, filter_property_id(0));
        ui.pick_item(cx, "popup-menu", 2usize);
        ui.click(cx, filter_condition_id(0));
        ui.pick_item(cx, "popup-menu", 4usize);
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: tasks.priority,
                condition: Condition::Number(NumberCondition::Gt(0.0)),
            }]
        );
        ui.click(cx, filter_value_id(0));
        ui.press(cx, "cmd-a");
        ui.input(cx, "4");
        assert_eq!(
            filters(&model, cx, tasks.view)[0].condition,
            Condition::Number(NumberCondition::Gt(4.0))
        );
        assert_eq!(titles(&model, cx, tasks.view), ["Build core", "Ship UI"]);
        ui.input(cx, "x");
        assert_eq!(
            filters(&model, cx, tasks.view)[0].condition,
            Condition::Number(NumberCondition::Gt(4.0))
        );
        assert_eq!(
            ui.find(cx, filter_value_id(0)).unwrap().value(),
            Some("invalid")
        );
    }

    #[gpui_kit::test]
    fn the_filter_panel_closes_on_outside_clicks_and_escape(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(!ui.exists(cx, "filter-panel"));
        ui.click(cx, "toolbar-filter");
        assert!(ui.exists(cx, "filter-panel"));
        ui.click(cx, "database-header");
        assert!(!ui.exists(cx, "filter-panel"));
        ui.click(cx, "toolbar-filter");
        assert!(ui.exists(cx, "filter-panel"));
        ui.press(cx, "escape");
        assert!(!ui.exists(cx, "filter-panel"));
        ui.click(cx, "toolbar-filter");
        ui.click(cx, "toolbar-sort");
        assert!(!ui.exists(cx, "filter-panel"));
        assert!(ui.exists(cx, "sort-panel"));
    }

    #[gpui_kit::test]
    fn date_and_multi_select_filters_pick_their_values(cx: &mut TestAppContext) {
        let mut seeded = None;
        let model = model_with(cx, |session| {
            let made = tasks(session);
            let tags = add_property(
                session,
                made.database,
                "Tags",
                PropertyKind::MultiSelect {
                    options: Vec::new(),
                },
            );
            let red = add_option(session, made.database, tags, "Red", OptionColor::Red);
            let blue = add_option(session, made.database, tags, "Blue", OptionColor::Blue);
            let due = add_property(session, made.database, "Due", PropertyKind::Date);
            run(
                session,
                Command::SetValue {
                    row: made.rows[0],
                    property: tags,
                    value: Some(PropertyValue::MultiSelect(vec![blue])),
                },
            );
            run(
                session,
                Command::SetValue {
                    row: made.rows[1],
                    property: due,
                    value: Some(PropertyValue::Date(CalendarDate::new(2026, 9, 15).unwrap())),
                },
            );
            seeded = Some((made, tags, red, blue, due));
        });
        let (tasks, tags, _, blue, due) = seeded.unwrap();
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "toolbar-filter");
        ui.click(cx, "filter-add");
        ui.click(cx, filter_property_id(0));
        ui.pick_item(cx, "popup-menu", 3usize);
        ui.click(cx, filter_condition_id(0));
        ui.pick_item(cx, "popup-menu", 0usize);
        ui.click(cx, filter_value_id(0));
        ui.pick_item(cx, "popup-menu", 1usize);
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: tags,
                condition: Condition::MultiSelect(MultiSelectCondition::Contains(blue)),
            }]
        );
        assert_eq!(titles(&model, cx, tasks.view), ["Write spec"]);

        execute(
            &model,
            cx,
            Command::SetFilters {
                view: tasks.view,
                filters: vec![Filter {
                    property: due,
                    condition: Condition::Date(DateCondition::Is(
                        CalendarDate::new(2026, 9, 15).unwrap(),
                    )),
                }],
            },
        );
        ui.settle(cx);
        ui.click(cx, filter_condition_id(0));
        ui.pick_item(cx, "popup-menu", 2usize);
        ui.click(cx, filter_value_id(0));
        ui.click(cx, "calendar-2026-09-16-0-2");
        assert_eq!(
            filters(&model, cx, tasks.view),
            vec![Filter {
                property: due,
                condition: Condition::Date(DateCondition::After(
                    CalendarDate::new(2026, 9, 16).unwrap()
                )),
            }]
        );
    }

    #[gpui_kit::test]
    fn two_view_blocks_keep_separate_filters(cx: &mut TestAppContext) {
        let mut seeded = None;
        let mut second = None;
        let model = model_with(cx, |session| {
            let made = tasks(session);
            let changes = crate::database::testing::run(
                session,
                Command::InsertDatabaseView {
                    after: made.block,
                    database: made.database,
                },
            );
            second = Some(crate::app::commands::created_block(&changes).unwrap());
            seeded = Some(made);
        });
        let (tasks, second) = (seeded.unwrap(), second.unwrap());
        let view_b = model.read_with(cx, |model, _| {
            model.workspace().views(second).next().unwrap().id
        });
        let ui = crate::database::testing::open_blocks(
            cx,
            &model,
            &[(tasks.block, tasks.database), (second, tasks.database)],
        );
        ui.with(cx, |window, cx| {
            window
                .within(block_id(tasks.block))
                .click("toolbar-filter", cx);
        });
        ui.settle(cx);
        ui.with(cx, |window, cx| {
            window.within(block_id(tasks.block)).click("filter-add", cx);
        });
        ui.settle(cx);
        ui.with(cx, |window, cx| {
            window
                .within(block_id(tasks.block))
                .click(filter_value_id(0), cx);
        });
        ui.input(cx, "spec");
        assert_eq!(titles(&model, cx, tasks.view), ["Write spec"]);
        assert_eq!(titles(&model, cx, view_b).len(), 4);
        assert!(filters(&model, cx, view_b).is_empty());
        ui.with(cx, |window, _| {
            let first = window.within(block_id(tasks.block));
            assert!(first.try_find(row_id(tasks.rows[0])).is_some());
            assert!(first.try_find(row_id(tasks.rows[1])).is_none());
            let other = window.within(block_id(second));
            assert!(other.try_find(row_id(tasks.rows[1])).is_some());
        });
    }
}
