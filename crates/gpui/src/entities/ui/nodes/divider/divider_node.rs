use anyhow::{Error, Ok};
use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::divider::Divider;
use serde_json::Value;

use crate::states::node_state::NodeState;

pub struct DividerNode {
    pub state: Option<Entity<NodeState>>,
}

impl DividerNode {
    pub fn parse(_: &Value, _: &mut Window, _: &mut Context<Self>) -> Result<Self, Error> {
        Ok(Self { state: None })
    }
}

impl Render for DividerNode {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().py_5().child(Divider::horizontal())
    }
}
