use gpui::*;
use gpui_component::{
    IconName,
    sidebar::{Sidebar, SidebarFooter, SidebarGroup, SidebarHeader, SidebarMenu, SidebarMenuItem},
};

use crate::{
    LoadingState,
    app::{
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

    fn render_documents(
        &self,
        documents: Vec<DocumentModel>,
        cx: &mut Context<Self>,
    ) -> SidebarGroup<SidebarMenu> {
        SidebarGroup::new("Documents").child(SidebarMenu::new().children(
            documents.into_iter().map(|document| {
                let document_id = document.id;
                let document_title = document.title.clone();

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
        let documents = match self.document_state.clone() {
            LoadingState::Loaded(documents) => self.render_documents(documents, cx),
            _ => SidebarGroup::new("Documents"),
        };

        Sidebar::left()
            .w(Pixels::from(240.0))
            .header(SidebarHeader::new())
            .child(documents)
            .footer(SidebarFooter::new().child("Footer"))
    }
}
