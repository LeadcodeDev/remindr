use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    notification::{Notification, NotificationType},
    tab::{Tab, TabBar},
};
use gpui_nav::{Screen, ScreenContext};

use crate::app::{
    components::node_code_renderer::NodeCodeRenderer,
    states::{app_state::AppState, document_state::DocumentState},
};

pub struct DocumentScreen {
    _ctx: ScreenContext<AppState>,
    show_code: bool,
}

impl Screen for DocumentScreen {
    fn id(&self) -> &'static str {
        "Documents"
    }
}

impl DocumentScreen {
    pub fn new(app_state: WeakEntity<AppState>) -> Self {
        Self {
            _ctx: ScreenContext::new(app_state),
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
        let (documents, current_document, current_index, pending_notification) = cx
            .read_global::<DocumentState, _>(|state, _| {
                let documents = state.documents.clone().into_iter().collect::<Vec<_>>();
                let current_document = state.current_document.clone();
                let current_index = state.get_current_document_index();
                let pending_notification = state.pending_notification;

                (
                    documents,
                    current_document,
                    current_index,
                    pending_notification,
                )
            });

        if pending_notification {
            window.push_notification(
                Notification::new()
                    .title("Update Available")
                    .message("A new version of the application is ready to install.")
                    .with_type(NotificationType::Info),
                cx,
            );

            // clear the flag so notification is shown only once
            cx.update_global::<DocumentState, _>(|state, _| {
                state.pending_notification = false;
            });
        }

        div()
            .w_full()
            .when(!documents.is_empty(), |this| {
                this.child(
                    TabBar::new("tabs")
                        .selected_index(current_index.unwrap_or(0))
                        .on_click(cx.listener(|_, index: &usize, _, cx| {
                            cx.update_global::<DocumentState, _>(|state, _| {
                                let docs = state.documents.clone().into_iter().collect::<Vec<_>>();
                                state.current_document = docs.get(*index).cloned();
                            });

                            cx.notify();
                        }))
                        .children(documents.iter().map(|element| {
                            Tab::new().label(element.title.clone()).suffix(
                                Button::new("btn")
                                    .xsmall()
                                    .mr_2()
                                    .icon(Icon::default().path("icons/x.svg"))
                                    .ghost()
                                    .tooltip("Close tab")
                                    .on_click({
                                        let element_id = element.uid.clone();
                                        cx.listener(move |_, _, _, cx| {
                                            let element_id = element_id.clone();
                                            cx.update_global::<DocumentState, _>(|state, cx| {
                                                let previous_document =
                                                    state.get_previous_document(element_id.clone());

                                                state.current_document = previous_document;
                                                state.remove_document(element_id);

                                                cx.notify();
                                            })
                                        })
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
                            div().max_w(px(820.0)).w_full().mx_auto().py_5().when_some(
                                current_document
                                    .clone()
                                    .and_then(|doc| doc.renderer.clone()),
                                |this, renderer| this.child(renderer),
                            ),
                        )
                        .when(self.show_code, |this| {
                            let nodes = current_document.clone().and_then(|document| {
                                document
                                    .renderer
                                    .clone()
                                    .map(|r| r.read(cx).state.read(cx).get_nodes().clone())
                            });

                            this.when_some(nodes, |this, nodes| {
                                this.child(NodeCodeRenderer::new(nodes, window, cx))
                            })
                        }),
                )
            })
            .when_none(&current_document, |this| this.child(DocumentStateEmpty))
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
