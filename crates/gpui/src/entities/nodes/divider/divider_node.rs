use anyhow::{Error, Ok};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::divider::Divider;
use serde_json::{Value, from_value};
use uuid::Uuid;

use crate::{entities::nodes::divider::data::DividerNodeData, states::node_state::NodeState};

pub struct DividerNode {
    pub id: Uuid,
    pub state: Option<Entity<NodeState>>,
}

impl DividerNode {
    pub fn parse(data: &Value, _: &mut Window, _: &mut Context<Self>) -> Result<Self, Error> {
        let data = from_value::<DividerNodeData>(data.clone())?;

        Ok(Self {
            id: data.id,
            state: None,
        })
    }
}

impl Render for DividerNode {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().py_5().child(Divider::horizontal())
    }
}
