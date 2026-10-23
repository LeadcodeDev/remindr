use std::collections::HashMap;

use remindr_core::domain::{
    Block, BlockId, BlockKind, Parent, RichText, SortKey, Timestamp, Uuid, Workspace,
    WorkspaceSnapshot,
};

pub struct Doc {
    blocks: Vec<Block>,
    last_key: HashMap<Parent, SortKey>,
    next: u128,
    page: BlockId,
}

impl Doc {
    pub fn new() -> Self {
        let page = BlockId::from_uuid(Uuid::from_u128(1));
        let root = Block {
            id: page,
            kind: BlockKind::Page { icon: None },
            parent: Parent::Root,
            sort_key: SortKey::between(None, None),
            text: RichText::plain("Page"),
            alive: true,
            created_at: Timestamp::from_millis(0),
        };
        Self {
            blocks: vec![root],
            last_key: HashMap::new(),
            next: 2,
            page,
        }
    }

    pub fn page(&self) -> BlockId {
        self.page
    }

    pub fn add(&mut self, parent: Parent, kind: BlockKind, text: &str) -> BlockId {
        let id = BlockId::from_uuid(Uuid::from_u128(self.next));
        self.next += 1;
        let sort_key = SortKey::between(self.last_key.get(&parent), None);
        self.last_key.insert(parent, sort_key.clone());
        self.blocks.push(Block {
            id,
            kind,
            parent,
            sort_key,
            text: RichText::plain(text),
            alive: true,
            created_at: Timestamp::from_millis(0),
        });
        id
    }

    pub fn child(&mut self, kind: BlockKind, text: &str) -> BlockId {
        self.add(Parent::Block(self.page), kind, text)
    }

    pub fn under(&mut self, parent: BlockId, kind: BlockKind, text: &str) -> BlockId {
        self.add(Parent::Block(parent), kind, text)
    }

    pub fn snapshot(&self) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            blocks: self.blocks.clone(),
            ..WorkspaceSnapshot::default()
        }
    }

    pub fn workspace(&self) -> Workspace {
        Workspace::from_snapshot(self.snapshot()).expect("valid fixture")
    }
}

pub use fixture::Fixture;

mod fixture {
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AnyWindowHandle, App, AppContext, Bounds, Entity, Point, TestAppContext, Window,
        WindowBounds, WindowOptions, px, size,
    };
    use remindr_core::application::{Session, TextPoint};
    use remindr_core::domain::BlockId;
    use remindr_core::testing::MemoryStore;

    use super::Doc;
    use crate::app::{AppStore, SystemClock, WorkspaceModel};
    use crate::editor::{self, BlockEditor};

    pub struct Fixture {
        pub handle: AnyWindowHandle,
        pub editor: Entity<BlockEditor>,
        pub model: Entity<WorkspaceModel>,
        pub store: MemoryStore,
        pub page: BlockId,
    }

    impl Fixture {
        pub fn open(cx: &mut TestAppContext, doc: &Doc) -> Self {
            cx.update(gpui_kit::init);
            cx.update(editor::init);
            let store = MemoryStore::with_snapshot(doc.snapshot());
            let probe = store.shared();
            let session =
                Session::open(AppStore::Memory(store), SystemClock).expect("open session");
            let model = cx.new(|_| WorkspaceModel::new(session));
            let page = doc.page();
            let bounds = Bounds {
                origin: Point::default(),
                size: size(px(800.), px(600.)),
            };
            let shared = model.clone();
            let (handle, editor) = cx.update(|cx| {
                gpui_kit::open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        ..Default::default()
                    },
                    cx,
                    |window, cx| cx.new(|cx| BlockEditor::new(shared, page, window, cx)),
                )
                .expect("open test window")
            });
            let fixture = Self {
                handle,
                editor,
                model,
                store: probe,
                page,
            };
            fixture.with(cx, |window, cx| window.render_frame(cx));
            fixture
        }

        pub fn with<R>(
            &self,
            cx: &mut TestAppContext,
            body: impl FnOnce(&mut Window, &mut App) -> R,
        ) -> R {
            cx.update_window(self.handle, |_, window, cx| body(window, cx))
                .expect("update test window")
        }

        pub fn text(&self, cx: &mut TestAppContext, block: BlockId) -> String {
            self.with(cx, |_, cx| {
                self.model
                    .read(cx)
                    .workspace()
                    .block(block)
                    .map(|found| found.text.to_plain())
                    .unwrap_or_default()
            })
        }

        pub fn texts(&self, cx: &mut TestAppContext) -> Vec<String> {
            self.with(cx, |_, cx| {
                let workspace = self.model.read(cx).workspace();
                editor::rows::rows(workspace, self.page, editor::rows::Expansion::All)
                    .into_iter()
                    .filter_map(|row| workspace.block(row.block))
                    .map(|block| block.text.to_plain())
                    .collect()
            })
        }

        pub fn caret(&self, cx: &mut TestAppContext) -> Option<TextPoint> {
            self.with(cx, |_, cx| {
                self.editor
                    .read(cx)
                    .selection()
                    .map(|selection| selection.head)
            })
        }

        pub fn press(&self, cx: &mut TestAppContext, key: &str) {
            self.with(cx, |window, cx| window.press(key, cx));
        }

        pub fn input(&self, cx: &mut TestAppContext, text: &str) {
            self.with(cx, |window, cx| window.input(text, cx));
        }

        pub fn click_block(&self, cx: &mut TestAppContext, block: BlockId) {
            self.with(cx, |window, cx| {
                window.click(editor::render::row_id(block), cx);
            });
        }

        pub fn focus_at(&self, cx: &mut TestAppContext, point: TextPoint) {
            self.with(cx, |window, cx| {
                self.editor.update(cx, |editor, cx| {
                    editor.place_caret(point, window, cx);
                });
                window.render_frame(cx);
            });
        }
    }
}
