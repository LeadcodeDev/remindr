use gpui::*;
use gpui_component::{
    IconName, Selectable, Sizable, StyledExt, button::Button, h_flex, popover::Popover,
};
use serde_json::to_value;
use uuid::Uuid;

use crate::{
    Utils,
    entities::nodes::{
        RemindrElement,
        node::RemindrNode,
        text::{
            data::{Metadata, TextNodeData},
            text_node::TextNode,
        },
    },
    states::node_state::NodeState,
};

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
        cx: &mut Context<Self>,
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

    fn on_insert_paragraph(
        this: &mut Self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        this.state.update(cx, |state, cx| {
            let id = Utils::generate_uuid();
            let data = to_value(TextNodeData {
                id,
                metadata: Metadata::default(),
            })
            .unwrap();

            let element = cx.new(|cx| TextNode::parse(&data, &this.state, window, cx).unwrap());
            element.update(cx, |this, cx| {
                this.focus(window, cx);
            });

            let node = RemindrNode {
                id,
                element: RemindrElement::Text(element),
            };

            state.insert_node_after(this.related_id, &node);
        });

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
                        cx.listener(Self::on_insert_paragraph),
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
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
    }
}
