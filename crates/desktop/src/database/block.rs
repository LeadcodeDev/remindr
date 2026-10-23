use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Escape, Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, Sizable as _, h_flex, v_flex};
use gpui_kit::{
    AnyElement, AnyView, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _, Subscription,
    TestSupportExt as _, Window, div, prelude::FluentBuilder as _, px,
};
use remindr_core::application::{Changes, Command, Created};
use remindr_core::domain::{
    BlockId, DatabaseId, PropertyId, PropertyKind, ViewId, ViewKind, Workspace,
};

use super::board::BoardView;
use super::ids::{block_id, tab_label_id};
use super::table::{Height, Scope, TableView};
use super::toolbar::Toolbar;
use crate::app::navigation::database_name;
use crate::app::{NavigatorHandle, Route, WorkspaceModel, commands};

pub struct DatabaseViewBlock {
    model: Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
    selected: Option<ViewId>,
    shown: Option<Shown>,
    renaming: Option<Renaming>,
    _subscriptions: Vec<Subscription>,
}

struct Shown {
    view: ViewId,
    board: bool,
    content: AnyView,
    toolbar: Entity<Toolbar>,
}

struct Renaming {
    view: ViewId,
    input: Entity<InputState>,
    _subscription: Subscription,
}

#[must_use]
pub fn resolve_view(
    workspace: &Workspace,
    block: BlockId,
    selected: Option<ViewId>,
) -> Option<(ViewId, ViewKind)> {
    let mut views = workspace.views(block);
    let first = views.next()?;
    let chosen = selected
        .and_then(|id| workspace.view(id))
        .filter(|view| view.block == block)
        .unwrap_or(first);
    Some((chosen.id, chosen.kind))
}

#[must_use]
pub fn first_select_property(workspace: &Workspace, database: DatabaseId) -> Option<PropertyId> {
    workspace
        .database(database)?
        .schema
        .properties
        .iter()
        .find(|property| matches!(property.kind, PropertyKind::Select { .. }))
        .map(|property| property.id)
}

impl DatabaseViewBlock {
    pub fn new(
        model: Entity<WorkspaceModel>,
        block: BlockId,
        database: DatabaseId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![cx.subscribe_in(&model, window, Self::on_changes)];
        let mut this = Self {
            model,
            block,
            database,
            selected: None,
            shown: None,
            renaming: None,
            _subscriptions: subscriptions,
        };
        this.sync(window, cx);
        this
    }

    fn alive(&self, cx: &Context<Self>) -> bool {
        self.model
            .read(cx)
            .workspace()
            .database(self.database)
            .is_some_and(|database| database.alive)
    }

    fn on_changes(
        &mut self,
        _: &Entity<WorkspaceModel>,
        changes: &Changes,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let relevant = changes.databases.contains(&self.database)
            || !changes.views.is_empty()
            || changes.blocks.contains(&self.block);
        if relevant {
            self.sync(window, cx);
            cx.notify();
        }
    }

    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current = resolve_view(self.model.read(cx).workspace(), self.block, self.selected);
        self.selected = current.map(|(view, _)| view);
        let Some((view, kind)) = current else {
            self.shown = None;
            return;
        };
        let board = matches!(kind, ViewKind::Board { .. });
        let unchanged = self
            .shown
            .as_ref()
            .is_some_and(|shown| shown.view == view && shown.board == board);
        if unchanged {
            return;
        }
        let model = self.model.clone();
        let database = self.database;
        let toolbar = cx.new(|cx| Toolbar::new(model, database, view, window, cx));
        let content: AnyView = if board {
            let model = self.model.clone();
            cx.new(|cx| BoardView::new(model, database, view, window, cx))
                .into()
        } else {
            let model = self.model.clone();
            cx.new(|cx| {
                TableView::new(model, database, Scope::View(view), Height::Rows, window, cx)
            })
            .into()
        };
        self.shown = Some(Shown {
            view,
            board,
            content,
            toolbar,
        });
    }

    fn select_index(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let view = self
            .model
            .read(cx)
            .workspace()
            .views(self.block)
            .nth(index)
            .map(|view| view.id);
        if view.is_some() {
            self.selected = view;
            self.sync(window, cx);
            cx.notify();
        }
    }

    fn add_view(
        &mut self,
        kind: ViewKind,
        name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let command = Command::AddView {
            block: self.block,
            name: name.to_owned(),
            kind,
        };
        let Some(changes) = commands::run(&self.model, command, window, cx) else {
            return;
        };
        let created = changes.created.iter().find_map(|created| match created {
            Created::View(view) => Some(*view),
            _ => None,
        });
        if created.is_some() {
            self.selected = created;
            self.sync(window, cx);
            cx.notify();
        }
    }

    fn begin_rename(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .model
            .read(cx)
            .workspace()
            .view(view)
            .map(|found| found.name.clone())
            .unwrap_or_default();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(name));
        let subscription = cx.subscribe_in(&input, window, Self::on_rename_event);
        input.update(cx, |input, cx| input.focus(window, cx));
        self.renaming = Some(Renaming {
            view,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn on_rename_event(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::PressEnter { .. } | InputEvent::Blur) {
            return;
        }
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let name = input.read(cx).value().trim().to_owned();
        let current = self
            .model
            .read(cx)
            .workspace()
            .view(renaming.view)
            .map(|view| view.name.clone());
        if !name.is_empty() && current.is_some_and(|current| current != name) {
            commands::run(
                &self.model,
                Command::RenameView {
                    view: renaming.view,
                    name,
                },
                window,
                cx,
            );
        }
        cx.notify();
    }

    fn remove_view(&mut self, view: ViewId, window: &mut Window, cx: &mut Context<Self>) {
        commands::run(&self.model, Command::RemoveView { view }, window, cx);
    }

    fn open_screen(&mut self, cx: &mut Context<Self>) {
        let Some(navigator) = cx
            .try_global::<NavigatorHandle>()
            .map(|handle| handle.0.clone())
        else {
            return;
        };
        let database = self.database;
        navigator.update(cx, |navigator, cx| {
            navigator.open(Route::Database(database), cx);
        });
    }

    fn header(&self, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self.model.read(cx).workspace();
        let (name, icon) = workspace
            .database(self.database)
            .map(|database| {
                (
                    database_name(database),
                    database.icon.as_ref().map(|icon| icon.as_str().to_owned()),
                )
            })
            .unwrap_or_default();
        let glyph = match icon {
            Some(emoji) => div().child(emoji).into_any_element(),
            None => Icon::new(IconName::LayoutDashboard)
                .size_4()
                .text_color(cx.theme().muted_foreground)
                .into_any_element(),
        };
        h_flex()
            .id("database-header")
            .test_support()
            .aria_label(name.clone())
            .gap_2()
            .items_center()
            .cursor_pointer()
            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
            .on_click(cx.listener(|this, _, _, cx| this.open_screen(cx)))
            .child(glyph)
            .child(name)
            .into_any_element()
    }

    fn tab_label(&self, view: ViewId, name: &str, cx: &mut Context<Self>) -> AnyElement {
        let renaming = self
            .renaming
            .as_ref()
            .filter(|renaming| renaming.view == view);
        if let Some(renaming) = renaming {
            return div()
                .w(px(120.))
                .on_action(cx.listener(|this, _: &Escape, _, cx| {
                    this.renaming = None;
                    cx.notify();
                }))
                .child(
                    Input::new(&renaming.input)
                        .id(super::ids::tab_rename_id(view))
                        .small(),
                )
                .into_any_element();
        }
        let block = cx.entity();
        div()
            .id(tab_label_id(view))
            .test_support()
            .child(name.to_owned())
            .context_menu(move |menu, _, _| {
                let renamer = block.clone();
                let remover = block.clone();
                menu.item(PopupMenuItem::new("Rename").on_click(move |_, window, cx| {
                    renamer.update(cx, |this, cx| this.begin_rename(view, window, cx));
                }))
                .item(PopupMenuItem::new("Delete").on_click(move |_, window, cx| {
                    remover.update(cx, |this, cx| this.remove_view(view, window, cx));
                }))
            })
            .into_any_element()
    }

    fn add_menu(&self, cx: &mut Context<Self>) -> AnyElement {
        let block = cx.entity();
        let model = self.model.clone();
        let database = self.database;
        Button::new("view-add")
            .ghost()
            .xsmall()
            .child(Icon::new(IconName::Plus).size_3())
            .dropdown_menu(move |menu, _, cx| {
                let group = first_select_property(model.read(cx).workspace(), database);
                let table = block.clone();
                let board = block.clone();
                let menu = menu
                    .item(PopupMenuItem::new("Table").on_click(move |_, window, cx| {
                        table.update(cx, |this, cx| {
                            this.add_view(ViewKind::Table, "Table", window, cx);
                        });
                    }))
                    .item(
                        PopupMenuItem::new("Board")
                            .disabled(group.is_none())
                            .on_click(move |_, window, cx| {
                                let Some(group_by) = group else {
                                    return;
                                };
                                board.update(cx, |this, cx| {
                                    this.add_view(
                                        ViewKind::Board { group_by },
                                        "Board",
                                        window,
                                        cx,
                                    );
                                });
                            }),
                    );
                if group.is_none() {
                    menu.item(
                        PopupMenuItem::element(|_, _| {
                            div()
                                .id("view-add-hint")
                                .test_support()
                                .text_xs()
                                .child("Add a Select property first")
                        })
                        .disabled(true),
                    )
                } else {
                    menu
                }
            })
            .into_any_element()
    }

    fn tabs(&self, shown: &Shown, cx: &mut Context<Self>) -> AnyElement {
        let workspace = self.model.read(cx).workspace();
        let views: Vec<(ViewId, String)> = workspace
            .views(self.block)
            .map(|view| (view.id, view.name.clone()))
            .collect();
        let selected = views
            .iter()
            .position(|(view, _)| *view == shown.view)
            .unwrap_or(0);
        let tabs: Vec<Tab> = views
            .iter()
            .map(|(view, name)| {
                Tab::new()
                    .aria_label(name.clone())
                    .child(self.tab_label(*view, name, cx))
            })
            .collect();
        TabBar::new("view-tabs")
            .underline()
            .selected_index(selected)
            .children(tabs)
            .suffix(self.add_menu(cx))
            .on_click(cx.listener(|this, index: &usize, window, cx| {
                this.select_index(*index, window, cx);
            }))
            .into_any_element()
    }
}

impl Render for DatabaseViewBlock {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let root = v_flex()
            .id(block_id(self.block))
            .test_support()
            .w_full()
            .gap_1()
            .py_2();
        if !self.alive(cx) {
            return root.child(
                div()
                    .id("database-deleted")
                    .test_support()
                    .aria_label("This database was deleted")
                    .py_6()
                    .px_3()
                    .rounded(px(6.))
                    .bg(cx.theme().muted)
                    .text_color(cx.theme().muted_foreground)
                    .child("This database was deleted"),
            );
        }
        let header = self.header(cx);
        let body = self.shown.as_ref().map(|shown| {
            let tabs = self.tabs(shown, cx);
            (tabs, shown.toolbar.clone(), shown.content.clone())
        });
        root.child(header)
            .when_some(body, |this, (tabs, toolbar, content)| {
                this.child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(tabs))
                        .child(toolbar),
                )
                .child(content)
            })
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::TestAppContext;
    use gpui_kit::test::TestWindowExt as _;
    use remindr_core::application::Command;
    use remindr_core::domain::{PropertyKind, ViewKind};

    use crate::app::testing::{Ui, execute, model_with};
    use crate::app::{Route, WorkspaceModel};
    use crate::database::ids::{block_id, tab_label_id};
    use crate::database::testing::{
        Tasks, UiExt as _, embed_database, open_block, tasks, view_count,
    };

    type Model = gpui_kit::Entity<WorkspaceModel>;

    fn tasks_model(cx: &mut TestAppContext) -> (Model, Tasks) {
        let mut seeded = None;
        let model = model_with(cx, |session| seeded = Some(tasks(session)));
        (model, seeded.unwrap())
    }

    #[gpui_kit::test]
    fn view_block_renders_the_database_name_and_its_tabs(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        execute(
            &model,
            cx,
            Command::AddView {
                block: tasks.block,
                name: "Board".to_owned(),
                kind: ViewKind::Board {
                    group_by: tasks.status,
                },
            },
        );
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(ui.exists(cx, block_id(tasks.block)));
        assert_eq!(ui.label(cx, "database-header").as_deref(), Some("Tasks"));
        ui.with(cx, |window, _| {
            let tabs = window.within("view-tabs");
            assert_eq!(tabs.find(0usize).label(), Some("Table"));
            assert_eq!(tabs.find(1usize).label(), Some("Board"));
            assert_eq!(tabs.find(0usize).selected(), Some(true));
            assert_eq!(tabs.find(1usize).selected(), Some(false));
        });
        ui.with(cx, |window, cx| {
            window.within("view-tabs").click(1usize, cx)
        });
        ui.settle(cx);
        ui.with(cx, |window, _| {
            let tabs = window.within("view-tabs");
            assert_eq!(tabs.find(0usize).selected(), Some(false));
            assert_eq!(tabs.find(1usize).selected(), Some(true));
        });
    }

    #[gpui_kit::test]
    fn clicking_the_header_opens_the_database_screen(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "database-header");
        assert_eq!(ui.route(cx), Some(Route::Database(tasks.database)));
    }

    #[gpui_kit::test]
    fn adding_a_board_view_needs_a_select_property(cx: &mut TestAppContext) {
        let mut seeded = None;
        let model = model_with(cx, |session| {
            let (_, block, database, _) = embed_database(session);
            seeded = Some((block, database));
        });
        let (block, database) = seeded.unwrap();
        let ui = open_block(cx, &model, block, database);
        assert_eq!(view_count(&model, cx, block), 1);

        ui.click(cx, "view-add");
        ui.with(cx, |window, _| {
            let menu = window.within("popup-menu");
            assert_eq!(menu.find(0usize).label(), Some("Table"));
            assert_eq!(menu.find(1usize).label(), Some("Board"));
        });
        assert!(ui.exists(cx, "view-add-hint"));
        ui.pick_item(cx, "popup-menu", 1usize);
        assert_eq!(view_count(&model, cx, block), 1);

        ui.press(cx, "escape");
        let status = execute(
            &model,
            cx,
            Command::AddProperty {
                database,
                name: "Status".to_owned(),
                kind: PropertyKind::Select {
                    options: Vec::new(),
                },
            },
        );
        assert!(status.databases.contains(&database));
        let status = model.read_with(cx, |model, _| {
            model
                .workspace()
                .database(database)
                .unwrap()
                .schema
                .properties
                .last()
                .unwrap()
                .id
        });
        ui.settle(cx);
        ui.click(cx, "view-add");
        assert!(!ui.exists(cx, "view-add-hint"));
        ui.pick_item(cx, "popup-menu", 1usize);
        assert_eq!(view_count(&model, cx, block), 2);
        model.read_with(cx, |model, _| {
            let views: Vec<_> = model.workspace().views(block).collect();
            assert_eq!(views[1].name, "Board");
            assert_eq!(views[1].kind, ViewKind::Board { group_by: status });
        });
        ui.with(cx, |window, _| {
            assert_eq!(
                window.within("view-tabs").find(1usize).selected(),
                Some(true)
            );
        });
    }

    #[gpui_kit::test]
    fn adding_a_table_view_selects_it(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "view-add");
        ui.pick_item(cx, "popup-menu", 0usize);
        assert_eq!(view_count(&model, cx, tasks.block), 2);
        ui.with(cx, |window, _| {
            assert_eq!(
                window.within("view-tabs").find(1usize).selected(),
                Some(true)
            );
        });
    }

    #[gpui_kit::test]
    fn renaming_a_view_from_its_tab_menu(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.right_click(cx, tab_label_id(tasks.view));
        ui.pick_item(cx, "popup-menu", 0usize);
        ui.press(cx, "cmd-a");
        ui.input(cx, "Backlog");
        model.read_with(cx, |model, _| {
            assert_eq!(model.workspace().view(tasks.view).unwrap().name, "Table");
        });
        ui.press(cx, "enter");
        model.read_with(cx, |model, _| {
            assert_eq!(model.workspace().view(tasks.view).unwrap().name, "Backlog");
        });
        ui.with(cx, |window, _| {
            assert_eq!(
                window.within("view-tabs").find(0usize).label(),
                Some("Backlog")
            );
        });
    }

    #[gpui_kit::test]
    fn escape_cancels_a_view_rename(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.right_click(cx, tab_label_id(tasks.view));
        ui.pick_item(cx, "popup-menu", 0usize);
        ui.press(cx, "cmd-a");
        ui.input(cx, "Backlog");
        ui.press(cx, "escape");
        ui.with(cx, |window, _| {
            assert_eq!(
                window.within("view-tabs").find(0usize).label(),
                Some("Table")
            );
        });
        model.read_with(cx, |model, _| {
            assert_eq!(model.workspace().view(tasks.view).unwrap().name, "Table");
        });
    }

    #[gpui_kit::test]
    fn removing_the_last_view_shows_an_error(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(!ui.exists(cx, "notification"));
        ui.right_click(cx, tab_label_id(tasks.view));
        ui.pick_item(cx, "popup-menu", 1usize);
        assert!(ui.exists(cx, "notification"));
        assert_eq!(view_count(&model, cx, tasks.block), 1);
    }

    #[gpui_kit::test]
    fn removing_a_view_selects_the_first_one(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        ui.click(cx, "view-add");
        ui.pick_item(cx, "popup-menu", 0usize);
        let second = model.read_with(cx, |model, _| {
            model.workspace().views(tasks.block).nth(1).unwrap().id
        });
        ui.right_click(cx, tab_label_id(second));
        ui.pick_item(cx, "popup-menu", 1usize);
        assert_eq!(view_count(&model, cx, tasks.block), 1);
        ui.with(cx, |window, _| {
            assert_eq!(
                window.within("view-tabs").find(0usize).selected(),
                Some(true)
            );
        });
    }

    #[gpui_kit::test]
    fn deleted_database_shows_the_placeholder(cx: &mut TestAppContext) {
        let (model, tasks) = tasks_model(cx);
        let ui = open_block(cx, &model, tasks.block, tasks.database);
        assert!(!ui.exists(cx, "database-deleted"));
        execute(
            &model,
            cx,
            Command::DeleteDatabase {
                database: tasks.database,
            },
        );
        ui.settle(cx);
        assert_eq!(
            ui.label(cx, "database-deleted").as_deref(),
            Some("This database was deleted")
        );
        assert!(!ui.exists(cx, "database-header"));
    }

    #[gpui_kit::test]
    fn the_editor_embeds_a_database_view_block(cx: &mut TestAppContext) {
        cx.update(crate::database::init);
        let (model, tasks) = tasks_model(cx);
        let ui = Ui::open(cx, &model);
        assert!(ui.exists(cx, block_id(tasks.block)));
    }
}
