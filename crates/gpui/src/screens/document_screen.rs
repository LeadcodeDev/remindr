use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    Icon, Sizable,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
};

use crate::states::document_state::DocumentState;

pub struct DocumentScreen {
    current_index: usize,
}

impl DocumentScreen {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self { current_index: 0 }
    }
}

impl Render for DocumentScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let documents = cx.read_global::<DocumentState, _>(|state, _| {
            state.documents.clone().into_iter().collect::<Vec<_>>()
        });

        let current_renderer = documents.get(self.current_index);

        div()
            .w_full()
            .child(
                TabBar::new("tabs")
                    .selected_index(self.current_index)
                    .on_click(cx.listener(|this, index: &usize, _, cx| {
                        this.current_index = *index;
                        cx.notify();
                    }))
                    .when(!documents.is_empty(), |this| {
                        this.children(documents.iter().map(|element| {
                            Tab::new().label(element.uid.clone()).suffix(
                                Button::new("btn")
                                    .xsmall()
                                    .mr_2()
                                    .icon(Icon::default().path("icons/x.svg"))
                                    .ghost()
                                    .tooltip("Close tab")
                                    .on_click({
                                        let element_id = element.uid.clone();
                                        move |_, _, cx| {
                                            let element_id = element_id.clone();
                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                state.remove_document(element_id);
                                            })
                                        }
                                    }),
                            )
                        }))
                    })
                    .when(documents.is_empty(), |this| this.child(Tab::default())),
            )
            .child(
                div()
                    .max_w(px(820.0))
                    .w_full()
                    .mx_auto()
                    .py_5()
                    .when_some(current_renderer, |this, element| {
                        this.child(element.renderer.clone())
                    })
                    .when_none(&current_renderer, |this| this.child("No element selected")),
            )
    }
}
