use std::{borrow::Cow, fs::read_to_string};

use anyhow::anyhow;
use gpui::*;
use gpui_component::Root;
use remindr_gpui::{
    components::node_renderer::NodeRenderer, screens::main_screen::MainScreen,
    states::document_state::ViewState,
};
use rust_embed::RustEmbed;
use serde_json::from_str;

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

fn main() {
    let app = Application::new().with_assets(Assets);

    app.run(move |cx| {
        gpui_component::init(cx);

        cx.set_global::<ViewState>(ViewState::default());
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
                ..Default::default()
            };

            let file_content =
                read_to_string("artifacts/demo.json").expect("Failed to read demo.json");
            let nodes = from_str(&file_content).expect("Failed to parse JSON from demo.json");

            let window = cx
                // .open_window(options, |window, cx| {
                //     let view = cx.new(|cx| MainScreen::new(window, cx));
                //     cx.new(|cx| Root::new(view.into(), window, cx))
                // })
                .open_window(options, |window, cx| {
                    let view = cx.new(|cx| NodeRenderer::new(nodes, window, cx));
                    cx.new(|cx| Root::new(view.into(), window, cx))
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
