use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::ui::nodes::RemindrElement;

#[derive(Clone)]
pub struct RemindrNode {
    pub id: Uuid,
    pub element: RemindrElement,
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
    Title,
}
