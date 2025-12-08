use std::borrow::Cow;

use anyhow::anyhow;
use gpui::*;
use gpui_component::{Root, theme};
use remindr_gpui::{
    entities::remindr::Remindr, screens::AppRouter, states::document_state::DocumentState,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

#[tokio::main]
async fn main() {
    let app = Application::new().with_assets(Assets);
    let remindr = Remindr::new();

    let settings = remindr.load_settings().await;

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_router::init(cx);
        theme::init(cx);

        if let Ok(settings) = settings {
            cx.set_global(settings);
        }

        cx.set_global(DocumentState {
            documents: Vec::new(),
        });

        cx.activate(true);

        let mut window_size = size(px(640.), px(480.));
        if let Some(display) = cx.primary_display() {
            let display_size = display.bounds().size;
            window_size.width = display_size.width * 0.85;
            window_size.height = display_size.height * 0.85;
        }
        let window_bounds = Bounds::centered(None, window_size, cx);

        cx.spawn(async move |cx| {
            let _ = remindr.init().await;

            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                window_min_size: Some(gpui::Size {
                    width: px(640.),
                    height: px(480.),
                }),
                kind: WindowKind::Normal,
                ..Default::default()
            };

            let window = cx
                .open_window(options, |window, cx| {
                    let view = cx.new(|cx| AppRouter::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                })
                .expect("failed to open window");

            window
                .update(cx, |_, window, _| {
                    window.activate_window();
                    window.set_window_title("Remindr");
                })
                .expect("failed to update window");

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
