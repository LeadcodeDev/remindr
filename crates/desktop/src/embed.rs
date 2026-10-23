use gpui_kit::{AnyView, App, Global, Window};
use remindr_core::domain::{BlockId, DatabaseId};

type EmbedFactory = Box<dyn Fn(BlockId, DatabaseId, &mut Window, &mut App) -> AnyView>;

#[derive(Default)]
pub struct EmbedRegistry {
    factory: Option<EmbedFactory>,
}

impl Global for EmbedRegistry {}

impl EmbedRegistry {
    pub fn register(
        &mut self,
        factory: impl Fn(BlockId, DatabaseId, &mut Window, &mut App) -> AnyView + 'static,
    ) {
        self.factory = Some(Box::new(factory));
    }

    #[must_use]
    pub fn build(
        &self,
        block: BlockId,
        database: DatabaseId,
        window: &mut Window,
        cx: &mut App,
    ) -> Option<AnyView> {
        self.factory
            .as_ref()
            .map(|factory| factory(block, database, window, cx))
    }
}
