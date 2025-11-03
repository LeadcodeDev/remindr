use std::fs::read_to_string;

use gpui::{AppContext, Context, Window};
use serde_json::{Value, from_str};
use uuid::Uuid;

use crate::entities::ui::elements::{
    ElementNodeParser, RemindrElement, divider::divider_element::DividerElement,
    text::text_element::TextElement, title::title_element::TitleElement,
};

pub struct DocumentParser;

impl DocumentParser {
    pub fn load_file(&self, value: impl Into<String>) -> Vec<Value> {
        let file_content = read_to_string(value.into()).expect("Failed to read demo.json");
        from_str(&file_content).expect("Failed to parse JSON from demo.json")
    }

    pub fn parse_nodes(
        &self,
        entries: &Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<(Uuid, RemindrElement)> {
        let mut elements = Vec::new();
        for entry in entries {
            let (id, element) = self.parse_node(entry, window, cx);

            elements.push((id, element));
        }

        elements
    }

    pub fn parse_node(
        &self,
        entry: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Uuid, RemindrElement) {
        let id = Uuid::parse_str(entry.get("id").unwrap().as_str().unwrap()).unwrap();
        let element_type = entry.get("type").unwrap().as_str().unwrap();

        let element = match element_type {
            "text" => {
                let element = cx.new(|cx| TextElement::parse(entry, window, cx).unwrap());
                RemindrElement::Text(element)
            }
            "title" => {
                let element = cx.new(|cx| TitleElement::parse(entry, window, cx).unwrap());
                RemindrElement::Title(element)
            }
            "divider" => {
                let element = cx.new(|cx| DividerElement::parse(entry, window, cx).unwrap());
                RemindrElement::Divider(element)
            }
            _ => panic!("Unknown element type"),
        };

        (id, element)
    }
}
