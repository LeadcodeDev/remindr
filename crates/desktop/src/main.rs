#![expect(dead_code)]

mod app;
mod database;
mod editor;
mod embed;
mod keymap;
mod page;
mod shell;
mod sidebar;
mod theme;

use std::process::ExitCode;

use anyhow::Context as _;
use gpui_kit::component::TitleBar;
use gpui_kit::{
    App, AppContext as _, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use remindr_core::application::Session;
use remindr_store::SqliteStore;

use app::startup::{open_session, workspace_path};
use app::{AppStore, SystemClock, WorkspaceModel};
use shell::Shell;

const WINDOW_TITLE: &str = "Remindr";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("remindr: {error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let path = workspace_path(
        std::env::var_os("REMINDR_WORKSPACE"),
        std::env::var_os("HOME"),
    )?;
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory)
            .with_context(|| format!("cannot create {}", directory.display()))?;
    }
    let session = open_session(|| {
        SqliteStore::open(&path)
            .map(AppStore::Sqlite)
            .with_context(|| format!("cannot open {}", path.display()))
    })?;
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| start(session, cx));
    Ok(())
}

fn start(session: Session<AppStore, SystemClock>, cx: &mut App) {
    gpui_kit::init(cx);
    editor::init(cx);
    database::init(cx);
    keymap::init(cx);
    cx.on_window_closed(|cx, _| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();
    let model = cx.new(|_| WorkspaceModel::new(session));
    let opened = gpui_kit::open_window(window_options(cx), cx, |window, cx| {
        cx.new(|cx| Shell::new(model, window, cx))
    });
    if let Err(error) = opened {
        eprintln!("remindr: cannot open the window: {error:#}");
        cx.quit();
    }
}

fn window_options(cx: &App) -> WindowOptions {
    let mut options = TitleBar::window_options();
    options.titlebar = Some(TitlebarOptions {
        title: Some(WINDOW_TITLE.into()),
        ..TitleBar::title_bar_options()
    });
    options.window_bounds = Some(WindowBounds::Windowed(Bounds::centered(
        None,
        size(px(1180.), px(760.)),
        cx,
    )));
    options.window_min_size = Some(size(px(720.), px(480.)));
    options
}
