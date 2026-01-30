use crate::infrastructure::repositories::database_repository::DatabaseRepository;
use crate::infrastructure::repositories::document_repository::DocumentRepository;
use crate::infrastructure::repositories::folder_repository::FolderRepository;
use gpui::Global;

pub struct RepositoryState {
    pub documents: DocumentRepository,
    pub folders: FolderRepository,
    pub databases: DatabaseRepository,
}

impl Global for RepositoryState {}
