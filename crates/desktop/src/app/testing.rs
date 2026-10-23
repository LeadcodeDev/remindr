use std::time::Duration;

use gpui_kit::test::{ElementSnapshot, TestAppContextExt as _, TestWindowExt as _};
use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, Bounds, ElementId, Entity, Point, TestAppContext,
    Window, WindowBounds, WindowOptions, px, size,
};
use remindr_core::application::{Command, DropTarget, Session};
use remindr_core::domain::{BlockId, BlockKind, Parent, RichText};
use remindr_core::testing::MemoryStore;

use super::commands::{created_block, created_database};
use super::{AppStore, Navigator, NavigatorHandle, Route, SystemClock, WorkspaceModel};
use crate::shell::Shell;

pub fn session() -> Session<AppStore, SystemClock> {
    Session::open(AppStore::Memory(MemoryStore::new()), SystemClock).unwrap()
}

pub fn add_page(
    session: &mut Session<AppStore, SystemClock>,
    parent: Parent,
    title: &str,
) -> BlockId {
    let after = session.workspace().children(parent).last().map(|b| b.id);
    let changes = session
        .execute(Command::CreatePage { parent, after })
        .unwrap();
    let page = created_block(&changes).unwrap();
    session
        .execute(Command::EditText {
            block: page,
            text: RichText::plain(title),
        })
        .unwrap();
    page
}

pub fn add_block(
    session: &mut Session<AppStore, SystemClock>,
    parent: BlockId,
    kind: BlockKind,
    text: &str,
) -> BlockId {
    let parent = Parent::Block(parent);
    let after = session.workspace().children(parent).last().map(|b| b.id);
    let changes = session
        .execute(Command::InsertBlockAt {
            target: DropTarget { parent, after },
            kind,
        })
        .unwrap();
    let block = created_block(&changes).unwrap();
    session
        .execute(Command::EditText {
            block,
            text: RichText::plain(text),
        })
        .unwrap();
    block
}

pub fn model(cx: &mut TestAppContext) -> Entity<WorkspaceModel> {
    cx.new(|_| WorkspaceModel::new(session()))
}

pub fn failing_model(cx: &mut TestAppContext) -> (Entity<WorkspaceModel>, MemoryStore) {
    let store = MemoryStore::new();
    let session = Session::open(AppStore::Memory(store.shared()), SystemClock).unwrap();
    (cx.new(|_| WorkspaceModel::new(session)), store)
}

pub fn model_with(
    cx: &mut TestAppContext,
    build: impl FnOnce(&mut Session<AppStore, SystemClock>),
) -> Entity<WorkspaceModel> {
    let mut session = session();
    build(&mut session);
    cx.new(|_| WorkspaceModel::new(session))
}

pub fn execute(
    model: &Entity<WorkspaceModel>,
    cx: &mut TestAppContext,
    command: Command,
) -> remindr_core::application::Changes {
    model.update(cx, |model, cx| model.execute(command, cx).unwrap())
}

pub fn create_database(model: &Entity<WorkspaceModel>, cx: &mut TestAppContext) {
    let changes = execute(model, cx, Command::CreateDatabase);
    created_database(&changes).unwrap();
}

pub struct Ui {
    pub handle: AnyWindowHandle,
    pub model: Entity<WorkspaceModel>,
}

impl Ui {
    pub fn open(cx: &mut TestAppContext, model: &Entity<WorkspaceModel>) -> Ui {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::keymap::init(cx);
            crate::editor::init(cx);
            crate::database::init(cx);
        });
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: size(px(1100.), px(700.)),
            })),
            ..Default::default()
        };
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| Shell::new(model.clone(), window, cx))
            })
            .unwrap()
        });
        let ui = Ui {
            handle,
            model: model.clone(),
        };
        ui.settle(cx);
        ui
    }

    pub fn settle(&self, cx: &mut TestAppContext) {
        cx.run_until_parked();
        self.with(cx, |window, cx| window.render_frame(cx));
        cx.run_until_parked();
        self.with(cx, |window, cx| window.render_frame(cx));
    }

    pub fn with<R>(
        &self,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Window, &mut App) -> R,
    ) -> R {
        cx.update_window(self.handle, |_, window, cx| f(window, cx))
            .unwrap()
    }

    pub fn click(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) {
        let id = id.into();
        self.with(cx, |window, cx| window.click(id, cx));
        self.settle(cx);
    }

    pub fn hover(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) {
        let id = id.into();
        self.with(cx, |window, cx| window.hover(id, cx));
        self.settle(cx);
    }

    pub fn press(&self, cx: &mut TestAppContext, key: &str) {
        self.with(cx, |window, cx| window.press(key, cx));
        self.settle(cx);
    }

    pub fn input(&self, cx: &mut TestAppContext, text: &str) {
        self.with(cx, |window, cx| window.input(text, cx));
        self.settle(cx);
    }

    pub fn find(
        &self,
        cx: &mut TestAppContext,
        id: impl Into<ElementId>,
    ) -> Option<ElementSnapshot> {
        let id = id.into();
        self.with(cx, |window, _| window.try_find(id))
    }

    pub fn label(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) -> Option<String> {
        self.find(cx, id)
            .and_then(|element| element.label().map(str::to_owned))
    }

    pub fn route(&self, cx: &mut TestAppContext) -> Option<Route> {
        cx.read(|cx| cx.global::<NavigatorHandle>().0.read(cx).route().copied())
    }

    pub fn navigator(&self, cx: &mut TestAppContext) -> Entity<Navigator> {
        cx.read(|cx| cx.global::<NavigatorHandle>().0.clone())
    }
}

impl Ui {
    pub async fn wait_for(
        &self,
        cx: &mut TestAppContext,
        predicate: impl FnMut(&mut Window, &mut App) -> bool,
    ) {
        cx.wait_for(self.handle, Duration::from_secs(2), predicate)
            .await;
    }

    pub async fn pick_menu_item(
        &self,
        cx: &mut TestAppContext,
        trigger: impl Into<ElementId>,
        item: usize,
    ) {
        self.click(cx, trigger);
        self.wait_for(cx, |window, _| window.try_find("popup-menu").is_some())
            .await;
        self.with(cx, |window, cx| window.within("popup-menu").click(item, cx));
        self.settle(cx);
    }
}
