use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::components::nodes::element::RemindrElement;

#[derive(Clone)]
pub struct RemindrNode {
    pub id: Uuid,
    pub element: RemindrElement,
}

impl RemindrNode {
    pub fn new(id: Uuid, element: RemindrElement) -> Self {
        Self { id, element }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PartialRemindrNode {
    pub id: Uuid,

    #[serde(rename = "type")]
    pub node_type: RemindrNodeType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RemindrNodeType {
    Text,
    Divider,
    Heading,
}
