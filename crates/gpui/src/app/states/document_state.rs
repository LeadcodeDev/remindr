use gpui::{App, AppContext, Entity, Global, Window};
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::app::components::node_renderer::NodeRenderer;

#[derive(Clone)]
pub struct Document {
    pub uid: String,
    pub title: String,
    pub nodes: Vec<Value>,
    pub renderer: Option<Entity<NodeRenderer>>,
}

#[derive(Clone, PartialEq)]
pub enum PersistenceState {
    Pending,
    Idle,
}

pub struct DocumentState {
    pub documents: Vec<Document>,
    pub current_document: Option<Document>,

    pub persistence: PersistenceState,
    pub last_change: Option<Instant>,
    pub pending_notification: bool,
}

impl DocumentState {
    pub fn get_current_document_index(&self) -> Option<usize> {
        self.documents.iter().position(|doc| {
            doc.uid
                == self
                    .current_document
                    .as_ref()
                    .map(|doc| doc.uid.clone())
                    .unwrap_or_default()
        })
    }

    pub fn get_previous_document(&self, uid: String) -> Option<Document> {
        let current_index = self.documents.iter().position(|doc| doc.uid == uid);
        if let Some(index) = current_index {
            if index > 0 {
                Some(self.documents[index - 1].clone())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn add_document(
        &mut self,
        uid: String,
        title: String,
        nodes: Vec<Value>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let already_has_document = self.documents.iter().any(|element| element.uid == uid);
        if !already_has_document {
            let renderer = NodeRenderer::new(nodes.clone(), window, cx);
            self.documents.push(Document {
                uid,
                title,
                nodes,
                renderer: Some(cx.new(|_| renderer)),
            });
        }
    }

    pub fn add_document_and_focus(
        &mut self,
        uid: String,
        title: String,
        nodes: Vec<Value>,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.add_document(uid.clone(), title.clone(), nodes, window, cx);
        self.current_document = self
            .documents
            .clone()
            .into_iter()
            .find(|element| element.uid == uid)
    }

    pub fn add_persisted_document(&mut self, uid: String, title: String, nodes: Vec<Value>) {
        let already_has_document = self.documents.iter().any(|element| element.uid == uid);
        if !already_has_document {
            self.documents.push(Document {
                uid,
                title,
                nodes,
                renderer: None,
            });
        }
    }

    pub fn ensure_renderer_for(&mut self, uid: &str, window: &mut Window, cx: &mut App) {
        if let Some(doc) = self.documents.iter_mut().find(|d| d.uid == uid) {
            if doc.renderer.is_none() {
                let renderer = NodeRenderer::new(doc.nodes.clone(), window, cx);
                doc.renderer = Some(cx.new(|_| renderer));
            }
        }
    }

    pub fn remove_document(&mut self, uid: String) {
        self.documents.retain(|element| element.uid != uid);
    }

    pub fn mark_changed(&mut self, _: &mut Window, cx: &mut App) {
        self.persistence = PersistenceState::Pending;
        let trigger_time = Instant::now();
        self.last_change = Some(trigger_time);

        cx.spawn(async move |cx| {
            sleep(Duration::from_secs(1)).await;

            let _ = cx.update_global::<DocumentState, _>(move |state, _| {
                if let Some(last) = state.last_change {
                    if last <= trigger_time {
                        state.persistence = PersistenceState::Idle;
                        state.pending_notification = true;
                    }
                } else {
                    state.persistence = PersistenceState::Idle;
                    state.pending_notification = true;
                }
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            documents: Vec::new(),
            current_document: None,
            persistence: PersistenceState::Idle,
            last_change: None,
            pending_notification: false,
        }
    }
}

impl Global for DocumentState {}
