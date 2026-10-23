use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    App, AppContext as _, Bounds, ElementId, Entity, Point, Render, TestAppContext, Window,
    WindowBounds, WindowOptions, div, px, size,
};
use remindr_core::application::{Changes, Command, Created, Session};
use remindr_core::domain::{
    BlockId, BlockKind, DatabaseId, OptionColor, OptionId, Parent, PropertyId, PropertyKind,
    PropertyValue, RichText, SelectOption, ViewId,
};

use crate::app::commands::{created_block, created_database};
use crate::app::testing::{Ui, add_block, add_page};
use crate::app::{AppStore, Navigator, NavigatorHandle, SystemClock, WorkspaceModel};
use crate::database::DatabaseViewBlock;

pub type TestSession = Session<AppStore, SystemClock>;

pub struct Tasks {
    pub page: BlockId,
    pub block: BlockId,
    pub database: DatabaseId,
    pub view: ViewId,
    pub title: PropertyId,
    pub status: PropertyId,
    pub priority: PropertyId,
    pub todo: OptionId,
    pub doing: OptionId,
    pub done: OptionId,
    pub rows: Vec<BlockId>,
}

pub fn run(session: &mut TestSession, command: Command) -> Changes {
    session.execute(command).unwrap()
}

pub fn last_property(session: &TestSession, database: DatabaseId) -> PropertyId {
    session
        .workspace()
        .database(database)
        .unwrap()
        .schema
        .properties
        .last()
        .unwrap()
        .id
}

pub fn add_property(
    session: &mut TestSession,
    database: DatabaseId,
    name: &str,
    kind: PropertyKind,
) -> PropertyId {
    run(
        session,
        Command::AddProperty {
            database,
            name: name.to_owned(),
            kind,
        },
    );
    last_property(session, database)
}

pub fn add_option(
    session: &mut TestSession,
    database: DatabaseId,
    property: PropertyId,
    name: &str,
    color: OptionColor,
) -> OptionId {
    run(
        session,
        Command::AddSelectOption {
            database,
            property,
            name: name.to_owned(),
            color,
        },
    );
    options_of(session, database, property).last().unwrap().id
}

pub fn options_of(
    session: &TestSession,
    database: DatabaseId,
    property: PropertyId,
) -> Vec<SelectOption> {
    let database = session.workspace().database(database).unwrap();
    let (_, found) = database
        .schema
        .properties
        .iter()
        .enumerate()
        .find(|(_, candidate)| candidate.id == property)
        .unwrap();
    match &found.kind {
        PropertyKind::Select { options } | PropertyKind::MultiSelect { options } => options.clone(),
        _ => Vec::new(),
    }
}

pub fn add_row(
    session: &mut TestSession,
    database: DatabaseId,
    title: &str,
    values: Vec<(PropertyId, PropertyValue)>,
) -> BlockId {
    let changes = run(session, Command::AddRow { database, values });
    let row = created_block(&changes).unwrap();
    run(
        session,
        Command::EditText {
            block: row,
            text: RichText::plain(title),
        },
    );
    row
}

pub fn embed_database(session: &mut TestSession) -> (BlockId, BlockId, DatabaseId, ViewId) {
    let page = add_page(session, Parent::Root, "Home");
    let intro = add_block(session, page, BlockKind::Text, "intro");
    let changes = run(session, Command::CreateDatabaseWithView { after: intro });
    let database = created_database(&changes).unwrap();
    let block = created_block(&changes).unwrap();
    let view = changes
        .created
        .iter()
        .find_map(|created| match created {
            Created::View(view) => Some(*view),
            _ => None,
        })
        .unwrap();
    (page, block, database, view)
}

pub fn tasks(session: &mut TestSession) -> Tasks {
    let (page, block, database, view) = embed_database(session);
    run(
        session,
        Command::RenameDatabase {
            database,
            name: "Tasks".to_owned(),
        },
    );
    let title = session
        .workspace()
        .database(database)
        .unwrap()
        .schema
        .properties[0]
        .id;
    let status = add_property(
        session,
        database,
        "Status",
        PropertyKind::Select {
            options: Vec::new(),
        },
    );
    let todo = add_option(session, database, status, "Todo", OptionColor::Gray);
    let doing = add_option(session, database, status, "Doing", OptionColor::Blue);
    let done = add_option(session, database, status, "Done", OptionColor::Green);
    let priority = add_property(
        session,
        database,
        "Priority",
        PropertyKind::Number {
            format: remindr_core::domain::NumberFormat::Plain,
        },
    );
    let rows = [
        ("Write spec", todo, 3.0),
        ("Build core", doing, 8.0),
        ("Ship UI", done, 5.0),
        ("Celebrate", done, 1.0),
    ]
    .into_iter()
    .map(|(name, option, number)| {
        add_row(
            session,
            database,
            name,
            vec![
                (status, PropertyValue::Select(option)),
                (priority, PropertyValue::Number(number)),
            ],
        )
    })
    .collect();
    Tasks {
        page,
        block,
        database,
        view,
        title,
        status,
        priority,
        todo,
        doing,
        done,
        rows,
    }
}

pub fn titles(
    model: &Entity<WorkspaceModel>,
    cx: &mut TestAppContext,
    view: ViewId,
) -> Vec<String> {
    model.read_with(cx, |model, _| {
        let workspace = model.workspace();
        workspace
            .query(view)
            .unwrap()
            .into_iter()
            .map(|row| workspace.block(row).unwrap().text.to_plain())
            .collect()
    })
}

pub fn view_count(
    model: &Entity<WorkspaceModel>,
    cx: &mut TestAppContext,
    block: BlockId,
) -> usize {
    model.read_with(cx, |model, _| model.workspace().views(block).count())
}

pub fn open<V: Render + 'static>(
    cx: &mut TestAppContext,
    model: &Entity<WorkspaceModel>,
    build: impl FnOnce(&mut Window, &mut App) -> Entity<V>,
) -> (Ui, Entity<V>) {
    let navigator = cx.new(|_| Navigator::new());
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::theme::apply(cx);
        cx.set_global(NavigatorHandle(navigator));
    });
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: Point::default(),
            size: size(px(1100.), px(700.)),
        })),
        ..Default::default()
    };
    let (handle, view) = cx.update(|cx| gpui_kit::open_window(options, cx, build).unwrap());
    let ui = Ui {
        handle,
        model: model.clone(),
    };
    ui.with(cx, |window, _| window.activate_window());
    ui.settle(cx);
    (ui, view)
}

pub fn open_block(
    cx: &mut TestAppContext,
    model: &Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
) -> Ui {
    let shared = model.clone();
    let (ui, _) = open(cx, model, move |window, cx| {
        cx.new(|cx| DatabaseViewBlock::new(shared, block, database, window, cx))
    });
    ui
}

pub struct Narrow {
    inner: gpui_kit::AnyView,
    width: gpui_kit::Pixels,
}

impl Render for Narrow {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        div().w(self.width).child(self.inner.clone())
    }
}

pub fn open_block_in_width(
    cx: &mut TestAppContext,
    model: &Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
    width: gpui_kit::Pixels,
) -> Ui {
    let shared = model.clone();
    let (ui, _) = open(cx, model, move |window, cx| {
        let inner: gpui_kit::AnyView = cx
            .new(|cx| DatabaseViewBlock::new(shared, block, database, window, cx))
            .into();
        cx.new(|_| Narrow { inner, width })
    });
    ui
}

pub struct Paged {
    inner: gpui_kit::AnyView,
    width: gpui_kit::Pixels,
    scroll: gpui_kit::ScrollHandle,
}

impl Render for Paged {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{
            InteractiveElement as _, ParentElement as _, StatefulInteractiveElement as _,
            Styled as _, px,
        };
        div()
            .id("page")
            .w(self.width)
            .h(px(420.))
            .overflow_y_scroll()
            .track_scroll(&self.scroll)
            .child(self.inner.clone())
            .child(div().h(px(2000.)))
    }
}

pub fn open_block_in_page(
    cx: &mut TestAppContext,
    model: &Entity<WorkspaceModel>,
    block: BlockId,
    database: DatabaseId,
    width: gpui_kit::Pixels,
) -> (Ui, gpui_kit::ScrollHandle) {
    let shared = model.clone();
    let scroll = gpui_kit::ScrollHandle::new();
    let held = scroll.clone();
    let (ui, _) = open(cx, model, move |window, cx| {
        let inner: gpui_kit::AnyView = cx
            .new(|cx| DatabaseViewBlock::new(shared, block, database, window, cx))
            .into();
        cx.new(|_| Paged {
            inner,
            width,
            scroll: held,
        })
    });
    (ui, scroll)
}

pub struct Stack {
    views: Vec<gpui_kit::AnyView>,
}

impl Render for Stack {
    fn render(
        &mut self,
        _: &mut Window,
        _: &mut gpui_kit::Context<Self>,
    ) -> impl gpui_kit::IntoElement {
        use gpui_kit::{ParentElement as _, Styled as _};
        gpui_kit::component::v_flex()
            .size_full()
            .children(self.views.iter().cloned())
    }
}

pub fn open_blocks(
    cx: &mut TestAppContext,
    model: &Entity<WorkspaceModel>,
    blocks: &[(BlockId, DatabaseId)],
) -> Ui {
    let shared = model.clone();
    let wanted = blocks.to_vec();
    let (ui, _) = open(cx, model, move |window, cx| {
        let views: Vec<gpui_kit::AnyView> = wanted
            .iter()
            .map(|(block, database)| {
                let model = shared.clone();
                let (block, database) = (*block, *database);
                cx.new(|cx| DatabaseViewBlock::new(model, block, database, window, cx))
                    .into()
            })
            .collect();
        cx.new(|_| Stack { views })
    });
    ui
}

pub trait UiExt {
    fn stored(
        &self,
        cx: &mut TestAppContext,
        row: BlockId,
        property: PropertyId,
    ) -> Option<PropertyValue>;
    fn right_click(&self, cx: &mut TestAppContext, id: impl Into<ElementId>);
    fn drag_to(
        &self,
        cx: &mut TestAppContext,
        from: impl Into<ElementId>,
        to: impl Into<ElementId>,
    );
    fn exists(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) -> bool;
    fn pick_item(&self, cx: &mut TestAppContext, scope: &'static str, item: impl Into<ElementId>);
}

impl UiExt for Ui {
    fn stored(
        &self,
        cx: &mut TestAppContext,
        row: BlockId,
        property: PropertyId,
    ) -> Option<PropertyValue> {
        value_of(&self.model, cx, row, property)
    }

    fn right_click(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) {
        let id = id.into();
        self.with(cx, |window, cx| window.right_click(id, cx));
        self.settle(cx);
    }

    fn drag_to(
        &self,
        cx: &mut TestAppContext,
        from: impl Into<ElementId>,
        to: impl Into<ElementId>,
    ) {
        let (from, to) = (from.into(), to.into());
        self.with(cx, |window, cx| window.drag_to(from, to, cx));
        self.settle(cx);
    }

    fn exists(&self, cx: &mut TestAppContext, id: impl Into<ElementId>) -> bool {
        self.find(cx, id).is_some()
    }

    fn pick_item(&self, cx: &mut TestAppContext, scope: &'static str, item: impl Into<ElementId>) {
        let item = item.into();
        self.with(cx, |window, cx| window.within(scope).click(item, cx));
        self.settle(cx);
    }
}

pub struct One {
    pub database: DatabaseId,
    pub property: PropertyId,
    pub row: BlockId,
}

pub fn one(session: &mut TestSession, kind: PropertyKind, value: Option<PropertyValue>) -> One {
    let changes = run(session, Command::CreateDatabase);
    let database = created_database(&changes).unwrap();
    let property = add_property(session, database, "Field", kind);
    let row = add_row(session, database, "Row", Vec::new());
    if value.is_some() {
        run(
            session,
            Command::SetValue {
                row,
                property,
                value,
            },
        );
    }
    One {
        database,
        property,
        row,
    }
}

pub fn value_of(
    model: &Entity<WorkspaceModel>,
    cx: &mut TestAppContext,
    row: BlockId,
    property: PropertyId,
) -> Option<PropertyValue> {
    model.read_with(cx, |model, _| {
        model.workspace().value(row, property).cloned()
    })
}

#[cfg(test)]
mod seeding {
    use anyhow::Context as _;
    use remindr_core::application::{Command, DropTarget};
    use remindr_core::domain::{
        BlockKind, CalendarDate, NumberFormat, OptionColor, Parent, PropertyKind, PropertyValue,
        ViewKind,
    };
    use remindr_store::SqliteStore;

    use super::{add_option, add_property, add_row, run};
    use crate::app::AppStore;
    use crate::app::commands::{created_block, created_database};
    use crate::app::startup::open_session;

    #[test]
    #[ignore = "writes the workspace file named by REMINDR_WORKSPACE"]
    fn seed_the_manual_workspace() {
        let path = std::path::PathBuf::from(std::env::var_os("REMINDR_WORKSPACE").unwrap());
        let mut session = open_session(|| {
            SqliteStore::open(&path)
                .map(AppStore::Sqlite)
                .with_context(|| format!("cannot open {}", path.display()))
        })
        .unwrap();
        let page = session
            .workspace()
            .children(Parent::Root)
            .next()
            .map(|block| block.id)
            .unwrap();
        let intro = created_block(&run(
            &mut session,
            Command::InsertBlockAt {
                target: DropTarget {
                    parent: Parent::Block(page),
                    after: None,
                },
                kind: BlockKind::Text,
            },
        ))
        .unwrap();
        run(
            &mut session,
            Command::EditText {
                block: intro,
                text: remindr_core::domain::RichText::plain("Everything we are shipping:"),
            },
        );
        let changes = run(
            &mut session,
            Command::CreateDatabaseWithView { after: intro },
        );
        let database = created_database(&changes).unwrap();
        let block = created_block(&changes).unwrap();
        run(
            &mut session,
            Command::RenameDatabase {
                database,
                name: "Project tracker".to_owned(),
            },
        );
        let status = add_property(
            &mut session,
            database,
            "Status",
            PropertyKind::Select {
                options: Vec::new(),
            },
        );
        let todo = add_option(&mut session, database, status, "Todo", OptionColor::Gray);
        let doing = add_option(
            &mut session,
            database,
            status,
            "In progress",
            OptionColor::Blue,
        );
        let done = add_option(&mut session, database, status, "Done", OptionColor::Green);
        let tags = add_property(
            &mut session,
            database,
            "Tags",
            PropertyKind::MultiSelect {
                options: Vec::new(),
            },
        );
        let design = add_option(&mut session, database, tags, "Design", OptionColor::Pink);
        let backend = add_option(&mut session, database, tags, "Backend", OptionColor::Orange);
        let urgent = add_option(&mut session, database, tags, "Urgent", OptionColor::Red);
        let budget = add_property(
            &mut session,
            database,
            "Budget",
            PropertyKind::Number {
                format: NumberFormat::Euro,
            },
        );
        let progress = add_property(
            &mut session,
            database,
            "Progress",
            PropertyKind::Number {
                format: NumberFormat::Percent,
            },
        );
        let due = add_property(&mut session, database, "Due", PropertyKind::Date);
        let shipped = add_property(&mut session, database, "Shipped", PropertyKind::Checkbox);
        let link = add_property(&mut session, database, "Link", PropertyKind::Url);
        let notes = add_property(&mut session, database, "Notes", PropertyKind::Text);
        let rows = [
            (
                "Write the spec",
                todo,
                vec![design],
                1200.0,
                10.0,
                (2026, 11, 2),
                false,
            ),
            (
                "Build the core",
                doing,
                vec![backend],
                8400.5,
                65.0,
                (2026, 11, 14),
                false,
            ),
            (
                "Database views",
                doing,
                vec![design, backend],
                3000.0,
                40.0,
                (2026, 11, 21),
                false,
            ),
            (
                "Editor polish",
                todo,
                vec![urgent],
                950.0,
                0.0,
                (2026, 12, 1),
                false,
            ),
            (
                "Sidebar",
                done,
                vec![design],
                600.0,
                100.0,
                (2026, 10, 20),
                true,
            ),
            (
                "Local storage",
                done,
                vec![backend],
                2100.0,
                100.0,
                (2026, 10, 25),
                true,
            ),
            (
                "Release notes",
                todo,
                Vec::new(),
                150.0,
                5.0,
                (2026, 12, 15),
                false,
            ),
            (
                "Launch party",
                todo,
                vec![urgent, design],
                4800.0,
                0.0,
                (2027, 1, 10),
                false,
            ),
        ];
        for (title, option, labels, money, percent, (year, month, day), shipped_value) in rows {
            add_row(
                &mut session,
                database,
                title,
                vec![
                    (status, PropertyValue::Select(option)),
                    (tags, PropertyValue::MultiSelect(labels)),
                    (budget, PropertyValue::Number(money)),
                    (progress, PropertyValue::Number(percent)),
                    (
                        due,
                        PropertyValue::Date(CalendarDate::new(year, month, day).unwrap()),
                    ),
                    (shipped, PropertyValue::Checkbox(shipped_value)),
                    (
                        link,
                        PropertyValue::Url("https://remindr.example/spec".to_owned()),
                    ),
                    (notes, PropertyValue::Text("Plan, build, ship".to_owned())),
                ],
            );
        }
        let first = session
            .workspace()
            .views(block)
            .next()
            .map(|view| view.id)
            .unwrap();
        let kind = ViewKind::Board { group_by: status };
        if std::env::var_os("REMINDR_SEED_BOARD_FIRST").is_some() {
            run(&mut session, Command::SetViewKind { view: first, kind });
        } else {
            run(
                &mut session,
                Command::AddView {
                    block,
                    name: "Board".to_owned(),
                    kind,
                },
            );
        }
    }
}
