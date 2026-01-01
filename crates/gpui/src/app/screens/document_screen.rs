use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    notification::{Notification, NotificationType},
    tab::{Tab, TabBar},
};
use gpui_nav::{Screen, ScreenContext};

use crate::{
    LoadingState,
    app::{
        components::node_code_renderer::NodeCodeRenderer,
        states::{
            app_state::AppState,
            document_state::{DocumentState, OpenedDocument},
            repository_state::RepositoryState,
        },
    },
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

    fn load_document_if_needed(&self, window: &mut Window, cx: &mut Context<Self>) {
        let needs_loading = cx.read_global::<DocumentState, _>(|state, _| {
            state
                .current_opened_document
                .map(|id| state.needs_loading(id))
                .unwrap_or(false)
        });

        if needs_loading {
            let document_id =
                cx.read_global::<DocumentState, _>(|state, _| state.current_opened_document);

            if let Some(doc_id) = document_id {
                let repository = cx.global::<RepositoryState>().documents.clone();
                let window_handle = window.window_handle();

                cx.spawn(async move |_, cx| {
                    let result = repository.get_document_by_id(doc_id).await;

                    match result {
                        Ok(document) => {
                            let _ = cx.update_window(window_handle, |_, window, cx| {
                                cx.update_global::<DocumentState, _>(|state, cx| {
                                    state.set_document_content(doc_id, document, window, cx);
                                });
                            });
                        }
                        Err(e) => {
                            let _ = cx.update(|cx| {
                                cx.update_global::<DocumentState, _>(|state, _| {
                                    state.set_document_error(doc_id, e.to_string());
                                });
                            });
                        }
                    }

                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
        }
    }
}

impl Render for DocumentScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Trigger loading if needed
        self.load_document_if_needed(window, cx);

        let (documents, current_document, current_index, pending_notification) = cx
            .read_global::<DocumentState, _>(|state, _| {
                let documents: Vec<OpenedDocument> = state.documents.clone();
                let current_document = state.get_current_document().cloned();
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
                    .title("Document saved")
                    .message("Your document has been saved successfully.")
                    .with_type(NotificationType::Info),
                cx,
            );

            cx.update_global::<DocumentState, _>(|state, _| {
                state.pending_notification = false;
            });
        }

        div()
            .w_full()
            .h_full()
            .when(!documents.is_empty(), |this| {
                this.child(
                    TabBar::new("tabs")
                        .selected_index(current_index.unwrap_or(0))
                        .on_click(cx.listener(|_, index: &usize, _, cx| {
                            cx.update_global::<DocumentState, _>(|state, _| {
                                if let Some(doc) = state.documents.get(*index) {
                                    state.current_opened_document = Some(doc.uid);
                                }
                            });
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
                                        let element_id = element.uid;
                                        cx.listener(move |_, _, _, cx| {
                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                let previous_document =
                                                    state.get_previous_document(element_id);

                                                state.current_opened_document =
                                                    previous_document.map(|doc| doc.uid);

                                                state.remove_document(element_id);
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
                .child(self.render_document_content(current_document, window, cx))
            })
            .when(documents.is_empty(), |this| this.child(DocumentStateEmpty))
    }
}

impl DocumentScreen {
    fn render_document_content(
        &self,
        current_document: Option<OpenedDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        match current_document {
            Some(doc) => match &doc.state {
                LoadingState::Loading => div()
                    .flex()
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .child("Loading..."),

                LoadingState::Loaded(content) => div()
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
                            .child(content.renderer.clone()),
                    )
                    .when(self.show_code, |this| {
                        let nodes = content.renderer.read(cx).state.read(cx).get_nodes().clone();
                        this.child(NodeCodeRenderer::new(nodes, window, cx))
                    }),

                LoadingState::Error(error) => div()
                    .flex()
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .child(format!("Error: {}", error)),
            },
            None => div()
                .flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .child("No document selected"),
        }
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
