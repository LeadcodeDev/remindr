use std::time::Duration;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::{
    LoadingState,
    app::{
        components::confirm_dialog::ConfirmDialog,
        screens::document_screen::DocumentScreen,
        states::{
            app_state::AppState, document_state::DocumentState, repository_state::RepositoryState,
        },
    },
    domain::database::document::DocumentModel,
};

pub struct AppSidebar {
    document_state: LoadingState<Vec<DocumentModel>>,
    app_state: Entity<AppState>,
}

impl AppSidebar {
    pub fn new(app_state: Entity<AppState>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let document_repository = cx.global::<RepositoryState>().documents.clone();

            // Initial fetch
            cx.spawn({
                let repository = document_repository.clone();
                async move |this, cx| {
                    let documents = repository.get_documents().await;
                    if let Ok(documents) = documents {
                        let _ = this.update(cx, |state: &mut Self, _| {
                            state.document_state = LoadingState::Loaded(documents);
                        });
                    }
                }
            })
            .detach();

            // Poll every 1 seconds
            cx.spawn({
                let repository = document_repository.clone();
                async move |this, cx| {
                    loop {
                        smol::Timer::after(Duration::from_secs(1)).await;
                        let documents = repository.get_documents().await;
                        if let Ok(documents) = documents {
                            let result = this.update(cx, |state: &mut Self, _| {
                                state.document_state = LoadingState::Loaded(documents);
                            });
                            if result.is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .detach();

            Self {
                document_state: LoadingState::Loading,
                app_state,
            }
        })
    }
}

impl Render for AppSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = cx.theme().sidebar;
        let border_color = cx.theme().border;
        let header_text_color = cx.theme().sidebar_foreground.opacity(0.5);
        let item_text_color = cx.theme().sidebar_foreground.opacity(0.9);
        let icon_color = cx.theme().sidebar_foreground.opacity(0.6);
        let accent_bg = cx.theme().sidebar_accent;
        let radius = cx.theme().radius;

        let this = cx.entity().clone();
        let app_state = self.app_state.clone();

        let documents = match &self.document_state {
            LoadingState::Loaded(docs) => docs.clone(),
            _ => vec![],
        };

        // Header
        let header = h_flex()
            .flex_shrink_0()
            .px_2()
            .rounded(radius)
            .text_xs()
            .text_color(header_text_color)
            .h_8()
            .justify_between()
            .items_center()
            .child("Documents")
            .child(
                h_flex().gap_1().child(
                    Button::new("create-document")
                        .icon(Icon::default().path("icons/plus.svg"))
                        .ghost()
                        .xsmall()
                        .cursor_pointer()
                        .tooltip("New document")
                        .on_click({
                            let this = this.clone();
                            let app_state = app_state.clone();
                            move |_, _, cx| {
                                let repository = cx.global::<RepositoryState>().documents.clone();
                                let this_clone = this.clone();
                                let app_state = app_state.clone();

                                cx.spawn(async move |cx| {
                                    let new_document = DocumentModel {
                                        id: 0,
                                        title: "Untitled".to_string(),
                                        content: serde_json::json!([]),
                                    };

                                    let new_id = repository.insert_document(new_document).await?;
                                    let documents = repository.get_documents().await?;

                                    let _ = cx.update(|cx: &mut App| {
                                        let _ = this_clone.update(cx, |state, _| {
                                            state.document_state = LoadingState::Loaded(documents);
                                        });

                                        cx.update_global::<DocumentState, _>(|state, _| {
                                            state.open_document(new_id, "Untitled".to_string());
                                        });

                                        app_state.update(cx, |app_state, cx| {
                                            let document_screen =
                                                DocumentScreen::new(cx.weak_entity());
                                            app_state.navigator.push(document_screen, cx);
                                        });
                                    });

                                    Ok::<_, anyhow::Error>(())
                                })
                                .detach();
                            }
                        }),
                ),
            );

        // Document items
        let items = documents.into_iter().map({
            let this = this.clone();
            let app_state = app_state.clone();
            move |document| {
                let document_id = document.id;
                let document_title = document.title.clone();
                let delete_title = document.title.clone();
                let this_clone = this.clone();
                let app_state_clone = app_state.clone();

                h_flex()
                    .id(("document-item", document_id as usize))
                    .w_full()
                    .h_7()
                    .px_2()
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(accent_bg))
                    .on_click({
                        let document_title = document_title.clone();
                        let app_state = app_state_clone.clone();
                        move |_, _, cx| {
                            cx.update_global::<DocumentState, _>(|state, _| {
                                state.open_document(document_id, document_title.clone());
                            });

                            app_state.update(cx, |app_state, cx| {
                                let document_screen = DocumentScreen::new(cx.weak_entity());
                                app_state.navigator.push(document_screen, cx);
                            });
                        }
                    })
                    .child(Icon::default().path("icons/file-text.svg").size_4().text_color(icon_color))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .overflow_hidden()
                            .text_color(item_text_color)
                            .child(document.title.clone()),
                    )
                    .child(
                        div()
                            .opacity(0.0)
                            .hover(|el| el.opacity(1.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                Button::new(("delete-doc", document_id as usize))
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .danger()
                                    .xsmall()
                                    .cursor_pointer()
                                    .on_click({
                                        let this_clone = this_clone.clone();
                                        move |_, window, cx| {
                                            let this_clone = this_clone.clone();
                                            let delete_title = delete_title.clone();

                                            ConfirmDialog::new("Delete Page")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"? This action cannot be undone.",
                                                    delete_title
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let repository = cx.global::<RepositoryState>().documents.clone();
                                                    let this_for_spawn = this_clone.clone();
                                                    let deleted_title = delete_title.clone();

                                                    cx.update_global::<DocumentState, _>(|state, _| {
                                                        state.remove_document(document_id);
                                                        if state.current_opened_document == Some(document_id) {
                                                            state.current_opened_document = None;
                                                        }
                                                    });

                                                    window.push_notification(
                                                        format!("\"{}\" has been deleted", deleted_title),
                                                        cx,
                                                    );

                                                    cx.spawn(async move |cx| {
                                                        let _ = repository.delete_document(document_id).await;

                                                        let documents = repository.get_documents().await;
                                                        if let Ok(documents) = documents {
                                                            let _ = this_for_spawn.update(cx, |state, _| {
                                                                state.document_state = LoadingState::Loaded(documents);
                                                            });
                                                        }

                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();

                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            ),
                    )
            }
        });

        v_flex()
            .h_full()
            .w(px(240.0))
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border_color)
            .child(header)
            .child(div().flex().flex_col().w_full().px_1().children(items))
    }
}
