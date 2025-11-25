use anyhow::{Error, Ok};
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::divider::Divider;
use serde_json::Value;

#[derive(Debug)]
pub struct DividerNode;

impl DividerNode {
    pub fn parse(_: &Value, _: &mut Window, _: &mut Context<Self>) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl Render for DividerNode {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().py_5().child(Divider::horizontal())
    }
}
