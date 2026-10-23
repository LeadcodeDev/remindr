use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, ElementId, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, SharedString, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{
    BlockId, BoardGroup, DatabaseId, OptionId, Property, PropertyId, PropertyKind, PropertyValue,
    SelectOption, ViewId, ViewKind,
};

use super::changes::touches;
use super::format::{date_text, format_number};
use super::ids::{card_id, card_prop_id, group_id, new_card_id};
use super::tags::option_tag;
use super::ui::icon_label;
use crate::app::navigation::page_title;
use crate::app::{NavigatorHandle, Route, WorkspaceModel, commands};

const COLUMN_WIDTH: f32 = 272.;
const COLUMN_MAX_HEIGHT: f32 = 520.;

#[derive(Clone)]
struct CardDrag {
    row: BlockId,
    group: Option<OptionId>,
    title: SharedString,
}

struct DragPreview {
    title: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(COLUMN_WIDTH - 32.))
            .px_3()
            .py_2()
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .opacity(0.9)
            .shadow_md()
            .child(self.title.clone())
    }
}

pub struct BoardView {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    view: ViewId,
    _subscription: Subscription,
}

impl BoardView {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        view: ViewId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscription =
            cx.subscribe_in(&model, window, |this, model, changes: &Changes, _, cx| {
                if touches(model.read(cx).workspace(), changes, this.database, None) {
                    cx.notify();
                }
            });
        Self {
            model,
            database,
            view,
            _subscription: subscription,
        }
    }

    fn group_by(&self, cx: &Context<Self>) -> Option<PropertyId> {
        match self.model.read(cx).workspace().view(self.view)?.kind {
            ViewKind::Board { group_by } => Some(group_by),
            ViewKind::Table => None,
        }
    }

    fn drop_card(
        &mut self,
        drag: &CardDrag,
        group: Option<OptionId>,
        before: Option<BlockId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let sorted = self
            .model
            .read(cx)
            .workspace()
            .view(self.view)
            .is_some_and(|view| !view.sorts.is_empty());
        let same_column = drag.group == group;
        if same_column && (sorted || before == Some(drag.row)) {
            return;
        }
        commands::run(
            &self.model,
            Command::MoveCard {
                view: self.view,
                row: drag.row,
                group,
                before,
            },
            window,
            cx,
        );
    }

    fn add_card(&mut self, group: Option<OptionId>, window: &mut Window, cx: &mut Context<Self>) {
        let values = match (self.group_by(cx), group) {
            (Some(property), Some(option)) => vec![(property, PropertyValue::Select(option))],
            _ => Vec::new(),
        };
        commands::run(
            &self.model,
            Command::AddRow {
                database: self.database,
                values,
            },
            window,
            cx,
        );
    }

    fn open_page(&mut self, row: BlockId, cx: &mut Context<Self>) {
        let Some(navigator) = cx
            .try_global::<NavigatorHandle>()
            .map(|handle| handle.0.clone())
        else {
            return;
        };
        navigator.update(cx, |navigator, cx| navigator.open(Route::Page(row), cx));
    }

    fn value_view(
        property: &Property,
        value: Option<&PropertyValue>,
        cx: &Context<Self>,
    ) -> Option<(String, AnyElement)> {
        let muted = cx.theme().muted_foreground;
        let (label, element) = match (&property.kind, value?) {
            (PropertyKind::Text, PropertyValue::Text(text))
            | (PropertyKind::Url, PropertyValue::Url(text)) => (
                text.clone(),
                div()
                    .text_color(muted)
                    .child(text.clone())
                    .into_any_element(),
            ),
            (PropertyKind::Number { format }, PropertyValue::Number(number)) => {
                let text = format_number(*number, *format);
                (
                    text.clone(),
                    div().text_color(muted).child(text).into_any_element(),
                )
            }
            (PropertyKind::Date, PropertyValue::Date(date)) => {
                let text = date_text(*date);
                (
                    text.clone(),
                    div().text_color(muted).child(text).into_any_element(),
                )
            }
            (PropertyKind::Checkbox, PropertyValue::Checkbox(checked)) => (
                checked.to_string(),
                h_flex()
                    .text_color(muted)
                    .gap_1()
                    .child(Icon::new(IconName::Check).size_3().opacity(if *checked {
                        1.
                    } else {
                        0.2
                    }))
                    .into_any_element(),
            ),
            (PropertyKind::Select { options }, PropertyValue::Select(id)) => {
                let option = options.iter().find(|option| option.id == *id)?;
                (option.name.clone(), option_tag(option).into_any_element())
            }
            (PropertyKind::MultiSelect { options }, PropertyValue::MultiSelect(ids)) => {
                let chosen: Vec<&SelectOption> = ids
                    .iter()
                    .filter_map(|id| options.iter().find(|option| option.id == *id))
                    .collect();
                if chosen.is_empty() {
                    return None;
                }
                let label = chosen
                    .iter()
                    .map(|option| option.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (
                    label,
                    h_flex()
                        .gap_1()
                        .flex_wrap()
                        .children(chosen.into_iter().map(option_tag))
                        .into_any_element(),
                )
            }
            _ => return None,
        };
        Some((label, element))
    }

    fn card(
        &self,
        row: BlockId,
        group: Option<OptionId>,
        shown: &[Property],
        sorted: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let workspace = self.model.read(cx).workspace();
        let title = workspace.block(row).map(page_title).unwrap_or_default();
        let values: Vec<AnyElement> = shown
            .iter()
            .filter_map(|property| {
                let value = workspace.value(row, property.id);
                let (label, element) = Self::value_view(property, value, cx)?;
                Some(
                    div()
                        .id(card_prop_id(row, property.id))
                        .test_support()
                        .aria_label(label)
                        .text_xs()
                        .child(element)
                        .into_any_element(),
                )
            })
            .collect();
        let drag = CardDrag {
            row,
            group,
            title: title.clone().into(),
        };
        let accent = cx.theme().primary;
        let border = cx.theme().border;
        let background = cx.theme().background;
        v_flex()
            .id(card_id(row))
            .test_support()
            .aria_label(title.clone())
            .w_full()
            .gap_1()
            .px_3()
            .py_2()
            .rounded(px(6.))
            .border_1()
            .border_color(border)
            .bg(background)
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| this.open_page(row, cx)))
            .on_drag(drag, |drag, _, _, cx| {
                let title = drag.title.clone();
                cx.new(|_| DragPreview { title })
            })
            .drag_over::<CardDrag>(move |style, dragged, _, _| {
                if sorted && dragged.group == group {
                    style
                } else {
                    style.border_t_2().border_color(accent)
                }
            })
            .on_drop(cx.listener(move |this, dragged: &CardDrag, window, cx| {
                this.drop_card(dragged, group, Some(row), window, cx);
            }))
            .child(div().font_weight(gpui_kit::FontWeight::MEDIUM).child(title))
            .children(values)
            .into_any_element()
    }

    fn column(
        &self,
        group: &BoardGroup,
        property: &Property,
        shown: &[Property],
        sorted: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let options = match &property.kind {
            PropertyKind::Select { options } => options.as_slice(),
            _ => &[],
        };
        let option = group
            .option
            .and_then(|id| options.iter().find(|candidate| candidate.id == id));
        let name = option.map_or_else(
            || format!("No {}", property.name),
            |option| option.name.clone(),
        );
        let heading: AnyElement = match option {
            Some(option) => option_tag(option).into_any_element(),
            None => div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(name.clone())
                .into_any_element(),
        };
        let cards: Vec<AnyElement> = group
            .rows
            .iter()
            .map(|row| self.card(*row, group.option, shown, sorted, cx))
            .collect();
        let target = group.option;
        let accent = cx.theme().accent;
        let stripe = cx.theme().muted;
        let scroll_id = ElementId::Name(
            format!(
                "board-cards-{}",
                group
                    .option
                    .map_or_else(|| "none".to_owned(), |option| option.as_uuid().to_string(),)
            )
            .into(),
        );
        v_flex()
            .id(group_id(group.option))
            .test_support()
            .aria_label(name)
            .aria_value(group.rows.len().to_string())
            .w(px(COLUMN_WIDTH))
            .flex_none()
            .gap_2()
            .p_2()
            .rounded(px(8.))
            .bg(stripe.opacity(0.5))
            .drag_over::<CardDrag>(move |style, dragged, _, _| {
                if sorted && dragged.group == target {
                    style
                } else {
                    style.bg(accent)
                }
            })
            .on_drop(cx.listener(move |this, dragged: &CardDrag, window, cx| {
                this.drop_card(dragged, target, None, window, cx);
            }))
            .child(
                h_flex().gap_2().items_center().child(heading).child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(group.rows.len().to_string()),
                ),
            )
            .child(
                v_flex()
                    .id(scroll_id)
                    .max_h(px(COLUMN_MAX_HEIGHT))
                    .overflow_y_scroll()
                    .gap_2()
                    .children(cards),
            )
            .child(
                Button::new(new_card_id(group.option))
                    .ghost()
                    .small()
                    .w_full()
                    .child(icon_label(IconName::Plus, "New").w_full())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.add_card(target, window, cx);
                    })),
            )
            .into_any_element()
    }
}

impl Render for BoardView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.model.read(cx).workspace();
        let groups = workspace.board_groups(self.view).unwrap_or_default();
        let view = workspace.view(self.view);
        let group_by = self.group_by(cx);
        let schema = workspace
            .database(self.database)
            .map(|database| database.schema.properties.clone())
            .unwrap_or_default();
        let shown: Vec<Property> = view
            .map(|view| {
                view.visible
                    .iter()
                    .filter(|id| Some(**id) != group_by)
                    .filter_map(|id| schema.iter().find(|property| property.id == *id))
                    .filter(|property| property.kind != PropertyKind::Title)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let sorted = view.is_some_and(|view| !view.sorts.is_empty());
        let property = group_by.and_then(|id| schema.iter().find(|property| property.id == id));
        let columns: Vec<AnyElement> = match property {
            Some(property) => groups
                .iter()
                .map(|group| self.column(group, property, &shown, sorted, cx))
                .collect(),
            None => Vec::new(),
        };
        h_flex()
            .id("board")
            .test_support()
            .w_full()
            .gap_3()
            .items_start()
            .overflow_x_scroll()
            .pb_2()
            .children(columns)
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{Entity, TestAppContext};
    use remindr_core::application::Command;
    use remindr_core::domain::{BlockId, PropertyValue, Sort, SortDirection, ViewId, ViewKind};

    use crate::app::testing::{Ui, execute, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::database::ids::{card_id, card_prop_id, group_id, new_card_id};
    use crate::database::testing::{Tasks, UiExt as _, add_row, open_block, tasks, value_of};

    type Model = Entity<WorkspaceModel>;

    struct Board {
        tasks: Tasks,
        loose: BlockId,
        board: ViewId,
    }

    fn board_model(cx: &mut TestAppContext) -> (Model, Board) {
        let mut seeded = None;
        let model = model_with(cx, |session| {
            let made = tasks(session);
            let loose = add_row(session, made.database, "Inbox item", Vec::new());
            seeded = Some((made, loose));
        });
        let (made, loose) = seeded.unwrap();
        execute(
            &model,
            cx,
            Command::AddView {
                block: made.block,
                name: "Board".to_owned(),
                kind: ViewKind::Board {
                    group_by: made.status,
                },
            },
        );
        let board = model.read_with(cx, |model, _| {
            model.workspace().views(made.block).nth(1).unwrap().id
        });
        (
            model,
            Board {
                tasks: made,
                loose,
                board,
            },
        )
    }

    fn show_board(cx: &mut TestAppContext, model: &Model, board: &Board) -> Ui {
        let ui = open_block(cx, model, board.tasks.block, board.tasks.database);
        ui.with(cx, |window, cx| {
            window.within("view-tabs").click(1usize, cx)
        });
        ui.settle(cx);
        ui
    }

    fn group_rows(model: &Model, cx: &mut TestAppContext, board: &Board) -> Vec<Vec<BlockId>> {
        model.read_with(cx, |model, _| {
            model
                .workspace()
                .board_groups(board.board)
                .unwrap()
                .into_iter()
                .map(|group| group.rows)
                .collect()
        })
    }

    #[gpui_kit::test]
    fn board_shows_the_empty_group_first_then_options(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        let tasks = &board.tasks;
        let groups = [None, Some(tasks.todo), Some(tasks.doing), Some(tasks.done)];
        let lefts: Vec<f32> = groups
            .iter()
            .map(|group| {
                let found = ui.find(cx, group_id(*group)).unwrap();
                found.bounds().origin.x.into()
            })
            .collect();
        assert!(lefts.windows(2).all(|pair| pair[0] < pair[1]), "{lefts:?}");
        let titles: Vec<Option<String>> = groups
            .iter()
            .map(|group| ui.label(cx, group_id(*group)))
            .collect();
        assert_eq!(
            titles,
            [
                Some("No Status".to_owned()),
                Some("Todo".to_owned()),
                Some("Doing".to_owned()),
                Some("Done".to_owned())
            ]
        );
        let counts: Vec<Option<String>> = groups
            .iter()
            .map(|group| {
                ui.find(cx, group_id(*group))
                    .and_then(|found| found.value().map(str::to_owned))
            })
            .collect();
        assert_eq!(
            counts,
            [
                Some("1".to_owned()),
                Some("1".to_owned()),
                Some("1".to_owned()),
                Some("2".to_owned())
            ]
        );
        assert_eq!(
            ui.label(cx, card_id(board.loose)).as_deref(),
            Some("Inbox item")
        );
        let done_tops: Vec<f32> = [tasks.rows[2], tasks.rows[3]]
            .iter()
            .map(|row| {
                let found = ui.find(cx, card_id(*row)).unwrap();
                found.bounds().origin.y.into()
            })
            .collect();
        assert!(done_tops[0] < done_tops[1]);
    }

    #[gpui_kit::test]
    fn cards_show_the_other_visible_properties_and_open_their_page(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        let tasks = &board.tasks;
        assert_eq!(
            ui.label(cx, card_prop_id(tasks.rows[1], tasks.priority))
                .as_deref(),
            Some("8")
        );
        assert!(!ui.exists(cx, card_prop_id(tasks.rows[1], tasks.status)));
        ui.click(cx, card_id(tasks.rows[1]));
        assert_eq!(ui.route(cx), Some(Route::Page(tasks.rows[1])));
    }

    #[gpui_kit::test]
    fn dragging_a_card_to_done_sets_its_value(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        let tasks = &board.tasks;
        ui.drag_to(cx, card_id(tasks.rows[0]), group_id(Some(tasks.done)));
        assert_eq!(
            value_of(&model, cx, tasks.rows[0], tasks.status),
            Some(PropertyValue::Select(tasks.done))
        );
        let groups = group_rows(&model, cx, &board);
        assert!(groups[3].contains(&tasks.rows[0]));
        assert_eq!(groups[3].len(), 3);
        assert!(groups[1].is_empty());
        assert!(!ui.exists(cx, "notification"));
    }

    #[gpui_kit::test]
    fn dropping_on_a_card_places_the_dragged_card_before_it(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        let tasks = &board.tasks;
        ui.drag_to(cx, card_id(tasks.rows[3]), card_id(tasks.rows[2]));
        let groups = group_rows(&model, cx, &board);
        assert_eq!(groups[3], vec![tasks.rows[3], tasks.rows[2]]);
    }

    #[gpui_kit::test]
    fn dropping_inside_the_same_column_with_sorts_does_nothing(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let tasks = &board.tasks;
        execute(
            &model,
            cx,
            Command::SetSorts {
                view: board.board,
                sorts: vec![Sort {
                    property: tasks.priority,
                    direction: SortDirection::Descending,
                }],
            },
        );
        let ui = show_board(cx, &model, &board);
        let before = group_rows(&model, cx, &board);
        ui.drag_to(cx, card_id(tasks.rows[3]), card_id(tasks.rows[2]));
        assert_eq!(group_rows(&model, cx, &board), before);
        assert!(!ui.exists(cx, "notification"));
        ui.drag_to(cx, card_id(tasks.rows[3]), group_id(Some(tasks.todo)));
        assert_eq!(
            value_of(&model, cx, tasks.rows[3], tasks.status),
            Some(PropertyValue::Select(tasks.todo))
        );
    }

    #[gpui_kit::test]
    fn removing_the_group_by_property_turns_the_board_into_a_table(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        assert!(ui.exists(cx, "board"));
        assert!(!ui.exists(cx, "table"));
        execute(
            &model,
            cx,
            Command::RemoveProperty {
                database: board.tasks.database,
                property: board.tasks.status,
            },
        );
        ui.settle(cx);
        assert!(!ui.exists(cx, "board"));
        assert!(ui.exists(cx, "table"));
    }

    #[gpui_kit::test]
    fn new_card_in_a_column_gets_its_value(cx: &mut TestAppContext) {
        let (model, board) = board_model(cx);
        let ui = show_board(cx, &model, &board);
        let tasks = &board.tasks;
        ui.click(cx, new_card_id(Some(tasks.doing)));
        let groups = group_rows(&model, cx, &board);
        assert_eq!(groups[2].len(), 2);
        let created = groups[2][1];
        assert_eq!(
            value_of(&model, cx, created, tasks.status),
            Some(PropertyValue::Select(tasks.doing))
        );
        assert!(ui.exists(cx, card_id(created)));
        ui.click(cx, new_card_id(None));
        let groups = group_rows(&model, cx, &board);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(value_of(&model, cx, groups[0][1], tasks.status), None);
    }
}
