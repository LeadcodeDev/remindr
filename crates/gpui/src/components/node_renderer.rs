use gpui::*;
use gpui_component::ActiveTheme;
use serde_json::Value;

use crate::{
    components::draggable::Draggable, entities::ui::nodes::node::RemindrNode,
    states::node_state::NodeState,
};

pub struct NodeRenderer {
    state: Entity<NodeState>,
}

impl NodeRenderer {
    pub fn new(nodes: Vec<Value>, window: &mut Window, app: &mut App) -> Self {
        let mut state = NodeState::default();

        for value in nodes.into_iter() {
            let node = state.parse_node(&value, window, app);
            state.push_node(&node);
        }

        let state = app.new(|_| state);
        Self { state }
    }

    fn on_drag_move(
        this: &mut Self,
        event: &DragMoveEvent<RemindrNode>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        this.state.update(cx, |state, cx| {
            if state.on_outside(event) {
                cx.notify();
            }
        })
    }
}

impl Render for NodeRenderer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nodes = self.state.read(cx).get_nodes().clone();

        let mut children: Vec<Entity<Draggable>> = Vec::with_capacity(nodes.len());
        for node in nodes.into_iter() {
            let ent = cx.new(|cx| Draggable::new(self.state.clone(), node.clone(), cx));
            children.push(ent);
        }

        div()
            .w_full()
            .h_full()
            .flex()
            .flex_1()
            .justify_center()
            .bg(cx.theme().background.opacity(0.8))
            .child(
                div()
                    .max_w(px(820.0))
                    .w_full()
                    .on_drag_move(cx.listener(Self::on_drag_move))
                    .children(children),
            )
    }
}
