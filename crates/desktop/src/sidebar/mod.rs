mod entries;
mod rename;
mod section;

use std::collections::HashSet;

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::sidebar::{Sidebar, SidebarCollapsible};
use gpui_kit::component::{Icon, IconName, h_flex, v_flex};
use gpui_kit::{
    App, Context, Entity, IntoElement, ParentElement as _, Render, Styled as _, Subscription,
    Window,
};
use remindr_core::application::{Changes, Command, Created};
use remindr_core::domain::{BlockId, DatabaseId};

use crate::app::commands;
use crate::app::navigation::{affects_navigation, alive_page, page_ancestors};
use crate::app::{Navigator, Route, WorkspaceModel};

pub use entries::*;
use section::Section;

#[derive(Clone)]
pub struct Handles {
    model: Entity<WorkspaceModel>,
    navigator: Entity<Navigator>,
    view: Entity<SidebarView>,
}

impl Handles {
    fn open(&self, route: Route, cx: &mut App) {
        self.navigator
            .update(cx, |navigator, cx| navigator.open(route, cx));
    }

    fn toggle(&self, page: BlockId, cx: &mut App) {
        self.view.update(cx, |view, cx| view.toggle(page, cx));
    }

    fn add_child_page(&self, parent: BlockId, window: &mut Window, cx: &mut App) {
        commands::create_child_page(&self.model, &self.navigator, parent, window, cx);
    }

    fn add_root_page(&self, window: &mut Window, cx: &mut App) {
        commands::create_root_page(&self.model, &self.navigator, window, cx);
    }

    fn add_database(&self, window: &mut Window, cx: &mut App) {
        commands::create_database(&self.model, &self.navigator, window, cx);
    }

    fn delete_page(&self, page: BlockId, window: &mut Window, cx: &mut App) {
        commands::run(
            &self.model,
            Command::DeleteBlocks { blocks: vec![page] },
            window,
            cx,
        );
    }

    fn delete_database(&self, database: DatabaseId, window: &mut Window, cx: &mut App) {
        commands::run(
            &self.model,
            Command::DeleteDatabase { database },
            window,
            cx,
        );
    }
}

pub struct SidebarView {
    model: Entity<WorkspaceModel>,
    navigator: Entity<Navigator>,
    expanded: HashSet<BlockId>,
    _subscriptions: Vec<Subscription>,
}

impl SidebarView {
    pub fn new(
        model: Entity<WorkspaceModel>,
        navigator: Entity<Navigator>,
        cx: &mut Context<Self>,
    ) -> Self {
        let subscriptions = vec![
            cx.subscribe(&model, Self::on_changes),
            cx.observe(&navigator, Self::on_route),
        ];
        let mut view = Self {
            model,
            navigator,
            expanded: HashSet::new(),
            _subscriptions: subscriptions,
        };
        view.reveal_route(cx);
        view
    }

    fn toggle(&mut self, page: BlockId, cx: &mut Context<Self>) {
        if !self.expanded.remove(&page) {
            self.expanded.insert(page);
        }
        cx.notify();
    }

    fn reveal(&mut self, page: BlockId, cx: &Context<Self>) {
        let ancestors = page_ancestors(self.model.read(cx).workspace(), page);
        self.expanded.extend(ancestors);
    }

    fn reveal_route(&mut self, cx: &Context<Self>) {
        if let Some(Route::Page(page)) = self.navigator.read(cx).route().copied() {
            self.reveal(page, cx);
        }
    }

    fn on_changes(
        &mut self,
        model: Entity<WorkspaceModel>,
        changes: &Changes,
        cx: &mut Context<Self>,
    ) {
        for created in &changes.created {
            if let Created::Block(block) = created
                && alive_page(model.read(cx).workspace(), *block).is_some()
            {
                self.reveal(*block, cx);
            }
        }
        if affects_navigation(model.read(cx).workspace(), changes) {
            cx.notify();
        }
    }

    fn on_route(&mut self, _: Entity<Navigator>, cx: &mut Context<Self>) {
        self.reveal_route(cx);
        cx.notify();
    }
}

impl Render for SidebarView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let route = self.navigator.read(cx).route().copied();
        let workspace = self.model.read(cx).workspace();
        let pages = page_entries(workspace, route.as_ref(), &self.expanded);
        let databases = database_entries(workspace, route.as_ref());
        let handles = Handles {
            model: self.model.clone(),
            navigator: self.navigator.clone(),
            view: cx.entity(),
        };
        Sidebar::<Section>::new("sidebar-nav")
            .collapsible(SidebarCollapsible::None)
            .w_full()
            .child(Section::new("sidebar-pages", "Pages", pages, &handles))
            .child(Section::new(
                "sidebar-databases",
                "Databases",
                databases,
                &handles,
            ))
            .footer(footer(&handles))
    }
}

fn footer_label(icon: IconName, label: &'static str) -> impl IntoElement {
    h_flex()
        .w_full()
        .gap_2()
        .items_center()
        .child(Icon::new(icon).size_4())
        .child(label)
}

fn footer(handles: &Handles) -> impl IntoElement {
    let pages = handles.clone();
    let databases = handles.clone();
    v_flex()
        .w_full()
        .gap_0p5()
        .child(
            Button::new("new-page")
                .ghost()
                .w_full()
                .accessibility_label("New page")
                .child(footer_label(IconName::Plus, "New page"))
                .on_click(move |_, window, cx| pages.add_root_page(window, cx)),
        )
        .child(
            Button::new("new-database")
                .ghost()
                .w_full()
                .accessibility_label("New database")
                .child(footer_label(IconName::LayoutDashboard, "New database"))
                .on_click(move |_, window, cx| databases.add_database(window, cx)),
        )
}

#[cfg(test)]
mod tests {
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{Entity, TestAppContext};
    use remindr_core::application::Command;
    use remindr_core::domain::{BlockId, Parent};

    use super::{
        database_entry_id, database_menu_id, database_open_id, page_add_id, page_entry_id,
        page_menu_id, page_open_id, page_toggle_id,
    };
    use crate::app::navigation::{child_pages, root_pages};
    use crate::app::testing::{Ui, add_page, create_database, execute, model, model_with};
    use crate::app::{Route, WorkspaceModel};

    fn roots(model: &Entity<WorkspaceModel>, cx: &mut TestAppContext) -> Vec<BlockId> {
        model.read_with(cx, |model, _| root_pages(model.workspace()))
    }

    fn children(
        model: &Entity<WorkspaceModel>,
        cx: &mut TestAppContext,
        parent: BlockId,
    ) -> Vec<BlockId> {
        model.read_with(cx, |model, _| child_pages(model.workspace(), parent))
    }

    #[gpui_kit::test]
    fn new_page_appears_in_the_pages_group_and_opens(cx: &mut TestAppContext) {
        let model = model(cx);
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, "empty-state").is_some());
        ui.click(cx, "new-page");
        let pages = roots(&model, cx);
        assert_eq!(pages.len(), 1);
        assert_eq!(
            ui.label(cx, page_open_id(pages[0])).as_deref(),
            Some("Untitled")
        );
        assert_eq!(ui.route(cx), Some(Route::Page(pages[0])));
        assert!(ui.find(cx, "empty-state").is_none());
    }

    #[gpui_kit::test]
    fn child_page_appears_under_its_parent(cx: &mut TestAppContext) {
        let mut parent = None;
        let model = model_with(cx, |session| {
            parent = Some(add_page(session, Parent::Root, "Parent"));
        });
        let parent = parent.unwrap();
        let ui = Ui::open(cx, &model);
        ui.hover(cx, page_entry_id(parent));
        ui.click(cx, page_add_id(parent));
        let created = children(&model, cx, parent);
        assert_eq!(created.len(), 1);
        assert_eq!(ui.route(cx), Some(Route::Page(created[0])));
        let parent_entry = ui.find(cx, page_open_id(parent)).unwrap();
        let child_entry = ui.find(cx, page_open_id(created[0])).unwrap();
        assert_eq!(child_entry.label(), Some("Untitled"));
        assert!(child_entry.bounds().top() > parent_entry.bounds().top());
        assert!(child_entry.bounds().left() > parent_entry.bounds().left());
    }

    #[gpui_kit::test]
    fn sub_pages_created_elsewhere_show_in_the_tree(cx: &mut TestAppContext) {
        let mut parent = None;
        let model = model_with(cx, |session| {
            parent = Some(add_page(session, Parent::Root, "Parent"));
        });
        let parent = parent.unwrap();
        let ui = Ui::open(cx, &model);
        execute(
            &model,
            cx,
            Command::CreatePage {
                parent: Parent::Block(parent),
                after: None,
            },
        );
        ui.settle(cx);
        let created = children(&model, cx, parent);
        assert_eq!(created.len(), 1);
        assert!(ui.find(cx, page_open_id(created[0])).is_some());
    }

    #[gpui_kit::test]
    fn the_toggle_collapses_and_expands_children(cx: &mut TestAppContext) {
        let (mut parent, mut child) = (None, None);
        let model = model_with(cx, |session| {
            let page = add_page(session, Parent::Root, "Parent");
            parent = Some(page);
            child = Some(add_page(session, Parent::Block(page), "Child"));
        });
        let (parent, child) = (parent.unwrap(), child.unwrap());
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, page_open_id(child)).is_none());
        ui.click(cx, page_toggle_id(parent));
        assert_eq!(ui.label(cx, page_open_id(child)).as_deref(), Some("Child"));
        ui.click(cx, page_toggle_id(parent));
        assert!(ui.find(cx, page_open_id(child)).is_none());
    }

    #[gpui_kit::test]
    fn editing_inside_a_collapsed_branch_does_not_expand_it(cx: &mut TestAppContext) {
        let (mut parent, mut child) = (None, None);
        let model = model_with(cx, |session| {
            let page = add_page(session, Parent::Root, "Parent");
            parent = Some(page);
            child = Some(add_page(session, Parent::Block(page), "Child"));
        });
        let (parent, child) = (parent.unwrap(), child.unwrap());
        let ui = Ui::open(cx, &model);
        ui.click(cx, page_toggle_id(parent));
        assert!(ui.find(cx, page_open_id(child)).is_some());
        ui.click(cx, page_toggle_id(parent));
        assert!(ui.find(cx, page_open_id(child)).is_none());
        execute(
            &model,
            cx,
            Command::InsertBlockAt {
                target: remindr_core::application::DropTarget {
                    parent: Parent::Block(child),
                    after: None,
                },
                kind: remindr_core::domain::BlockKind::Text,
            },
        );
        ui.settle(cx);
        assert!(ui.find(cx, page_open_id(child)).is_none());
    }

    #[gpui_kit::test]
    fn clicking_an_entry_opens_it_and_selects_it(cx: &mut TestAppContext) {
        let (mut first, mut second) = (None, None);
        let model = model_with(cx, |session| {
            first = Some(add_page(session, Parent::Root, "First"));
            second = Some(add_page(session, Parent::Root, "Second"));
        });
        let (first, second) = (first.unwrap(), second.unwrap());
        let ui = Ui::open(cx, &model);
        assert_eq!(ui.route(cx), Some(Route::Page(first)));
        assert_eq!(
            ui.find(cx, page_open_id(first)).unwrap().selected(),
            Some(true)
        );
        ui.click(cx, page_open_id(second));
        assert_eq!(ui.route(cx), Some(Route::Page(second)));
        assert_eq!(
            ui.find(cx, page_open_id(second)).unwrap().selected(),
            Some(true)
        );
        assert_eq!(
            ui.find(cx, page_open_id(first)).unwrap().selected(),
            Some(false)
        );
    }

    #[gpui_kit::test]
    fn renamed_page_updates_the_sidebar(cx: &mut TestAppContext) {
        let mut page = None;
        let model = model_with(cx, |session| {
            page = Some(add_page(session, Parent::Root, "Before"));
        });
        let page = page.unwrap();
        let ui = Ui::open(cx, &model);
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("Before"));
        execute(
            &model,
            cx,
            Command::EditText {
                block: page,
                text: remindr_core::domain::RichText::plain("After"),
            },
        );
        ui.settle(cx);
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("After"));
    }

    #[gpui_kit::test]
    async fn rename_from_the_menu_edits_the_title(cx: &mut TestAppContext) {
        let mut page = None;
        let model = model_with(cx, |session| {
            page = Some(add_page(session, Parent::Root, "Before"));
        });
        let page = page.unwrap();
        let ui = Ui::open(cx, &model);
        ui.hover(cx, page_entry_id(page));
        ui.pick_menu_item(cx, page_menu_id(page), 0).await;
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_some())
            .await;
        assert_eq!(
            ui.find(cx, "rename-input")
                .and_then(|input| input.value().map(str::to_owned)),
            Some("Before".to_owned())
        );
        ui.press(cx, "cmd-a");
        ui.input(cx, "After");
        ui.press(cx, "enter");
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_none())
            .await;
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("After"));
    }

    #[gpui_kit::test]
    async fn escape_cancels_the_rename(cx: &mut TestAppContext) {
        let mut page = None;
        let model = model_with(cx, |session| {
            page = Some(add_page(session, Parent::Root, "Before"));
        });
        let page = page.unwrap();
        let ui = Ui::open(cx, &model);
        ui.hover(cx, page_entry_id(page));
        ui.pick_menu_item(cx, page_menu_id(page), 0).await;
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_some())
            .await;
        ui.press(cx, "cmd-a");
        ui.input(cx, "Discarded");
        ui.press(cx, "escape");
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_none())
            .await;
        assert_eq!(ui.label(cx, page_open_id(page)).as_deref(), Some("Before"));
    }

    #[gpui_kit::test]
    async fn deleted_page_leaves_the_sidebar(cx: &mut TestAppContext) {
        let (mut keep, mut gone, mut child) = (None, None, None);
        let model = model_with(cx, |session| {
            keep = Some(add_page(session, Parent::Root, "Keep"));
            let page = add_page(session, Parent::Root, "Gone");
            gone = Some(page);
            child = Some(add_page(session, Parent::Block(page), "Gone child"));
        });
        let (keep, gone, child) = (keep.unwrap(), gone.unwrap(), child.unwrap());
        let ui = Ui::open(cx, &model);
        ui.click(cx, page_toggle_id(gone));
        assert!(ui.find(cx, page_open_id(gone)).is_some());
        assert!(ui.find(cx, page_open_id(child)).is_some());
        ui.hover(cx, page_entry_id(gone));
        ui.pick_menu_item(cx, page_menu_id(gone), 1).await;
        assert!(ui.find(cx, page_open_id(gone)).is_none());
        assert!(ui.find(cx, page_open_id(child)).is_none());
        assert!(ui.find(cx, page_open_id(keep)).is_some());
        ui.press(cx, "cmd-z");
        assert!(ui.find(cx, page_open_id(gone)).is_some());
        assert!(ui.find(cx, page_open_id(child)).is_some());
        assert!(ui.find(cx, page_open_id(keep)).is_some());
    }

    #[gpui_kit::test]
    fn new_database_appears_in_the_databases_group_not_in_pages(cx: &mut TestAppContext) {
        let mut page = None;
        let model = model_with(cx, |session| {
            page = Some(add_page(session, Parent::Root, "Only page"));
        });
        let page = page.unwrap();
        let ui = Ui::open(cx, &model);
        ui.click(cx, "new-database");
        let database = model.read_with(cx, |model, _| {
            model
                .workspace()
                .databases()
                .next()
                .map(|database| database.id)
        });
        let database = database.unwrap();
        assert_eq!(
            ui.label(cx, database_open_id(database)).as_deref(),
            Some("Untitled")
        );
        assert_eq!(ui.route(cx), Some(Route::Database(database)));
        assert_eq!(roots(&model, cx), vec![page]);
        ui.with(cx, |window, _| {
            assert!(
                window
                    .within("sidebar-databases")
                    .try_find(database_entry_id(database))
                    .is_some()
            );
            assert!(
                window
                    .within("sidebar-pages")
                    .try_find(database_entry_id(database))
                    .is_none()
            );
            assert!(
                window
                    .within("sidebar-pages")
                    .try_find(page_entry_id(page))
                    .is_some()
            );
            assert!(
                window
                    .within("sidebar-databases")
                    .try_find(page_entry_id(page))
                    .is_none()
            );
        });
    }

    #[gpui_kit::test]
    fn rows_are_not_listed(cx: &mut TestAppContext) {
        let model = model(cx);
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        execute(
            &model,
            cx,
            Command::AddRow {
                database,
                values: Vec::new(),
            },
        );
        let row = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .rows(database)
                    .next()
                    .map(|block| block.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, database_open_id(database)).is_some());
        assert!(ui.find(cx, page_open_id(row)).is_none());
        assert!(roots(&model, cx).is_empty());
    }

    #[gpui_kit::test]
    async fn deleting_a_database_removes_its_entry(cx: &mut TestAppContext) {
        let model = model(cx);
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        assert!(ui.find(cx, database_open_id(database)).is_some());
        ui.hover(cx, database_entry_id(database));
        ui.pick_menu_item(cx, database_menu_id(database), 1).await;
        assert!(ui.find(cx, database_open_id(database)).is_none());
    }

    #[gpui_kit::test]
    async fn renaming_a_database_updates_its_entry(cx: &mut TestAppContext) {
        let model = model(cx);
        create_database(&model, cx);
        let database = model
            .read_with(cx, |model, _| {
                model
                    .workspace()
                    .databases()
                    .next()
                    .map(|database| database.id)
            })
            .unwrap();
        let ui = Ui::open(cx, &model);
        ui.hover(cx, database_entry_id(database));
        ui.pick_menu_item(cx, database_menu_id(database), 0).await;
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_some())
            .await;
        ui.press(cx, "cmd-a");
        ui.input(cx, "Tasks");
        ui.press(cx, "enter");
        ui.wait_for(cx, |window, _| window.try_find("rename-input").is_none())
            .await;
        assert_eq!(
            ui.label(cx, database_open_id(database)).as_deref(),
            Some("Tasks")
        );
    }
}
