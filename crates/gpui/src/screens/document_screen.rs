use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
};

use crate::{
    components::node_code_renderer::NodeCodeRenderer,
    states::document_state::{Document, DocumentState},
};

pub struct DocumentScreen {
    current_index: usize,
    current_document: Option<Document>,
    show_code: bool,
}

impl DocumentScreen {
    pub fn new(_: &mut Context<Self>) -> Self {
        Self {
            current_index: 0,
            current_document: None,
            show_code: false,
        }
    }

    fn toggle_code_mode(this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        this.show_code = !this.show_code;
        cx.notify();
    }
}

impl Render for DocumentScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let documents = cx.read_global::<DocumentState, _>(|state, _| {
            state.documents.clone().into_iter().collect::<Vec<_>>()
        });

        if self.current_document.is_none() {
            self.current_document = documents.get(0).cloned();
        }

        div()
            .w_full()
            .when(!documents.is_empty(), |this| {
                this.child(
                    TabBar::new("tabs")
                        .selected_index(self.current_index)
                        .on_click(cx.listener(|this, index: &usize, _, cx| {
                            let documents = cx.read_global::<DocumentState, _>(|state, _| {
                                state.documents.clone().into_iter().collect::<Vec<_>>()
                            });

                            this.current_index = *index;
                            this.current_document = documents.get(*index).cloned();

                            cx.notify();
                        }))
                        .children(documents.iter().map(|element| {
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
                        })),
                )
                .child(
                    div()
                        .border_b_1()
                        .border_color(cx.theme().border)
                        .h_8()
                        .flex()
                        .justify_between()
                        .items_center()
                        .px_3()
                        .child("")
                        .child(
                            div().child(
                                Button::new("btn")
                                    .xsmall()
                                    .compact()
                                    .icon(Icon::default().path("icons/braces.svg"))
                                    .on_click(cx.listener(Self::toggle_code_mode)),
                            ),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_10()
                        .h_full()
                        .w_full()
                        .child(
                            div()
                                .max_w(px(820.0))
                                .w_full()
                                .mx_auto()
                                .py_5()
                                .when_some(self.current_document.clone(), |this, element| {
                                    this.child(element.renderer.clone())
                                }),
                        )
                        .when(self.show_code, |this| {
                            let nodes = self.current_document.clone().map(|document| {
                                document
                                    .renderer
                                    .read(cx)
                                    .state
                                    .read(cx)
                                    .get_nodes()
                                    .clone()
                            });

                            this.when_some(nodes, |this, nodes| {
                                this.child(NodeCodeRenderer::new(nodes, window, cx))
                            })
                        }),
                )
            })
            .when_none(&self.current_document, |this| {
                this.p_10().child(DocumentStateEmpty)
            })
    }
}

#[derive(IntoElement)]
struct DocumentStateEmpty;
impl RenderOnce for DocumentStateEmpty {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .flex()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .child("No element selected")
    }
}
