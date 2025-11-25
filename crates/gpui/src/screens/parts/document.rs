use gpui::{Context, DragMoveEvent, Window, div, prelude::*, px};
use gpui_component::ActiveTheme;

use crate::entities::ui::nodes::RemindrElement;

pub struct Document;

impl Render for Document {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .justify_center()
            .bg(cx.theme().background.opacity(0.8))
            .child(div())
    }
}
