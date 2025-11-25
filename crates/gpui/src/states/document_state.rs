use gpui::{App, AppContext, Entity, Global, Window};
use serde_json::Value;

use crate::components::node_renderer::NodeRenderer;

#[derive(Clone)]
pub struct Document {
    pub uid: String,
    pub renderer: Entity<NodeRenderer>,
}

pub struct DocumentState {
    pub documents: Vec<Document>,
}

impl DocumentState {
    pub fn add_document(
        &mut self,
        uid: impl Into<String>,
        nodes: Vec<Value>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let uid = uid.into();
        let already_has_document = self.documents.iter().any(|element| element.uid == uid);

        if !already_has_document {
            let renderer = NodeRenderer::new(nodes, window, cx);
            self.documents.push(Document {
                uid,
                renderer: cx.new(|_| renderer),
            });
        }
    }

    pub fn remove_document(&mut self, uid: String) {
        self.documents.retain(|element| element.uid != uid);
    }
}

impl Global for DocumentState {}
