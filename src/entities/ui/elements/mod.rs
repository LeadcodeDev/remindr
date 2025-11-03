use anyhow::Error;
use gpui::{AnyElement, Context, Entity, IntoElement, Render, Window};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    Utils,
    controllers::drag_controller::DragElement,
    entities::ui::elements::{
        divider::divider_element::DividerElement, text::text_element::TextElement,
        title::title_element::TitleElement,
    },
};

pub mod divider;
pub mod text;
pub mod title;

#[derive(Clone, Debug)]
pub enum RemindrElement {
    Text(Entity<TextElement>),
    Divider(Entity<DividerElement>),
    Title(Entity<TitleElement>),
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
