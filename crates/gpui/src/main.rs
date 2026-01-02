use std::borrow::Cow;

use anyhow::Error;
use gpui::*;
use gpui_component::{
    Root,
    theme::{self, ThemeRegistry},
};
use remindr_gpui::{
    app::{
        apply_theme,
        components::rich_text,
        remindr::Remindr,
        screens::AppRouter,
        states::{document_state::DocumentState, repository_state::RepositoryState},
    },
    infrastructure::repositories::document_repository::DocumentRepository,
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

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_router::init(cx);
        theme::init(cx);
        rich_text::init(cx);

        // Load custom themes from the themes directory (~/.config/remindr/themes)
        let themes_dir = remindr
            .get_config_dir("remindr")
            .map(|p| p.join("themes"))
            .ok();

        if let Some(themes_dir) = themes_dir {
            if themes_dir.exists() {
                let _ = ThemeRegistry::watch_dir(themes_dir, cx, move |_cx| {
                    // Themes will be applied when settings are set
                });
            }
        }

        // Set settings as global (must be done before apply_theme)
        if let Ok(settings) = settings {
            cx.set_global(settings);
        }

        cx.set_global(RepositoryState {
            documents: DocumentRepository::new(pool.clone()),
        });

        cx.set_global(DocumentState::default());
        cx.activate(true);

        let mut window_size = size(px(640.), px(480.));
        if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            window_size.width = display_size.width * 0.85;
            window_size.height = display_size.height * 0.85;
        }
        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                window_min_size: Some(gpui::Size {
                    width: px(640.),
                    height: px(480.),
                }),
                kind: WindowKind::Normal,
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    title: Some("Remindr".into()),
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                }),
                ..Default::default()
            };

            let window = cx
                .open_window(options, |window, cx| {
                    let view = cx.new(AppRouter::new);
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open window");

            window
                .update(cx, |_, window, cx| {
                    window.activate_window();
                    window.set_window_title("Remindr");
                    // Apply theme after window is created
                    apply_theme(window, cx);
                })
                .expect("failed to update window");

            Ok::<_, anyhow::Error>(())
        })
        .detach();

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
