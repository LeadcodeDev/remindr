use gpui_kit::{AnyView, App, Entity, Global, Window};
use remindr_core::domain::{BlockId, DatabaseId};

use crate::app::WorkspaceModel;

type EmbedFactory =
    Box<dyn Fn(Entity<WorkspaceModel>, BlockId, DatabaseId, &mut Window, &mut App) -> AnyView>;

#[derive(Default)]
pub struct EmbedRegistry {
    factory: Option<EmbedFactory>,
}

impl Global for EmbedRegistry {}

impl EmbedRegistry {
    pub fn register(
        &mut self,
        factory: impl Fn(Entity<WorkspaceModel>, BlockId, DatabaseId, &mut Window, &mut App) -> AnyView
        + 'static,
    ) {
        self.factory = Some(Box::new(factory));
    }

    #[must_use]
    pub fn build(
        &self,
        model: Entity<WorkspaceModel>,
        block: BlockId,
        database: DatabaseId,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyView> {
        self.factory
            .as_ref()
            .map(|factory| factory(model, block, database, window, cx))
    }
}
