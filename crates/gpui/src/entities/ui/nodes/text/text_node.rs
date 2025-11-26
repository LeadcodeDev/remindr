use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::input::{Input, InputEvent, InputState, Position};
use serde_json::{Value, from_value, to_value};

use crate::{
    Utils,
    components::slash_menu::SlashMenu,
    entities::ui::nodes::{
        RemindrElement,
        node::RemindrNode,
        text::data::{Metadata, TextNodeData},
    },
    states::node_state::NodeState,
};

pub struct TextNode {
    pub state: Option<Entity<NodeState>>,
    pub data: TextNodeData,
    input_state: Entity<InputState>,
    show_contextual_menu: bool,
    menu: Entity<SlashMenu>,
    is_focus: bool,
}

impl TextNode {
    pub fn parse(data: &Value, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error> {
        let data = from_value::<TextNodeData>(data.clone())?;

        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
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
        let menu = cx.new(|cx| SlashMenu::new(window, cx));

        Ok(Self {
            state: None,
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

        let show_menu = if let Some(last_slash_idx) = input_state_str.rfind('/') {
            let next_char_idx = last_slash_idx + 1;
            if next_char_idx == input_state_str.len() {
                true
            } else {
                input_state_str
                    .chars()
                    .nth(next_char_idx)
                    .map_or(false, |c| c != ' ')
            }
        } else {
            false
        };

        self.show_contextual_menu = show_menu && self.is_focus;

        if show_menu {
            let search_query = input_state_str
                .rfind('/')
                .map(|idx| SharedString::from(input_state_str[idx + 1..].to_string()))
                .unwrap_or_default();
            self.menu
                .update(cx, |state, _| state.search = Some(search_query));
        } else {
            self.menu.update(cx, |state, _| state.search = None);
        }

        if self.data.metadata.content.is_empty() && input_state_value.is_empty() {
            if let Some(state) = self.state.clone() {
                state.update(cx, |state, cx| {
                    if !state.get_nodes().is_empty() {
                        let previous_element = state.get_previous_node(self.data.id);
                        state.remove_node(self.data.id);

                        if let Some(previous_element) = previous_element {
                            if let RemindrElement::Text(element) = previous_element.element.clone()
                            {
                                element.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                    this.move_cursor_end(window, cx);
                                });
                            }

                            if let RemindrElement::Title(element) = previous_element.element.clone()
                            {
                                element.update(cx, |this, cx| {
                                    this.focus(window, cx);
                                    this.move_cursor_end(window, cx);
                                });
                            }
                        }
                    }
                });
            }
        } else {
            self.data.metadata.content = input_state_value;
        }
    }

    fn on_press_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value();
            state.set_value(value.trim().to_string(), window, cx);
        });

        if let Some(state) = self.state.clone() {
            self.is_focus = false;
            self.show_contextual_menu = false;
            self.menu.update(cx, |state, _| state.search = None);

            state.update(cx, |state, cx| {
                let id = Utils::generate_uuid();
                let data = to_value(TextNodeData {
                    id,
                    metadata: Metadata::default(),
                })
                .unwrap();

                let element = cx.new(|cx| TextNode::parse(&data, window, cx).unwrap());
                element.update(cx, |this, cx| {
                    this.focus(window, cx);
                });

                let node = RemindrNode {
                    id,
                    element: RemindrElement::Text(element),
                };

                state.insert_node_after(self.data.id, &node);
            });
        }
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

impl Render for TextNode {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .child(
                Input::new(&self.input_state)
                    .bordered(false)
                    .bg(transparent_white()),
            )
            .when(self.show_contextual_menu, |this| {
                this.child(self.menu.clone())
            })
    }
}
