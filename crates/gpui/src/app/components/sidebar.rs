use gpui::*;
use gpui_component::{
    ActiveTheme, Collapsible, Icon, IconName, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    h_flex,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
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
            cx.spawn(async move |this, cx| {
                let documents = document_repository.get_documents().await;
                if let Ok(documents) = documents {
                    let _ = this.update(cx, |state: &mut Self, _| {
                        state.document_state = LoadingState::Loaded(documents);
                    });
                }
            })
            .detach();

            Self {
                document_state: LoadingState::Loading,
                app_state,
            }
        })
    }

    fn render_documents_header(&self, cx: &mut Context<Self>) -> Div {
        h_flex()
            .flex_shrink_0()
            .px_2()
            .rounded(cx.theme().radius)
            .text_xs()
            .text_color(cx.theme().sidebar_foreground.opacity(0.7))
            .h_8()
            .justify_between()
            .items_center()
            .child("Documents")
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("create-document")
                            .icon(Icon::default().path("icons/plus.svg"))
                            .ghost()
                            .xsmall()
                            .tooltip("New document")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let repository = cx.global::<RepositoryState>().documents.clone();
                                let this_clone = cx.entity().clone();
                                let app_state = this.app_state.clone();

                                cx.spawn(async move |_, cx| {
                                    // Create a new document with default content
                                    let new_document = DocumentModel {
                                        id: 0,
                                        title: "Untitled".to_string(),
                                        content: serde_json::json!([]),
                                    };

                                    // Insert into database
                                    let new_id = repository.insert_document(new_document).await?;

                                    // Refresh documents list
                                    let documents = repository.get_documents().await?;

                                    let _ = cx.update(|cx| {
                                        // Update sidebar
                                        let _ = this_clone.update(cx, |state, _| {
                                            state.document_state = LoadingState::Loaded(documents);
                                        });

                                        // Open the new document
                                        cx.update_global::<DocumentState, _>(|state, _| {
                                            state.open_document(new_id, "Untitled".to_string());
                                        });

                                        // Navigate to document screen
                                        app_state.update(cx, |app_state, cx| {
                                            let document_screen =
                                                DocumentScreen::new(cx.weak_entity());
                                            app_state.navigator.push(document_screen, cx);
                                        });
                                    });

                                    Ok::<_, anyhow::Error>(())
                                })
                                .detach();
                            })),
                    )
                    .child(
                        Button::new("reload-documents")
                            .icon(Icon::default().path("icons/refresh-cw.svg"))
                            .ghost()
                            .xsmall()
                            .tooltip("Reload documents")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let repository = cx.global::<RepositoryState>().documents.clone();
                                let this_clone = cx.entity().clone();

                                this.document_state = LoadingState::Loading;

                                cx.spawn(async move |_, cx| {
                                    let documents = repository.get_documents().await;
                                    if let Ok(documents) = documents {
                                        let _ = this_clone.update(cx, |state, _| {
                                            state.document_state = LoadingState::Loaded(documents);
                                        });
                                    }

                                    Ok::<_, anyhow::Error>(())
                                })
                                .detach();
                            })),
                    ),
            )
    }

    fn render_documents(
        &self,
        documents: Vec<DocumentModel>,
        cx: &mut Context<Self>,
    ) -> SidebarGroup<SidebarMenu> {
        let this = cx.entity().clone();

        SidebarGroup::new("").collapsed(true).child(SidebarMenu::new().children(
            documents.into_iter().map(|document| {
                let document_id = document.id;
                let document_title = document.title.clone();
                let delete_title = document.title.clone();
                let this_clone = this.clone();

                SidebarMenuItem::new(document.title.clone())
                    .icon(IconName::File)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Only add document metadata to state (lazy loading)
                        cx.update_global::<DocumentState, _>(|state, _| {
                            state.open_document(document_id, document_title.clone());
                        });

                        this.app_state.update(cx, |app_state, cx| {
                            let document_screen = DocumentScreen::new(cx.weak_entity());
                            app_state.navigator.push(document_screen, cx);
                        });
                    }))
                    .suffix(
                        div()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                div()
                                    .opacity(0.0)
                                    .hover(|this| this.opacity(1.0))
                                    .child(
                                        Button::new(("delete-doc", document_id as usize))
                                            .icon(Icon::default().path("icons/trash-2.svg"))
                                            .danger()
                                            .xsmall()
                                            .on_click(move |_, window, cx| {
                                                let this_clone = this_clone.clone();
                                                let delete_title = delete_title.clone();

                                                ConfirmDialog::new("Delete Document")
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

                                                        // Remove from DocumentState if open
                                                        cx.update_global::<DocumentState, _>(|state, _| {
                                                            state.remove_document(document_id);
                                                            if state.current_opened_document == Some(document_id) {
                                                                state.current_opened_document = None;
                                                            }
                                                        });

                                                        // Show notification
                                                        window.push_notification(
                                                            format!("\"{}\" has been deleted", deleted_title),
                                                            cx,
                                                        );

                                                        // Delete from database and refresh sidebar
                                                        cx.spawn(async move |cx| {
                                                            let _ = repository.delete_document(document_id).await;

                                                            // Refresh the sidebar document list
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
                                            }),
                                    ),
                            ),
                    )
                    .collapsed(false)
                    .active(cx.read_global::<DocumentState, _>({
                        move |state, _| {
                            state
                                .current_opened_document
                                .map(|id| id == document_id)
                                .unwrap_or(false)
                        }
                    }))
            }),
        ))
    }
}

impl Render for AppSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let documents_header = self.render_documents_header(cx);
        let documents = match self.document_state.clone() {
            LoadingState::Loaded(documents) => self.render_documents(documents, cx),
            _ => self.render_documents(vec![], cx),
        };

        Sidebar::left()
            .w(Pixels::from(240.0))
            .header(SidebarHeader::new().child(documents_header))
            .child(documents)
            .footer(SidebarFooter::new().child("Footer"))
    }
}
