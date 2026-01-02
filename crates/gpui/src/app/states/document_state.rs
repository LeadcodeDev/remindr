use gpui::{App, AppContext, BorrowAppContext, Context, Entity, Global, Window};
use gpui_component::input::{InputEvent, InputState};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::{
    LoadingState,
    app::{
        components::{
            node_renderer::NodeRenderer,
            nodes::{
                element::{NodePayload, RemindrElement},
                text::data::TextMetadata,
            },
        },
        states::repository_state::RepositoryState,
    },
    domain::database::document::DocumentModel,
};

/// Helper entity to handle title input events with proper subscription context
pub struct TitleInputHandler {
    pub input_state: Entity<InputState>,
    pub renderer: Entity<NodeRenderer>,
    pub document_id: i32,
}

impl TitleInputHandler {
    pub fn new(
        document_id: i32,
        input_state: Entity<InputState>,
        renderer: Entity<NodeRenderer>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input_state_for_closure = input_state.clone();
        cx.subscribe_in(&input_state, window, {
            let renderer = renderer.clone();
            move |_this, _, event: &InputEvent, window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    // Insert a new text node at the beginning and focus it
                    renderer.update(cx, |renderer, cx| {
                        let state = renderer.state.clone();
                        let node = RemindrElement::create_node(
                            NodePayload::Text((TextMetadata::default(), true)),
                            &state,
                            window,
                            cx,
                        );
                        state.update(cx, |node_state, _| {
                            node_state.insert_node_at(0, &node);
                        });
                    });
                }
                InputEvent::Change => {
                    let new_title = input_state_for_closure.read(cx).value().to_string();
                    cx.update_global::<DocumentState, _>(|doc_state, cx| {
                        if let Some(doc) = doc_state
                            .documents
                            .iter_mut()
                            .find(|d| d.uid == document_id)
                        {
                            doc.title = new_title;
                        }
                        doc_state.mark_changed(window, cx);
                    });
                }
                _ => {}
            }
        })
        .detach();

        Self {
            input_state,
            renderer,
            document_id,
        }
    }
}

#[derive(Clone)]
pub struct OpenedDocument {
    pub uid: i32,
    pub title: String,
    pub state: LoadingState<DocumentContent>,
}

#[derive(Clone)]
pub struct DocumentContent {
    pub nodes: Vec<Value>,
    pub renderer: Entity<NodeRenderer>,
    pub title_input: Entity<InputState>,
    _title_handler: Entity<TitleInputHandler>,
}

#[derive(Clone, PartialEq)]
pub enum PersistenceState {
    Pending,
    Idle,
}

pub struct DocumentState {
    pub documents: Vec<OpenedDocument>,
    pub current_opened_document: Option<i32>,

    pub persistence: PersistenceState,
    pub last_change: Option<Instant>,
}

impl DocumentState {
    pub fn get_current_document(&self) -> Option<&OpenedDocument> {
        self.current_opened_document
            .and_then(|id| self.documents.iter().find(|doc| doc.uid == id))
    }

    pub fn get_current_document_index(&self) -> Option<usize> {
        self.current_opened_document
            .and_then(|id| self.documents.iter().position(|doc| doc.uid == id))
    }

    pub fn get_previous_document(&self, uid: i32) -> Option<&OpenedDocument> {
        let current_index = self.documents.iter().position(|doc| doc.uid == uid);
        current_index.and_then(|index| {
            if index > 0 {
                Some(&self.documents[index - 1])
            } else if self.documents.len() > 1 {
                Some(&self.documents[1])
            } else {
                None
            }
        })
    }

    /// Add a document tab with just metadata (loading state)
    pub fn open_document(&mut self, id: i32, title: String) {
        let already_exists = self.documents.iter().any(|doc| doc.uid == id);
        if !already_exists {
            self.documents.push(OpenedDocument {
                uid: id,
                title,
                state: LoadingState::Loading,
            });
        }
        self.current_opened_document = Some(id);
    }

    /// Set the loaded content for a document
    pub fn set_document_content(
        &mut self,
        uid: i32,
        document: DocumentModel,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.uid == uid) {
            let nodes = document
                .content
                .as_array()
                .map(|arr| arr.clone())
                .unwrap_or_default();

            let renderer = NodeRenderer::new(nodes.clone(), window, cx);
            let renderer = cx.new(|_| renderer);

            // Create title input state
            let title = document.title.clone();
            let title_input = cx.new(|cx| {
                let mut state = InputState::new(window, cx).placeholder("Untitled");

                state.set_value(title, window, cx);
                state
            });

            // Create the title handler to manage Enter key events
            let title_handler = cx.new(|cx| {
                TitleInputHandler::new(uid, title_input.clone(), renderer.clone(), window, cx)
            });

            doc.state = LoadingState::Loaded(DocumentContent {
                nodes,
                renderer,
                title_input,
                _title_handler: title_handler,
            });
        }
    }

    /// Set error state for a document
    pub fn set_document_error(&mut self, uid: i32, error: String) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.uid == uid) {
            doc.state = LoadingState::Error(error);
        }
    }

    /// Check if a document needs loading
    pub fn needs_loading(&self, uid: i32) -> bool {
        self.documents
            .iter()
            .find(|d| d.uid == uid)
            .map(|d| matches!(d.state, LoadingState::Loading))
            .unwrap_or(false)
    }

    pub fn remove_document(&mut self, uid: i32) {
        self.documents.retain(|element| element.uid != uid);
    }

    pub fn mark_changed(&mut self, _: &mut Window, cx: &mut App) {
        let trigger_time = Instant::now();

        self.last_change = Some(trigger_time);

        let documents = cx.global::<RepositoryState>().documents.clone();

        let document = self
            .documents
            .iter()
            .find(|doc| Some(doc.uid) == self.current_opened_document)
            .cloned();

        if let Some(document) = document {
            if let LoadingState::Loaded(content) = &document.state {
                let renderer = content.renderer.clone();
                let doc_uid = document.uid;
                let doc_title = document.title.clone();

                cx.spawn(async move |cx| {
                    sleep(Duration::from_secs(1)).await;

                    let _ = cx.update_global::<DocumentState, _>(move |state, cx| {
                        if let Some(last) = state.last_change {
                            if last <= trigger_time {
                                // Debounce expired, start saving
                                state.persistence = PersistenceState::Pending;

                                let nodes = {
                                    let nodes = renderer.read(cx).state.clone();
                                    let nodes = nodes.read(cx).get_nodes().clone();
                                    nodes
                                        .iter()
                                        .map(|node| node.element.get_data(cx))
                                        .collect::<Vec<_>>()
                                };

                                let document_model = DocumentModel {
                                    id: doc_uid,
                                    title: doc_title,
                                    content: Value::from_iter(nodes),
                                };

                                cx.spawn(async move |cx| {
                                    let result = documents.update_document(document_model).await;

                                    // Minimum display time for the loader
                                    sleep(Duration::from_secs(1)).await;

                                    // Mark as idle when save completes
                                    let _ = cx.update_global::<DocumentState, _>(|state, _| {
                                        state.persistence = PersistenceState::Idle;
                                    });

                                    result
                                })
                                .detach();
                            }
                        }
                    });

                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
        }
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            current_opened_document: None,
            persistence: PersistenceState::Idle,
            last_change: None,
        }
    }
}

impl Global for DocumentState {}
