use gpui::{App, AppContext, DragMoveEvent, Entity, Window};
use serde_json::{Value, from_value};
use uuid::Uuid;

use crate::app::components::nodes::{
    divider::divider_node::DividerNode,
    element::RemindrElement,
    heading::heading_node::HeadingNode,
    node::{PartialRemindrNode, RemindrNode, RemindrNodeType},
    text::text_node::TextNode,
};

#[derive(Clone, PartialEq)]
pub enum MovingElement {
    Before,
    After,
}

#[derive(Clone)]
pub struct NodeState {
    elements: Vec<RemindrNode>,
    pub hovered_drop_zone: Option<(Uuid, MovingElement)>,
    pub dragging_id: Option<Uuid>,
    pub is_dragging: bool,
}

impl NodeState {
    pub fn get_nodes(&self) -> &Vec<RemindrNode> {
        &self.elements
    }

    pub fn get_current_nodes(&self, id: Uuid) -> Option<&RemindrNode> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn start_drag(&mut self, id: Uuid) {
        self.dragging_id = Some(id);
        self.is_dragging = true;
    }

    pub fn stop_drag(&mut self) {
        self.dragging_id = None;
        self.is_dragging = false;
        self.hovered_drop_zone = None;
    }

    pub fn update_hover_zone(
        &mut self,
        id: Uuid,
        mouse_y: f32,
        bounds_top: f32,
        bounds_height: f32,
    ) -> bool {
        let middle_y = bounds_top + bounds_height / 2.0;
        let zone = if mouse_y < middle_y {
            MovingElement::After
        } else {
            MovingElement::Before
        };

        if mouse_y >= bounds_top && mouse_y <= bounds_top + bounds_height {
            if self.hovered_drop_zone != Some((id, zone.clone())) {
                self.hovered_drop_zone = Some((id, zone.clone()));
                return true;
            }
        } else if let Some((i, _)) = self.hovered_drop_zone {
            if i == id {
                self.hovered_drop_zone = None;
                return true;
            }
        }

        false
    }

    pub fn drop_element_by_index(
        &mut self,
        from_index: usize,
        target_index: usize,
        position: MovingElement,
    ) {
        let element = self.elements.remove(from_index);

        let mut to_index = target_index;

        match position {
            MovingElement::After => {
                if from_index < target_index {
                    to_index = target_index.saturating_sub(1);
                }
            }
            MovingElement::Before => {
                if from_index >= target_index {
                    to_index = target_index + 1;
                }
            }
        }

        let final_index = to_index.clamp(0, self.elements.len());
        self.elements.insert(final_index, element);

        self.stop_drag();
    }

    pub fn on_outside<T>(&mut self, event: &DragMoveEvent<T>) -> bool {
        let mouse_position = event.event.position;
        let bounds = event.bounds;

        let is_outside = mouse_position.x < bounds.origin.x
            || mouse_position.y < bounds.origin.y
            || mouse_position.x > bounds.origin.x + bounds.size.width
            || mouse_position.y > bounds.origin.y + bounds.size.height;

        if is_outside.clone() {
            self.stop_drag();
        }

        is_outside
    }

    pub fn parse_node(
        &self,
        value: &Value,
        state: &Entity<NodeState>,
        window: &mut Window,
        app: &mut App,
    ) -> RemindrNode {
        let partial_node = from_value::<PartialRemindrNode>(value.clone()).unwrap();
        let element = match partial_node.node_type {
            RemindrNodeType::Text => {
                let element = app.new(|cx| TextNode::parse(value, state, window, cx).unwrap());
                RemindrElement::Text(element)
            }
            RemindrNodeType::Heading => {
                let element = app.new(|cx| HeadingNode::parse(value, state, window, cx).unwrap());
                RemindrElement::Heading(element)
            }
            RemindrNodeType::Divider => {
                let element = app.new(|cx| DividerNode::parse(value, window, cx).unwrap());
                RemindrElement::Divider(element)
            }
        };

        RemindrNode {
            id: partial_node.id,
            element,
        }
    }

    pub fn push_node(&mut self, node: &RemindrNode) {
        self.elements.push(node.clone());
    }

    pub fn remove_node(&mut self, id: Uuid) {
        self.elements.retain(|node| node.id != id);
    }

    pub fn insert_node_after(&mut self, id: Uuid, node: &RemindrNode) {
        let index = self.elements.iter().position(|node| node.id == id).unwrap();
        self.elements.insert(index + 1, node.clone());
    }

    pub fn insert_node_at(&mut self, index: usize, node: &RemindrNode) {
        self.elements.insert(index, node.clone());
    }

    pub fn get_previous_node(&self, id: Uuid) -> Option<RemindrNode> {
        let index = self.elements.iter().position(|node| node.id == id)?;
        if index == 0 {
            return None;
        }
        self.elements.get(index - 1).cloned()
    }
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            elements: Vec::new(),
            dragging_id: None,
            hovered_drop_zone: None,
            is_dragging: false,
        }
    }
}
