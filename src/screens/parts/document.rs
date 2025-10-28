use gpui::{Context, DragMoveEvent, Window, div, prelude::*, px};
use gpui_component::ActiveTheme;

use crate::{entities::ui::elements::RemindrElement, states::document_state::ViewState};

pub struct Document;

impl Render for Document {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<ViewState>().current.as_ref().unwrap();

        div()
            .flex()
            .flex_1()
            .justify_center()
            .bg(cx.theme().background.opacity(0.8))
            .child(
                div()
                    .max_w(px(820.0))
                    .w_full()
                    .on_drag_move(cx.listener(
                        move |_, event: &DragMoveEvent<RemindrElement>, _, cx| {
                            let state = cx.global_mut::<ViewState>().current.as_mut().unwrap();
                            let is_outside = state.drag_controller.on_outside(event);

                            if is_outside {
                                cx.notify();
                            }
                        },
                    ))
                    .children(state.elements.iter().map(|node| node.element.clone())),
            )
    }
}
