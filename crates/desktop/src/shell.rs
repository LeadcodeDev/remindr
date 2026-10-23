use gpui_kit::component::resizable::{h_resizable, resizable_panel};
use gpui_kit::component::{ActiveTheme as _, TitleBar, v_flex};
use gpui_kit::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, ParentElement as _, Render, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, div,
};
use remindr_core::application::Changes;
use remindr_core::domain::{BlockId, DatabaseId};

use crate::app::commands;
use crate::app::navigation::{affects_navigation, first_root_page, route_alive, trail};
use crate::app::{Navigator, NavigatorHandle, Route, WorkspaceModel};
use crate::database::DatabaseScreen;
use crate::keymap::{Back, NewPage, Redo, Undo};
use crate::page::PageView;
use crate::sidebar::SidebarView;
use crate::theme::{self, SIDEBAR_MAX_WIDTH, SIDEBAR_MIN_WIDTH, SIDEBAR_WIDTH, UI_TEXT};

pub struct Shell {
    model: Entity<WorkspaceModel>,
    navigator: Entity<Navigator>,
    sidebar: Entity<SidebarView>,
    page: Option<(BlockId, Entity<PageView>)>,
    database: Option<(DatabaseId, Entity<DatabaseScreen>)>,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl Shell {
    pub fn new(model: Entity<WorkspaceModel>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        theme::install(window, cx);
        let navigator = cx.new(|_| Navigator::new());
        cx.set_global(NavigatorHandle(navigator.clone()));
        if let Some(page) = first_root_page(model.read(cx).workspace()) {
            navigator.update(cx, |navigator, cx| navigator.open(Route::Page(page), cx));
        }
        let sidebar = cx.new(|cx| SidebarView::new(model.clone(), navigator.clone(), cx));
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let subscriptions = vec![
            cx.subscribe_in(&model, window, Self::on_changes),
            cx.observe_in(&navigator, window, Self::on_route),
            cx.on_focus_lost(window, |this, window, cx| {
                this.focus_handle.focus(window, cx);
            }),
        ];
        let mut shell = Self {
            model,
            navigator,
            sidebar,
            page: None,
            database: None,
            focus_handle,
            _subscriptions: subscriptions,
        };
        shell.sync_route(window, cx);
        shell
    }

    fn on_changes(
        &mut self,
        model: &Entity<WorkspaceModel>,
        changes: &Changes,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !affects_navigation(model.read(cx).workspace(), changes) {
            return;
        }
        let model = model.clone();
        self.navigator.update(cx, |navigator, cx| {
            let workspace = model.read(cx).workspace();
            if navigator.prune(|route| route_alive(workspace, route)) {
                cx.notify();
            }
            if navigator.route().is_none()
                && let Some(page) = first_root_page(model.read(cx).workspace())
            {
                navigator.open(Route::Page(page), cx);
            }
        });
        cx.notify();
    }

    fn on_route(&mut self, _: Entity<Navigator>, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_route(window, cx);
    }

    fn sync_route(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let route = self
            .navigator
            .read(cx)
            .route()
            .copied()
            .filter(|route| route_alive(self.model.read(cx).workspace(), route));
        match route {
            Some(Route::Page(id)) => {
                if self.page.as_ref().map(|(page, _)| *page) != Some(id) {
                    let model = self.model.clone();
                    let view = cx.new(|cx| PageView::new(model, id, window, cx));
                    self.page = Some((id, view));
                }
                self.database = None;
            }
            Some(Route::Database(id)) => {
                if self.database.as_ref().map(|(database, _)| *database) != Some(id) {
                    let model = self.model.clone();
                    let screen = cx.new(|cx| DatabaseScreen::new(model, id, window, cx));
                    self.database = Some((id, screen));
                }
                self.page = None;
            }
            None => {
                self.page = None;
                self.database = None;
            }
        }
        cx.notify();
    }

    fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
        commands::undo(&self.model, window, cx);
    }

    fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
        commands::redo(&self.model, window, cx);
    }

    fn new_page(&mut self, _: &NewPage, window: &mut Window, cx: &mut Context<Self>) {
        commands::create_root_page(&self.model, &self.navigator, window, cx);
    }

    fn back(&mut self, _: &Back, _: &mut Window, cx: &mut Context<Self>) {
        self.navigator
            .update(cx, |navigator, cx| navigator.back(cx));
    }

    fn breadcrumb(&self, cx: &Context<Self>) -> String {
        self.navigator
            .read(cx)
            .route()
            .map(|route| trail(self.model.read(cx).workspace(), route).join(" / "))
            .unwrap_or_default()
    }

    fn main_area(&self, cx: &Context<Self>) -> AnyElement {
        if let Some((_, page)) = &self.page {
            page.clone().into_any_element()
        } else if let Some((_, screen)) = &self.database {
            screen.clone().into_any_element()
        } else {
            div()
                .id("empty-state")
                .test_support()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().muted_foreground)
                .child("Select or create a page")
                .into_any_element()
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let background = cx.theme().background;
        let foreground = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let breadcrumb = self.breadcrumb(cx);
        let main_area = self.main_area(cx);
        v_flex()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_action(cx.listener(Self::new_page))
            .on_action(cx.listener(Self::back))
            .size_full()
            .bg(background)
            .text_color(foreground)
            .text_size(UI_TEXT)
            .child(
                TitleBar::new().child(
                    div()
                        .id("breadcrumb")
                        .aria_label(breadcrumb.clone())
                        .test_support()
                        .min_w_0()
                        .truncate()
                        .text_size(UI_TEXT)
                        .text_color(muted)
                        .child(breadcrumb),
                ),
            )
            .child(
                div().flex_1().min_h_0().child(
                    h_resizable("shell-panels")
                        .child(
                            resizable_panel()
                                .size(SIDEBAR_WIDTH)
                                .size_range(SIDEBAR_MIN_WIDTH..SIDEBAR_MAX_WIDTH)
                                .fixed()
                                .child(
                                    div()
                                        .id("sidebar")
                                        .test_support()
                                        .size_full()
                                        .child(self.sidebar.clone()),
                                ),
                        )
                        .child(
                            resizable_panel().child(
                                div()
                                    .id("main-area")
                                    .test_support()
                                    .size_full()
                                    .child(main_area),
                            ),
                        ),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::px;
    use gpui_kit::test::TestWindowExt as _;
    use remindr_core::domain::Parent;

    use crate::app::Route;
    use crate::app::navigation::root_pages;
    use crate::app::testing::{Ui, add_page, model, model_with};
    use crate::sidebar::page_open_id;

    #[gpui_kit::test]
    fn shell_renders_sidebar_and_main_area(cx: &mut gpui_kit::TestAppContext) {
        let model = model(cx);
        let ui = Ui::open(cx, &model);
        ui.with(cx, |window, _| {
            let sidebar = window.find("sidebar");
            let main_area = window.find("main-area");
            assert_eq!(sidebar.bounds().size.width, px(240.));
            assert!(main_area.bounds().size.width > px(0.));
            assert!(main_area.bounds().left() >= sidebar.bounds().right());
        });
    }

    #[gpui_kit::test]
    fn breadcrumb_shows_ancestor_titles(cx: &mut gpui_kit::TestAppContext) {
        let mut child = None;
        let model = model_with(cx, |session| {
            let parent = add_page(session, Parent::Root, "A");
            child = Some(add_page(session, Parent::Block(parent), "B"));
        });
        let ui = Ui::open(cx, &model);
        let navigator = ui.navigator(cx);
        navigator.update(cx, |navigator, cx| {
            navigator.open(crate::app::Route::Page(child.unwrap()), cx);
        });
        ui.settle(cx);
        assert_eq!(ui.label(cx, "breadcrumb").as_deref(), Some("A / B"));
    }

    #[gpui_kit::test]
    fn new_page_shortcut_creates_and_opens_a_root_page(cx: &mut gpui_kit::TestAppContext) {
        let model = model(cx);
        let ui = Ui::open(cx, &model);
        ui.press(cx, "cmd-n");
        let pages = model.read_with(cx, |model, _| root_pages(model.workspace()));
        assert_eq!(pages.len(), 1);
        assert_eq!(ui.route(cx), Some(Route::Page(pages[0])));
    }

    #[gpui_kit::test]
    fn undo_and_redo_shortcuts_drive_the_model(cx: &mut gpui_kit::TestAppContext) {
        let model = model(cx);
        let ui = Ui::open(cx, &model);
        ui.press(cx, "cmd-n");
        let count = |cx: &mut gpui_kit::TestAppContext| {
            model.read_with(cx, |model, _| root_pages(model.workspace()).len())
        };
        assert_eq!(count(cx), 1);
        let page = model.read_with(cx, |model, _| root_pages(model.workspace())[0]);
        assert!(ui.find(cx, page_open_id(page)).is_some());
        ui.press(cx, "cmd-z");
        assert_eq!(count(cx), 0);
        assert_eq!(ui.route(cx), None);
        assert!(ui.find(cx, page_open_id(page)).is_none());
        ui.press(cx, "cmd-shift-z");
        assert_eq!(count(cx), 1);
        assert_eq!(ui.route(cx), Some(Route::Page(page)));
        assert!(ui.find(cx, page_open_id(page)).is_some());
    }

    #[gpui_kit::test]
    fn alternate_back_shortcut_returns_to_the_previous_route(cx: &mut gpui_kit::TestAppContext) {
        let (mut first, mut second) = (None, None);
        let model = model_with(cx, |session| {
            first = Some(add_page(session, Parent::Root, "First"));
            second = Some(add_page(session, Parent::Root, "Second"));
        });
        let (first, second) = (first.unwrap(), second.unwrap());
        let ui = Ui::open(cx, &model);
        ui.click(cx, page_open_id(second));
        assert_eq!(ui.route(cx), Some(Route::Page(second)));
        ui.press(cx, "cmd-alt-left");
        assert_eq!(ui.route(cx), Some(Route::Page(first)));
    }

    #[gpui_kit::test]
    fn back_shortcut_returns_to_the_previous_route(cx: &mut gpui_kit::TestAppContext) {
        let (mut first, mut second) = (None, None);
        let model = model_with(cx, |session| {
            first = Some(add_page(session, Parent::Root, "First"));
            second = Some(add_page(session, Parent::Root, "Second"));
        });
        let (first, second) = (first.unwrap(), second.unwrap());
        let ui = Ui::open(cx, &model);
        ui.click(cx, page_open_id(second));
        assert_eq!(ui.route(cx), Some(Route::Page(second)));
        ui.press(cx, "cmd-[");
        assert_eq!(ui.route(cx), Some(Route::Page(first)));
    }
}
