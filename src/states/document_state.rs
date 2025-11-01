use gpui::Global;

use crate::{controllers::drag_controller::DragController, entities::ui::elements::ElementNode};

pub struct ViewState {
    pub current: Option<DocumentState>,
}

impl Global for ViewState {}

impl Default for ViewState {
    fn default() -> Self {
        Self { current: None }
    }
}

pub struct DocumentState {
    pub elements: Vec<ElementNode>,
    pub drag_controller: DragController,
}
