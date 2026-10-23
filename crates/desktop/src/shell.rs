use gpui_kit::{Context, IntoElement, Render, TestSupportExt, Window, div, prelude::*, px};

pub struct Shell;

impl Shell {
    #[must_use]
    pub fn new(_: &mut Context<Self>) -> Self {
        Self
    }
}

impl Render for Shell {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_row()
            .child(
                div()
                    .id("sidebar")
                    .w(px(260.))
                    .h_full()
                    .flex_none()
                    .test_support(),
            )
            .child(div().id("main-area").h_full().flex_1().test_support())
    }
}

#[cfg(test)]
mod tests {
    use super::Shell;
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{
        AppContext, Bounds, Point, TestAppContext, WindowBounds, WindowOptions, px, size,
    };

    #[gpui_kit::test]
    fn shell_renders_sidebar_and_main_area(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(960.), px(640.)),
        };
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(Shell::new),
            )
            .expect("open test window")
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let sidebar = window.find("sidebar");
            let main_area = window.find("main-area");
            assert_eq!(sidebar.bounds().size.width, px(260.));
            assert!(main_area.bounds().size.width > px(0.));
            assert!(main_area.bounds().left() >= sidebar.bounds().right());
        })
        .expect("update test window");
    }
}
