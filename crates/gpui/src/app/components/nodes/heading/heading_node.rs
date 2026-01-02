use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState, Position};
use serde_json::{Value, from_value, to_value};

use crate::{
    Utils,
    app::{
        components::{
            nodes::{
                element::RemindrElement,
                heading::data::HeadingNodeData,
                node::RemindrNode,
                text::{
                    data::{TextMetadata, TextNodeData},
                    text_node::TextNode,
                },
            },
            slash_menu::{SlashMenu, SlashMenuDismissEvent},
        },
        states::node_state::NodeState,
    },
};

pub struct HeadingNode {
    pub state: Entity<NodeState>,
    pub data: HeadingNodeData,
    pub input_state: Entity<InputState>,
    show_contextual_menu: bool,
    menu: Entity<SlashMenu>,
    is_focus: bool,
}

impl HeadingNode {
    pub fn parse(
        data: &Value,
        state: &Entity<NodeState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, Error> {
        let data = from_value::<HeadingNodeData>(data.clone())?;

        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("New document")
                .default_value(data.metadata.content.clone())
                .auto_grow(1, INFINITY as usize)
                .soft_wrap(true)
        });

        cx.subscribe_in(&input_state, window, {
            move |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Focus => this.is_focus = true,
                InputEvent::Change => this.on_change(window, cx),
                InputEvent::PressEnter { .. } => this.on_press_enter(window, cx),
                _ => {}
            }
        })
        .detach();

        let menu = cx.new(|cx| SlashMenu::new(data.id, state, window, cx));

        cx.subscribe_in(&menu, window, {
            move |this, _, event: &SlashMenuDismissEvent, window, cx| {
                if event.restore_focus {
                    let input_state = this.input_state.clone();
                    cx.defer_in(window, move |_, window, cx| {
                        input_state.update(cx, |element, cx| {
                            element.focus(window, cx);
                        });
                    });
                }
            }
        })
        .detach();

        Ok(Self {
            state: state.clone(),
            data,
            input_state,
            show_contextual_menu: false,
            menu,
            is_focus: false,
        })
    }

    fn on_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state_value = self.input_state.read(cx).value();
        let input_state_owned = input_state_value.clone();
        let input_state_str = input_state_owned.as_str();

        // Check if we should open the slash menu (when "/" is typed)
        let should_open = input_state_str.ends_with('/') && self.is_focus;
        let menu_open = self.menu.read(cx).open;

        if should_open && !menu_open {
            self.show_contextual_menu = true;
            self.menu.update(cx, |menu, cx| {
                menu.set_open(true, window, cx);
            });
        }

        if self.data.metadata.content.is_empty() && input_state_value.is_empty() {
            self.state.update(cx, |state, cx| {
                if !state.get_nodes().is_empty() {
                    let previous_element = state.get_previous_node(self.data.id);
                    state.remove_node(self.data.id);

                    if let Some(previous_element) = previous_element {
                        if let RemindrElement::Text(element) = previous_element.element.clone() {
                            element.update(cx, |this, cx| {
                                this.focus(window, cx);
                                this.move_cursor_end(window, cx);
                            });
                        }

                        if let RemindrElement::Heading(element) = previous_element.element.clone() {
                            element.update(cx, |this, cx| {
                                this.focus(window, cx);
                                this.move_cursor_end(window, cx);
                            });
                        }
                    }
                }
            });

            cx.update_global::<crate::app::states::document_state::DocumentState, _>(
                |state, app| {
                    state.mark_changed(window, app);
                },
            );
        } else {
            self.data.metadata.content = input_state_value;
            cx.update_global::<crate::app::states::document_state::DocumentState, _>(
                |state, app| {
                    state.mark_changed(window, app);
                },
            );
        }
    }

    fn on_press_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // If slash menu is open, don't create new line (menu handles Enter)
        if self.show_contextual_menu {
            return;
        }

        self.input_state.update(cx, |state, cx| {
            let value = state.value();
            state.set_value(value.trim().to_string(), window, cx);
        });

        self.is_focus = false;
        self.show_contextual_menu = false;

        self.state.update(cx, |state, cx| {
            let id = Utils::generate_uuid();
            let data = to_value(TextNodeData::new(
                id,
                "text".to_string(),
                TextMetadata::default(),
            ))
            .unwrap();

            let element = cx.new(|cx| TextNode::parse(&data, &self.state, window, cx).unwrap());
            element.update(cx, |this, cx| {
                this.focus(window, cx);
            });

            let node = RemindrNode::new(id, RemindrElement::Text(element));

            state.insert_node_after(self.data.id, &node);
            cx.update_global::<crate::app::states::document_state::DocumentState, _>(
                |state, app| {
                    state.mark_changed(window, app);
                },
            );
        });
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |element, cx| {
            element.focus(window, cx);
        });
    }

    pub fn move_cursor_end(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |element, cx| {
            element.set_cursor_position(
                Position::new(INFINITY as u32, INFINITY as u32),
                window,
                cx,
            );
        });
    }
}

impl Render for HeadingNode {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .child(
                Input::new(&self.input_state)
                    .bordered(false)
                    .text_3xl()
                    .bg(transparent_white()),
            )
            .child(self.menu.clone())
    }
}
