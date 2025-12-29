use crate::infrastructure::repositories::document_repository::DocumentRepository;
use gpui::Global;

pub struct RepositoryState {
    pub documents: DocumentRepository,
}

impl Global for RepositoryState {}
