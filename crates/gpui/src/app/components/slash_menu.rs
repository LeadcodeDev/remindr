use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Selectable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    label::Label,
    popover::Popover,
};
use uuid::Uuid;

use crate::app::{
    components::nodes::{
        element::{NodePayload, RemindrElement},
        heading::data::HeadingMetadata,
        text::data::TextMetadata,
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
        icon: Icon,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Button {
        let custom = ButtonCustomVariant::new(cx)
            .hover(cx.theme().primary.opacity(0.1))
            .active(cx.theme().secondary);

        Button::new(label)
            .custom(custom)
            .justify_start()
            .items_center()
            .py_3()
            .px_1()
            .cursor_pointer()
            .gap_2()
            .child(icon)
            .child(SharedString::new(label))
            .on_click(cx.listener(on_click))
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
            state.insert_node_after(
                this.related_id,
                &RemindrElement::create_node(
                    NodePayload::Text((TextMetadata::default(), true)),
                    &this.state,
                    window,
                    cx,
                ),
            );
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
            state.insert_node_after(
                this.related_id,
                &RemindrElement::create_node(
                    NodePayload::Heading((HeadingMetadata::default(), true)),
                    &this.state,
                    window,
                    cx,
                ),
            );
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
            let node = RemindrElement::create_node(NodePayload::Divider, &this.state, window, cx);

            state.insert_node_after(this.related_id, &node);
            this.related_id = node.id;
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
                .p_2()
                .w(px(365.0))
                .bg(cx.theme().secondary)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .gap_1()
                        .child(
                            Label::new("Components")
                                .text_xs()
                                .font_semibold()
                                .opacity(0.5),
                        )
                        .children([
                            self.render_item(
                                "Paragraph",
                                Icon::default().path("icons/pilcrow.svg"),
                                Self::on_insert_paragraph,
                                cx,
                            ),
                            self.render_item(
                                "Heading",
                                Icon::default().path("icons/heading.svg"),
                                Self::on_insert_heading,
                                cx,
                            ),
                            self.render_item(
                                "Divider",
                                Icon::default().path("icons/separator-horizontal.svg"),
                                Self::on_insert_divider,
                                cx,
                            ),
                        ]),
                ),
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
