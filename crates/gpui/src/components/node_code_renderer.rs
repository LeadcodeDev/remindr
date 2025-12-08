use gpui::*;
use gpui_component::input::{Input, InputState, TabSize};
use serde_json::to_string_pretty;

use crate::entities::nodes::{RemindrElement, node::RemindrNode};

#[derive(IntoElement)]
pub struct NodeCodeRenderer {
    editor_state: Entity<InputState>,
}

impl NodeCodeRenderer {
    pub fn new(nodes: Vec<RemindrNode>, window: &mut Window, cx: &mut App) -> Self {
        let mut editor_buffer = String::new();

        for node in nodes {
            let node = match node.element.clone() {
                RemindrElement::Text(node) => to_string_pretty(&node.read(cx).data).unwrap(),
                RemindrElement::Heading(node) => to_string_pretty(&node.read(cx).data).unwrap(),
                RemindrElement::Divider(node) => to_string_pretty(&node.read(cx).data).unwrap(),
            };
            editor_buffer.push_str(format!("{}\n", node.as_str()).as_str());
        }

        let state = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor("json")
                .line_number(false)
                .searchable(true)
                .tab_size(TabSize {
                    tab_size: 2,
                    hard_tabs: false,
                })
                .default_value(editor_buffer)
        });

        Self {
            editor_state: state,
        }
    }
}

impl RenderOnce for NodeCodeRenderer {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("scrollable-container")
            .size_full()
            .overflow_scroll()
            .child(Input::new(&self.editor_state).h_full())
    }
}
