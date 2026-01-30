#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::borrow::Cow;

use anyhow::Error;
use gpui::*;
use gpui_component::{
    Root,
    theme::{self, ThemeRegistry},
};
use remindr_gpui::{
    app::{
        apply_theme, apply_theme_global,
        components::rich_text,
        remindr::Remindr,
        screens::AppRouter,
        states::{
            database_state::DatabaseState, document_state::DocumentState,
            repository_state::RepositoryState, settings_state::Settings,
        },
    },
    infrastructure::repositories::{
        database_repository::DatabaseRepository, document_repository::DocumentRepository,
        folder_repository::FolderRepository,
    },
};
use rust_embed::RustEmbed;
use sqlx::{SqlitePool, migrate};

#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| Error::msg("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

actions!(window, [Quit]);

const MIN_WINDOW_SIZE: Size<Pixels> = Size {
    width: px(640.),
    height: px(480.),
};

fn create_window_options(bounds: Bounds<Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(MIN_WINDOW_SIZE),
        kind: WindowKind::Normal,
        titlebar: Some(TitlebarOptions {
            appears_transparent: true,
            title: Some("Remindr".into()),
            traffic_light_position: Some(point(px(9.0), px(9.0))),
        }),
        ..Default::default()
    }
}

fn compute_window_bounds(cx: &App) -> Bounds<Pixels> {
    let mut window_size = size(MIN_WINDOW_SIZE.width, MIN_WINDOW_SIZE.height);
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = display_size.width * 0.85;
        window_size.height = display_size.height * 0.85;
    }
    Bounds::centered(None, window_size, cx)
}

fn open_main_window(cx: &mut App) -> anyhow::Result<WindowHandle<Root>> {
    let bounds = compute_window_bounds(cx);
    cx.open_window(create_window_options(bounds), |window, cx| {
        let view = cx.new(AppRouter::new);
        cx.new(|cx| Root::new(view, window, cx))
    })
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    let app = Application::new().with_assets(Assets);
    let remindr = Remindr::new();

    let settings = remindr.load_settings().await;

    let _ = remindr.init().await;
    let database_path = remindr.init_default_database().await;

    let pool = if let Ok(database_path) = database_path {
        let database_url = format!("sqlite://{}", database_path.to_str().unwrap());
        SqlitePool::connect(&database_url).await?
    } else {
        panic!("Failed to initialize database");
    };

    migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|err| Error::msg(err.to_string()))?;

    app.on_reopen(|cx| {
        if let Some(window) = cx.active_window() {
            window
                .update(cx, |_, window, _cx| {
                    window.activate_window();
                })
                .ok();
        } else {
            open_main_window(cx).ok();
        }
    });

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_router::init(cx);
        theme::init(cx);
        rich_text::init(cx);

        // Set settings as global (must be done before apply_theme)
        if let Ok(settings) = settings {
            cx.set_global(settings);
        }

        cx.set_global(RepositoryState {
            documents: DocumentRepository::new(pool.clone()),
            folders: FolderRepository::new(pool.clone()),
            databases: DatabaseRepository::new(pool.clone()),
        });

        cx.set_global(DocumentState::default());
        cx.set_global(DatabaseState::default());
        cx.activate(true);

        let window = open_main_window(cx).expect("failed to open window");

        // Load custom themes from the themes directory (~/.config/remindr/themes)
        let themes_dir = remindr
            .get_config_dir("remindr")
            .map(|p| p.join("themes"))
            .ok();

        if let Some(themes_dir) = themes_dir {
            if themes_dir.exists() {
                let _ = ThemeRegistry::watch_dir(themes_dir, cx, move |cx| {
                    // Apply theme after custom themes are loaded
                    apply_theme_global(cx);
                });
            }
        }

        window
            .update(cx, |_, window, cx| {
                window.activate_window();
                window.set_window_title("Remindr");
                apply_theme(window, cx);
            })
            .expect("failed to update window");

        // Watch settings.json for external changes
        if let Ok(config_dir) = remindr.get_config_dir("remindr") {
            let settings_file = config_dir.join("settings.json");
            cx.spawn(async move |cx| {
                let mut last_modified = std::fs::metadata(&settings_file)
                    .and_then(|m| m.modified())
                    .ok();

                loop {
                    smol::Timer::after(std::time::Duration::from_secs(2)).await;

                    let current_modified = std::fs::metadata(&settings_file)
                        .and_then(|m| m.modified())
                        .ok();

                    if current_modified != last_modified {
                        last_modified = current_modified;

                        if let Ok(content) = std::fs::read_to_string(&settings_file) {
                            if let Ok(new_settings) = serde_json::from_str::<Settings>(&content) {
                                let result = cx.update(|cx| {
                                    cx.update_global::<Settings, _>(|settings, _| {
                                        *settings = new_settings;
                                    });
                                    apply_theme_global(cx);
                                });
                                if result.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            })
            .detach();
        }

        set_app_menus(cx);
        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
    });

    Ok(())
}

fn set_app_menus(cx: &mut App) {
    cx.set_dock_menu(vec![
        MenuItem::os_submenu("Services", SystemMenuType::Services),
        MenuItem::separator(),
        MenuItem::action("Quit", Quit),
    ]);

    cx.set_menus(vec![Menu {
        name: "set_menus".into(),
        items: vec![
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Quit", Quit),
        ],
    }]);
}
