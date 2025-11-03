use anyhow::Error;
use gpui::{AnyElement, Context, Entity, IntoElement, Render, Window};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    Utils,
    controllers::drag_controller::DragElement,
    entities::ui::nodes::{
        divider::divider_node::DividerNode, heading::heading_node::HeadingNode,
        text::text_node::TextNode,
    },
};

pub mod divider;
pub mod heading;
pub mod text;

#[derive(Clone, Debug)]
pub enum RemindrElement {
    Text(Entity<TextNode>),
    Divider(Entity<DividerNode>),
    Title(Entity<HeadingNode>),
}

impl Render for RemindrElement {
    #[allow(refining_impl_trait)]
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> AnyElement {
        match self {
            RemindrElement::Text(element) => element.clone().into_any_element(),
            RemindrElement::Divider(element) => element.clone().into_any_element(),
            RemindrElement::Title(element) => element.clone().into_any_element(),
        }
    }
}

pub struct ElementNode {
    pub id: Uuid,
    pub element: Entity<DragElement>,
}

impl ElementNode {
    pub fn new(element: Entity<DragElement>) -> Self {
        Self {
            id: Utils::generate_uuid(),
            element,
        }
    }

    pub fn with_id(id: Uuid, element: Entity<DragElement>) -> Self {
        Self { id, element }
    }
}

pub trait ElementNodeParser {
    fn parse(payload: &Value, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error>
    where
        Self: Sized;
}
