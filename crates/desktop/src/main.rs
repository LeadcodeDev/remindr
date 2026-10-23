#![expect(dead_code, unused_imports)]

mod app;
mod database;
mod editor;
mod embed;
mod shell;

use gpui_kit::{AppContext, WindowOptions};

use shell::Shell;

fn main() {
    gpui_kit::application().run(|cx| {
        gpui_kit::init(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let opened =
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(Shell::new));
        if let Err(error) = opened {
            eprintln!("window open failed: {error}");
            cx.quit();
        }
    });
}
