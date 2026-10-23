use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::notification::Notification;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Disableable as _, Sizable as _, WindowExt as _, h_flex, v_flex};
use gpui_kit::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
    Pixels, Render, StatefulInteractiveElement as _, Styled, Subscription, TestSupportExt as _,
    Window, div, px,
};
use remindr_core::application::{Changes, Command};
use remindr_core::domain::{BlockKind, DatabaseId, Emoji, Parent, Workspace};

use super::table::{Height, Scope, TableView};
use crate::app::{WorkspaceModel, commands};
use crate::theme::{
    PAGE_ICON, PAGE_PADDING_TOP, PAGE_PADDING_X, PAGE_TITLE, PAGE_TITLE_WEIGHT, UI_TEXT,
    UI_TEXT_SMALL, labeled,
};

const TITLE_HEIGHT: Pixels = px(48.);

#[must_use]
pub fn shown_in(workspace: &Workspace, database: DatabaseId) -> usize {
    let mut pending: Vec<Parent> = vec![Parent::Root];
    pending.extend(
        workspace
            .databases()
            .map(|found| Parent::Database(found.id)),
    );
    let mut count = 0;
    while let Some(parent) = pending.pop() {
        for block in workspace.children(parent) {
            if matches!(block.kind, BlockKind::DatabaseView { database: shown } if shown == database)
            {
                count += 1;
            }
            pending.push(Parent::Block(block.id));
        }
    }
    count
}

pub struct DatabaseScreen {
    model: Entity<WorkspaceModel>,
    database: DatabaseId,
    name: Entity<InputState>,
    icon_input: Entity<InputState>,
    icon_open: bool,
    table: Entity<TableView>,
    _subscriptions: Vec<Subscription>,
}

impl DatabaseScreen {
    pub fn new(
        model: Entity<WorkspaceModel>,
        database: DatabaseId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let current = model
            .read(cx)
            .workspace()
            .database(database)
            .map(|found| found.name.clone())
            .unwrap_or_default();
        let name = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Untitled")
                .default_value(current)
        });
        let icon_input = cx.new(|cx| InputState::new(window, cx).placeholder("Any emoji"));
        let table = {
            let model = model.clone();
            cx.new(|cx| TableView::new(model, database, Scope::Database, Height::Fill, window, cx))
        };
        let subscriptions = vec![
            cx.subscribe_in(&name, window, Self::on_name_event),
            cx.subscribe_in(&icon_input, window, Self::on_icon_event),
            cx.subscribe_in(&model, window, Self::on_changes),
        ];
        Self {
            model,
            database,
            name,
            icon_input,
            icon_open: false,
            table,
            _subscriptions: subscriptions,
        }
    }

    fn stored_name(&self, cx: &Context<Self>) -> String {
        self.model
            .read(cx)
            .workspace()
            .database(self.database)
            .map(|found| found.name.clone())
            .unwrap_or_default()
    }

    fn on_name_event(
        &mut self,
        name: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let typed = name.read(cx).value().to_string();
        if typed != self.stored_name(cx) {
            commands::run(
                &self.model,
                Command::RenameDatabase {
                    database: self.database,
                    name: typed,
                },
                window,
                cx,
            );
        }
    }

    fn on_icon_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::PressEnter { .. }) {
            return;
        }
        let typed = input.read(cx).value().trim().to_owned();
        if !typed.is_empty() {
            self.choose_icon(Some(&typed), window, cx);
        }
    }

    fn on_changes(
        &mut self,
        _: &Entity<WorkspaceModel>,
        changes: &Changes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !changes.databases.contains(&self.database) && changes.blocks.is_empty() {
            return;
        }
        let stored = self.stored_name(cx);
        if self.name.read(cx).value() != stored {
            self.name
                .update(cx, |name, cx| name.set_value(stored, window, cx));
        }
        cx.notify();
    }

    fn choose_icon(&mut self, raw: Option<&str>, window: &mut Window, cx: &mut Context<Self>) {
        let icon = match raw.map(Emoji::try_from).transpose() {
            Ok(icon) => icon,
            Err(error) => {
                window.push_notification(Notification::error(error.to_string()), cx);
                return;
            }
        };
        commands::run(
            &self.model,
            Command::SetDatabaseIcon {
                database: self.database,
                icon,
            },
            window,
            cx,
        );
        self.icon_input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.icon_open = false;
        cx.notify();
    }

    fn icon_picker(&self, current: Option<String>, cx: &mut Context<Self>) -> impl IntoElement {
        let screen = cx.entity();
        let content_screen = cx.entity();
        let input = self.icon_input.clone();
        let has_icon = current.is_some();
        let muted = cx.theme().muted_foreground;
        let trigger = match &current {
            Some(emoji) => Button::new("database-icon")
                .ghost()
                .h(PAGE_ICON + px(16.))
                .px_2()
                .accessibility_label(emoji.clone())
                .child(div().text_size(PAGE_ICON).child(emoji.clone())),
            None => Button::new("database-icon")
                .ghost()
                .small()
                .text_color(muted)
                .accessibility_label("Add icon")
                .child(div().text_size(UI_TEXT_SMALL).child("Add icon")),
        };
        Popover::new("database-icon-popover")
            .open(self.icon_open)
            .on_open_change(move |open, _, cx| {
                let open = *open;
                screen.update(cx, |this, cx| {
                    this.icon_open = open;
                    cx.notify();
                });
            })
            .trigger(trigger)
            .content(move |_, _, _| {
                let remove_screen = content_screen.clone();
                v_flex()
                    .w(px(260.))
                    .gap_2()
                    .text_size(UI_TEXT)
                    .child(Input::new(&input).id("database-icon-input").small())
                    .child(
                        div().child(
                            labeled(
                                Button::new("database-icon-remove").ghost().small(),
                                "Remove",
                            )
                            .disabled(!has_icon)
                            .on_click(move |_, window, cx| {
                                remove_screen.update(cx, |this, cx| {
                                    this.choose_icon(None, window, cx);
                                });
                            }),
                        ),
                    )
            })
    }
}

impl Render for DatabaseScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = self.model.read(cx).workspace();
        let icon = workspace
            .database(self.database)
            .and_then(|found| found.icon.as_ref().map(|icon| icon.as_str().to_owned()));
        let count = shown_in(workspace, self.database);
        let shown = format!(
            "Shown in {count} {}",
            if count == 1 { "page" } else { "pages" }
        );
        let muted = cx.theme().muted_foreground;
        let background = cx.theme().background;
        let picker = self.icon_picker(icon, cx);
        let title = Input::new(&self.name)
            .id("database-name")
            .appearance(false)
            .bordered(false)
            .focus_bordered(false);
        let title = Styled::h(title, TITLE_HEIGHT)
            .px_0()
            .text_size(PAGE_TITLE)
            .font_weight(PAGE_TITLE_WEIGHT);
        v_flex()
            .id("database-screen")
            .test_support()
            .size_full()
            .bg(background)
            .text_size(UI_TEXT)
            .child(
                v_flex()
                    .w_full()
                    .flex_none()
                    .px(PAGE_PADDING_X)
                    .pt(PAGE_PADDING_TOP)
                    .pb_3()
                    .gap_1()
                    .child(h_flex().child(picker))
                    .child(title)
                    .child(
                        div()
                            .id("database-shown-in")
                            .test_support()
                            .aria_label(shown.clone())
                            .text_size(UI_TEXT_SMALL)
                            .text_color(muted)
                            .child(shown),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .px(PAGE_PADDING_X)
                    .pb_4()
                    .child(self.table.clone()),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{AppContext as _, ElementId, Entity, TestAppContext, point, px};
    use remindr_core::application::Command;
    use remindr_core::domain::BlockKind;

    use super::DatabaseScreen;
    use crate::app::testing::{Ui, add_block, execute, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::database::ids::{header_id, row_id};
    use crate::database::testing::{Tasks, UiExt as _, open, tasks};
    use crate::sidebar::database_open_id;

    type Model = Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    fn open_screen(cx: &mut TestAppContext, model: &Model, tasks: &Tasks) -> Ui {
        let shared = model.clone();
        let database = tasks.database;
        let (ui, _) = open(cx, model, move |window, cx| {
            cx.new(|cx| DatabaseScreen::new(shared, database, window, cx))
        });
        ui
    }

    #[gpui_kit::test]
    fn database_screen_lists_all_rows_and_properties(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        execute(
            &model,
            cx,
            Command::SetVisibleProperties {
                view: tasks.view,
                properties: vec![tasks.title],
            },
        );
        let ui = open_screen(cx, &model, &tasks);
        assert!(ui.exists(cx, "database-screen"));
        let lefts: Vec<f32> = [tasks.title, tasks.status, tasks.priority]
            .iter()
            .map(|property| {
                let found = ui.find(cx, header_id(*property)).unwrap();
                found.bounds().origin.x.into()
            })
            .collect();
        assert!(lefts.windows(2).all(|pair| pair[0] < pair[1]), "{lefts:?}");
        let tops: Vec<f32> = tasks
            .rows
            .iter()
            .map(|row| {
                let found = ui.find(cx, row_id(*row)).unwrap();
                found.bounds().origin.y.into()
            })
            .collect();
        assert!(tops.windows(2).all(|pair| pair[0] < pair[1]), "{tops:?}");
        assert_eq!(ui.find(cx, "database-name").unwrap().value(), Some("Tasks"));
        assert_eq!(
            ui.label(cx, "database-shown-in").as_deref(),
            Some("Shown in 1 page")
        );
        let last = ui.find(cx, row_id(tasks.rows[3])).unwrap().bounds();
        let new_row = ui.find(cx, "table-new-row").unwrap().bounds();
        assert!(new_row.origin.y >= last.bottom());
        assert!(new_row.bottom() <= px(700.));
    }

    #[gpui_kit::test]
    fn the_new_row_button_sits_right_after_the_last_row(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_screen(cx, &model, &tasks);
        let last = ui.find(cx, row_id(tasks.rows[3])).unwrap().bounds();
        let new_row = ui.find(cx, "table-new-row").unwrap().bounds();
        let gap = (new_row.top() - last.bottom()).abs();
        assert!(gap <= px(1.), "gap {gap:?}");
        assert!(new_row.bottom() < px(400.));
    }

    #[gpui_kit::test]
    fn a_long_table_scrolls_and_keeps_the_new_row_in_the_window(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        for _ in 0..40 {
            execute(
                &model,
                cx,
                Command::AddRow {
                    database: tasks.database,
                    values: Vec::new(),
                },
            );
        }
        let ui = open_screen(cx, &model, &tasks);
        let new_row = ui.find(cx, "table-new-row").unwrap().bounds();
        assert!(new_row.bottom() <= px(700.), "{new_row:?}");
        assert!(ui.find(cx, row_id(tasks.rows[0])).is_some());
        assert!(ui.find(cx, row_id(tasks.rows[3])).is_some());
    }

    #[gpui_kit::test]
    fn shown_in_counts_the_view_blocks_that_are_alive(cx: &mut TestAppContext) {
        let mut seeded = None;
        let mut extra = None;
        let model = model_with(cx, |session| {
            let made = tasks(session);
            let after = add_block(session, made.page, BlockKind::Text, "more");
            let changes = crate::database::testing::run(
                session,
                Command::InsertDatabaseView {
                    after,
                    database: made.database,
                },
            );
            extra = crate::app::commands::created_block(&changes);
            seeded = Some(made);
        });
        let (tasks, extra) = (seeded.unwrap(), extra.unwrap());
        let ui = open_screen(cx, &model, &tasks);
        assert_eq!(
            ui.label(cx, "database-shown-in").as_deref(),
            Some("Shown in 2 pages")
        );
        execute(
            &model,
            cx,
            Command::DeleteBlocks {
                blocks: vec![extra],
            },
        );
        ui.settle(cx);
        assert_eq!(
            ui.label(cx, "database-shown-in").as_deref(),
            Some("Shown in 1 page")
        );
    }

    #[gpui_kit::test]
    fn renaming_on_the_screen_updates_the_sidebar(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = Ui::open(cx, &model);
        ui.click(cx, database_open_id(tasks.database));
        assert_eq!(ui.route(cx), Some(Route::Database(tasks.database)));
        ui.click(cx, "database-name");
        ui.press(cx, "cmd-a");
        ui.input(cx, "Projects");
        let name = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .name
                .clone()
        });
        assert_eq!(name, "Projects");
        assert_eq!(
            ui.label(cx, database_open_id(tasks.database)).as_deref(),
            Some("Projects")
        );
        assert_eq!(ui.label(cx, "breadcrumb").as_deref(), Some("Projects"));
    }

    #[gpui_kit::test]
    fn the_screen_sets_and_clears_the_database_icon(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_screen(cx, &model, &tasks);
        ui.click(cx, "database-icon");
        ui.click(cx, "database-icon-input");
        ui.input(cx, "🚀");
        ui.press(cx, "enter");
        let icon = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .icon
                .as_ref()
                .map(|icon| icon.as_str().to_owned())
        });
        assert_eq!(icon.as_deref(), Some("🚀"));
        assert_eq!(ui.label(cx, "database-icon").as_deref(), Some("🚀"));
        ui.click(cx, "database-icon");
        ui.click(cx, "database-icon-remove");
        let icon = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .icon
                .clone()
        });
        assert_eq!(icon, None);
    }

    #[gpui_kit::test]
    fn resizing_a_column_on_the_screen_is_not_stored(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_screen(cx, &model, &tasks);
        let header = ui
            .find(cx, ElementId::NamedInteger("col-header".into(), 1))
            .unwrap()
            .bounds();
        let edge = header.origin.x + header.size.width;
        let middle = header.origin.y + header.size.height / 2.;
        ui.with(cx, |window, cx| {
            window.drag(
                point(edge - px(1.), middle),
                point(edge + px(80.), middle),
                cx,
            );
        });
        ui.settle(cx);
        let stored = model.read_with(cx, |model, _| {
            model
                .workspace()
                .view(tasks.view)
                .unwrap()
                .column_widths
                .clone()
        });
        assert!(stored.is_empty());
        let resized = ui
            .find(cx, ElementId::NamedInteger("col-header".into(), 1))
            .unwrap()
            .bounds();
        assert!(resized.size.width > header.size.width);
    }

    #[gpui_kit::test]
    fn moving_a_column_on_the_screen_moves_the_property(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_screen(cx, &model, &tasks);
        ui.drag_to(cx, header_id(tasks.priority), header_id(tasks.title));
        let order = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(tasks.database)
                .unwrap()
                .schema
                .properties
                .iter()
                .map(|property| property.id)
                .collect::<Vec<_>>()
        });
        assert_eq!(order, vec![tasks.priority, tasks.title, tasks.status]);
    }
}
