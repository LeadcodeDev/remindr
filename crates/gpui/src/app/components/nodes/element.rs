use crate::{
    Utils,
    app::{
        components::nodes::{
            database_view::{
                data::{DatabaseViewMetadata, DatabaseViewNodeData},
                database_view_node::DatabaseViewNode,
            },
            divider::{data::DividerNodeData, divider_node::DividerNode},
            heading::{
                data::{HeadingMetadata, HeadingNodeData},
                heading_node::HeadingNode,
            },
            menu_provider::{NodeMenuItem, NodeMenuProvider},
            node::RemindrNode,
            text::{
                data::{TextMetadata, TextNodeData},
                text_node::TextNode,
            },
            textual_node::TextualNode,
        },
        states::node_state::NodeState,
    },
};
use gpui::{AnyElement, App, AppContext, Context, Entity, IntoElement, Render, RenderOnce, Window};
use serde_json::{Value, to_value};

pub enum NodePayload {
    Text((TextMetadata, bool)),
    Heading((HeadingMetadata, bool)),
    Divider,
    DatabaseView(DatabaseViewMetadata),
}

#[derive(Clone, Debug, IntoElement)]
pub enum RemindrElement {
    Text(Entity<TextNode>),
    Divider(Entity<DividerNode>),
    Heading(Entity<HeadingNode>),
    DatabaseView(Entity<DatabaseViewNode>),
}

impl RemindrElement {
    pub fn get_data(&self, cx: &mut App) -> Value {
        match self {
            RemindrElement::Text(text) => to_value(text.read(cx).data.clone()).unwrap(),
            RemindrElement::Divider(divider) => to_value(divider.read(cx).data.clone()).unwrap(),
            RemindrElement::Heading(heading) => to_value(heading.read(cx).data.clone()).unwrap(),
            RemindrElement::DatabaseView(node) => to_value(node.read(cx).data.clone()).unwrap(),
        }
    }

    pub fn menu_items(&self, cx: &App) -> Vec<NodeMenuItem> {
        match self {
            RemindrElement::Text(text) => text.read(cx).menu_items(cx),
            RemindrElement::Divider(divider) => divider.read(cx).menu_items(cx),
            RemindrElement::Heading(heading) => heading.read(cx).menu_items(cx),
            RemindrElement::DatabaseView(node) => node.read(cx).menu_items(cx),
        }
    }

    pub fn create_node(
        payload: NodePayload,
        state: &Entity<NodeState>,
        window: &mut Window,
        cx: &mut App,
    ) -> RemindrNode {
        Self::create_node_with_id(Utils::generate_uuid(), payload, state, window, cx)
    }

    pub fn create_node_with_id(
        id: uuid::Uuid,
        payload: NodePayload,
        state: &Entity<NodeState>,
        window: &mut Window,
        cx: &mut App,
    ) -> RemindrNode {
        let node = match payload {
            NodePayload::Heading((payload, is_focus)) => {
                let data =
                    to_value(HeadingNodeData::new(id, "heading".to_string(), payload)).unwrap();

                let element = cx.new(|cx| HeadingNode::parse(&data, &state, window, cx).unwrap());
                if is_focus {
                    element.update(cx, |this, cx| {
                        this.focus(window, cx);
                    });
                }

                RemindrElement::Heading(element)
            }
            NodePayload::Text((payload, is_focus)) => {
                let data = to_value(TextNodeData::new(id, "text".to_string(), payload)).unwrap();

                let element = cx.new(|cx| TextNode::parse(&data, &state, window, cx).unwrap());
                if is_focus {
                    element.update(cx, |this, cx| {
                        this.focus(window, cx);
                    });
                }

                RemindrElement::Text(element)
            }
            NodePayload::Divider => {
                let data = to_value(DividerNodeData::new(id, "divider".to_string())).unwrap();
                let element = cx.new(|cx| DividerNode::parse(&data, window, cx).unwrap());

                RemindrElement::Divider(element)
            }
            NodePayload::DatabaseView(metadata) => {
                let data = to_value(DatabaseViewNodeData::new(
                    id,
                    "database_view".to_string(),
                    metadata,
                ))
                .unwrap();
                let element = cx.new(|cx| DatabaseViewNode::parse(&data, window, cx).unwrap());

                RemindrElement::DatabaseView(element)
            }
        };

        RemindrNode::new(id, node)
    }
}

impl RenderOnce for RemindrElement {
    #[allow(refining_impl_trait)]
    fn render(self, _: &mut Window, _: &mut App) -> AnyElement {
        match self {
            RemindrElement::Text(element) => element.clone().into_any_element(),
            RemindrElement::Divider(element) => element.clone().into_any_element(),
            RemindrElement::Heading(element) => element.clone().into_any_element(),
            RemindrElement::DatabaseView(element) => element.clone().into_any_element(),
        }
    }
}

impl Render for RemindrElement {
    #[allow(refining_impl_trait)]
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> AnyElement {
        match self {
            RemindrElement::Text(element) => element.clone().into_any_element(),
            RemindrElement::Divider(element) => element.clone().into_any_element(),
            RemindrElement::Heading(element) => element.clone().into_any_element(),
            RemindrElement::DatabaseView(element) => element.clone().into_any_element(),
        }
    }
}
