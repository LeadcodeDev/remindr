use gpui::Global;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Settings {}

impl Global for Settings {}
