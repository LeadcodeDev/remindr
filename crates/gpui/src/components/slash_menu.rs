use gpui::*;
use gpui_component::{IconName, Selectable, Sizable, button::Button, h_flex, popover::Popover};
use serde_json::to_value;
use uuid::Uuid;

use crate::{
    Utils,
    entities::nodes::{
        RemindrElement,
        divider::{data::DividerNodeData, divider_node::DividerNode},
        heading::{
            data::{HeadingNodeData, Metadata as HeadingMetadata},
            heading_node::HeadingNode,
        },
        node::RemindrNode,
        text::{
            data::{Metadata as TextMetadata, TextNodeData},
            text_node::TextNode,
        },
    },
    states::node_state::NodeState,
};

#[derive(Clone)]
pub struct SlashMenu {
    related_id: Uuid,
    pub state: Entity<NodeState>,
    pub open: bool,
    pub search: Option<SharedString>,
}

impl SlashMenu {
    pub fn new(
        related_id: Uuid,
        state: &Entity<NodeState>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Self {
        Self {
            related_id,
            state: state.clone(),
            open: false,
            search: None,
        }
    }

    fn render_item(
        &self,
        label: &'static str,
        icon: IconName,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Button {
        Button::new(label)
            .small()
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(SharedString::new(label)),
            )
            .child(icon)
            .on_click(on_click)
    }

    fn remove_slash_command(&self, element: SharedString) -> SharedString {
        let text = element.as_str().to_string();

        let stripped_string = if let Some((before, _)) = text.rsplit_once('/') {
            before.to_string()
        } else {
            text
        };

        SharedString::from(stripped_string)
    }

    fn remove_slash(this: &mut Self, window: &mut Window, cx: &mut Context<Self>) {
        let current_node = this.state.read(cx).get_current_nodes(this.related_id);
        if let Some(node) = current_node {
            match node.element.clone() {
                RemindrElement::Text(element) => element.update(cx, |element, cx| {
                    element.input_state.update(cx, |element, cx| {
                        let value = this.remove_slash_command(element.value());
                        element.set_value(value, window, cx);
                    })
                }),
                RemindrElement::Heading(element) => element.update(cx, |element, cx| {
                    element.input_state.update(cx, |element, cx| {
                        let value = this.remove_slash_command(element.value());
                        element.set_value(value, window, cx);
                    })
                }),
                _ => {}
            }
        }
    }

    fn on_insert_paragraph(
        this: &mut Self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::remove_slash(this, window, cx);
        this.state.update(cx, |state, cx| {
            let id = Utils::generate_uuid();
            let data = to_value(TextNodeData::new(id, TextMetadata::default())).unwrap();

            let element = cx.new(|cx| TextNode::parse(&data, &this.state, window, cx).unwrap());
            element.update(cx, |this, cx| {
                this.focus(window, cx);
            });

            let node = RemindrNode::new(id, RemindrElement::Text(element));

            state.insert_node_after(this.related_id, &node);
        });

        this.open = false;
        cx.notify();
    }

    fn on_insert_heading(
        this: &mut Self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::remove_slash(this, window, cx);
        this.state.update(cx, |state, cx| {
            let id = Utils::generate_uuid();
            let data = to_value(HeadingNodeData::new(id, HeadingMetadata::default())).unwrap();

            let element = cx.new(|cx| HeadingNode::parse(&data, &this.state, window, cx).unwrap());
            element.update(cx, |this, cx| {
                this.focus(window, cx);
            });

            let node = RemindrNode::new(id, RemindrElement::Heading(element));

            state.insert_node_after(this.related_id, &node);
        });

        this.open = false;
        cx.notify();
    }

    fn on_insert_divider(
        this: &mut Self,
        event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        Self::remove_slash(this, window, cx);

        let current_slash_menu_id = this.related_id;

        this.state.update(cx, |state, cx| {
            let id = Utils::generate_uuid();
            let data = to_value(DividerNodeData { id }).unwrap();

            let element = cx.new(|cx| DividerNode::parse(&data, window, cx).unwrap());
            let node = RemindrNode::new(id, RemindrElement::Divider(element));

            state.insert_node_after(this.related_id, &node);
            this.related_id = id;
        });

        Self::on_insert_paragraph(this, event, window, cx);
        this.related_id = current_slash_menu_id;

        this.open = false;
        cx.notify();
    }
}

impl Render for SlashMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().child(
            Popover::new("controlled-popover")
                .anchor(Corner::TopLeft)
                .trigger(Empty::default())
                .open(self.open)
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    this.open = *open;
                    cx.notify();
                }))
                .child(div().flex().flex_col().flex_1().gap_1().children([
                    self.render_item(
                        "Paragraph",
                        IconName::ChevronDown,
                        cx.listener(Self::on_insert_paragraph),
                    ),
                    self.render_item(
                        "Heading",
                        IconName::ChevronDown,
                        cx.listener(Self::on_insert_heading),
                    ),
                    self.render_item(
                        "Divider",
                        IconName::ChevronDown,
                        cx.listener(Self::on_insert_divider),
                    ),
                ])),
        )
    }
}

#[derive(IntoElement)]
struct Empty {
    selected: bool,
}

impl Default for Empty {
    fn default() -> Self {
        Self { selected: false }
    }
}

impl Selectable for Empty {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for Empty {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
    }
}
