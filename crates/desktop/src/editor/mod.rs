use gpui_kit::{Context, Entity, IntoElement, Render, TestSupportExt, Window, div, prelude::*};
use remindr_core::domain::BlockId;

use crate::app::WorkspaceModel;

pub struct BlockEditor {
    model: Entity<WorkspaceModel>,
    page: BlockId,
}

impl BlockEditor {
    pub fn new(
        model: Entity<WorkspaceModel>,
        page: BlockId,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self { model, page }
    }
}

impl Render for BlockEditor {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id("block-editor").test_support()
    }
}
