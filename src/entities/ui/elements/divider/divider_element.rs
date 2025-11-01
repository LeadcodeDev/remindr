use anyhow::{Error, Ok};
use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::divider::Divider;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::entities::ui::elements::ElementNodeParser;

#[derive(Debug)]
pub struct DividerElement;

impl ElementNodeParser for DividerElement {
    fn parse(_: &Value, _: &mut Window, _: &mut Context<Self>) -> Result<Self, Error> {
        Ok(Self)
    }
}

impl Render for DividerElement {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().py_5().child(Divider::horizontal())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DividerElementData {
    pub id: Uuid,
}
