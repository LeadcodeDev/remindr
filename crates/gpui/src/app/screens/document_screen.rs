use std::time::Duration;

use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    input::Input,
    scroll::ScrollableElement,
    tab::{Tab, TabBar},
};
use gpui_nav::{Screen, ScreenContext};

use crate::{
    LoadingState,
    app::{
        components::node_code_renderer::NodeCodeRenderer,
        states::{
            app_state::AppState,
            document_state::{DocumentContent, DocumentState, OpenedDocument, PersistenceState},
            repository_state::RepositoryState,
        },
    },
};

pub struct DocumentScreen {
    _ctx: ScreenContext<AppState>,
    show_code: bool,
    initialized: bool,
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
            initialized: false,
        }
    }

    fn ensure_initialized(&mut self, cx: &mut Context<Self>) {
        if !self.initialized {
            self.initialized = true;
            // Observe global DocumentState changes to re-render when document is loaded
            cx.observe_global::<DocumentState>(|_, cx| {
                cx.notify();
            })
            .detach();
        }
    }

    fn toggle_code_mode(this: &mut Self, _: &ClickEvent, _: &mut Window, cx: &mut Context<Self>) {
        this.show_code = !this.show_code;
        cx.notify();
    }

    fn load_document_if_needed(&self, window: &mut Window, cx: &mut Context<Self>) {
        let (needs_loading, document_id) = cx.read_global::<DocumentState, _>(|state, _| {
            let id = state.current_opened_document;
            let needs = id.map(|id| state.needs_loading(id)).unwrap_or(false);
            (needs, id)
        });

        if needs_loading {
            if let Some(doc_id) = document_id {
                // Mark as loading in progress to prevent duplicate loads
                cx.update_global::<DocumentState, _>(|state, _| {
                    state.set_loading_in_progress(doc_id, true);
                });

                let repository = cx.global::<RepositoryState>().documents.clone();
                let window_handle = window.window_handle();

                cx.spawn(async move |_, cx| {
                    let result = repository.get_document_by_id(doc_id).await;

                    match result {
                        Ok(document) => {
                            let update_result = cx.update_window(window_handle, |_, window, cx| {
                                cx.update_global::<DocumentState, _>(|state, cx| {
                                    state.set_document_content(doc_id, document, window, cx);
                                    state.set_loading_in_progress(doc_id, false);
                                });
                            });

                            // If window update failed, try to update via cx.update
                            if update_result.is_err() {
                                let _ = cx.update(|cx| {
                                    cx.update_global::<DocumentState, _>(|state, _| {
                                        state.set_loading_in_progress(doc_id, false);
                                        state.set_document_error(
                                            doc_id,
                                            "Failed to update window".to_string(),
                                        );
                                    });
                                });
                            }
                        }
                        Err(e) => {
                            let _ = cx.update(|cx| {
                                cx.update_global::<DocumentState, _>(|state, _| {
                                    state.set_loading_in_progress(doc_id, false);
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
        self.ensure_initialized(cx);
        self.load_document_if_needed(window, cx);

        let (documents, current_document, current_index, is_saving, can_go_previous, can_go_next) =
            cx.read_global::<DocumentState, _>(|state, _| {
                let documents: Vec<OpenedDocument> = state.documents.clone();
                let current_document = state.get_current_document().cloned();
                let current_index = state.get_current_document_index();
                let is_saving = state.persistence == PersistenceState::Pending;
                let can_go_previous = current_index.map(|i| i > 0).unwrap_or(false);
                let can_go_next = current_index
                    .map(|i| i < documents.len().saturating_sub(1))
                    .unwrap_or(false);

                (
                    documents,
                    current_document,
                    current_index,
                    is_saving,
                    can_go_previous,
                    can_go_next,
                )
            });

        div()
            .w_full()
            .h_full()
            .relative()
            .when(is_saving, |this| {
                this.child(
                    div().absolute().bottom_4().right_4().child(
                        Icon::default()
                            .path("icons/loader-circle.svg")
                            .size_4()
                            .with_animation(
                                "rotate-loader",
                                Animation::new(Duration::from_secs(1)).repeat(),
                                |icon, delta| {
                                    icon.transform(Transformation::rotate(percentage(delta)))
                                },
                            ),
                    ),
                )
            })
            .when(!documents.is_empty(), |this| {
                this.child(
                    TabBar::new("tabs")
                        .prefix(
                            div()
                                .px_1()
                                .flex()
                                .items_center()
                                .child(
                                    Button::new("nav-previous")
                                        .xsmall()
                                        .ghost()
                                        .when(can_go_previous, |this| this.cursor_pointer())
                                        .icon(Icon::default().path("icons/chevron-left.svg"))
                                        .disabled(!can_go_previous)
                                        .tooltip("Previous tab")
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                if let Some(index) =
                                                    state.get_current_document_index()
                                                {
                                                    if index > 0 {
                                                        if let Some(doc) =
                                                            state.documents.get(index - 1)
                                                        {
                                                            state.current_opened_document =
                                                                Some(doc.uid);
                                                        }
                                                    }
                                                }
                                            });
                                        })),
                                )
                                .child(
                                    Button::new("nav-next")
                                        .xsmall()
                                        .ghost()
                                        .when(can_go_next, |this| this.cursor_pointer())
                                        .icon(Icon::default().path("icons/chevron-right.svg"))
                                        .disabled(!can_go_next)
                                        .tooltip("Next tab")
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                let current_index =
                                                    state.get_current_document_index();

                                                if let Some(index) = current_index {
                                                    if index < state.documents.len() - 1 {
                                                        let document =
                                                            state.documents.get(index + 1);
                                                        if let Some(doc) = document {
                                                            state.current_opened_document =
                                                                Some(doc.uid);
                                                        }
                                                    }
                                                }
                                            });
                                        })),
                                ),
                        )
                        .suffix(
                            div().px_4().flex().items_center().child(
                                Button::new("toggle-code-btn")
                                    .xsmall()
                                    .ghost()
                                    .cursor_pointer()
                                    .icon(Icon::default().path("icons/braces.svg"))
                                    .tooltip("Toggle code view")
                                    .on_click(cx.listener(Self::toggle_code_mode)),
                            ),
                        )
                        .selected_index(current_index.unwrap_or(0))
                        .on_click(cx.listener(|_, index: &usize, _, cx| {
                            cx.update_global::<DocumentState, _>(|state, _| {
                                if let Some(doc) = state.documents.get(*index) {
                                    state.current_opened_document = Some(doc.uid);
                                }
                            });
                        }))
                        .children(documents.iter().map(|element| {
                            Tab::new()
                                .bg(cx.theme().background.lighten(0.2))
                                .cursor_pointer()
                                .label(element.title.clone())
                                .suffix(
                                    Button::new("btn")
                                        .xsmall()
                                        .mr_2()
                                        .cursor_pointer()
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
                .child(self.render_document_content(current_document))
            })
            .when(documents.is_empty(), |this| this.child(DocumentStateEmpty))
    }
}

impl DocumentScreen {
    fn render_document_content(
        &self,
        current_document: Option<OpenedDocument>,
    ) -> impl IntoElement {
        match current_document {
            Some(doc) => match &doc.state {
                LoadingState::Loading => DocumentLoading.into_any_element(),
                LoadingState::Loaded(content) => DocumentStateLoaded {
                    content: content.clone(),
                    show_code: self.show_code,
                }
                .into_any_element(),
                LoadingState::Error(error) => DocumentLoadingError {
                    error: error.to_string(),
                }
                .into_any_element(),
            },
            None => DocumentStateEmpty.into_any_element(),
        }
    }
}

#[derive(IntoElement)]
struct DocumentLoading;
impl RenderOnce for DocumentLoading {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .bg(cx.theme().background.lighten(0.2))
            .flex()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .child("Loading...")
    }
}

#[derive(IntoElement)]
struct DocumentLoadingError {
    error: String,
}

impl RenderOnce for DocumentLoadingError {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .bg(cx.theme().background.lighten(0.2))
            .flex()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .child(self.error)
    }
}

#[derive(IntoElement)]
struct DocumentStateLoaded {
    content: DocumentContent,
    show_code: bool,
}

impl RenderOnce for DocumentStateLoaded {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .bg(cx.theme().background.lighten(0.2))
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .child(
                div()
                    .flex()
                    .gap_10()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .child(
                        div()
                            .max_w(px(820.0))
                            .w_full()
                            .mx_auto()
                            .py_5()
                            .child(
                                Input::new(&self.content.title_input)
                                    .appearance(false)
                                    .text_3xl()
                                    .ml_10()
                                    .large(),
                            )
                            .child(self.content.renderer.clone()),
                    )
                    .when(self.show_code, |this| {
                        let nodes = self
                            .content
                            .renderer
                            .read(cx)
                            .state
                            .read(cx)
                            .get_nodes()
                            .clone();
                        this.child(NodeCodeRenderer::new(nodes, window, cx))
                    }),
            )
    }
}

#[derive(IntoElement)]
struct DocumentStateEmpty;
impl RenderOnce for DocumentStateEmpty {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .bg(cx.theme().background.lighten(0.2))
            .flex()
            .w_full()
            .h_full()
            .items_center()
            .justify_center()
            .child("No element selected")
    }
}
