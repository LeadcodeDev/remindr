use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState, Position};
use serde_json::{Value, from_value};

use crate::app::{
    components::{
        nodes::{
            element::{NodePayload, RemindrElement},
            text::data::{TextMetadata, TextNodeData},
        },
        slash_menu::SlashMenu,
    },
    states::{document_state::DocumentState, node_state::NodeState},
};

pub struct TextNode {
    pub state: Entity<NodeState>,
    pub data: TextNodeData,
    pub input_state: Entity<InputState>,
    show_contextual_menu: bool,
    menu: Entity<SlashMenu>,
    is_focus: bool,
}

impl TextNode {
    pub fn parse(
        data: &Value,
        state: &Entity<NodeState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, Error> {
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
        let menu = cx.new(|cx| SlashMenu::new(data.id, state, window, cx));

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

        self.menu.update(cx, |state, _| {
            state.open = show_menu && self.is_focus;
            let search_query = input_state_str
                .rfind('/')
                .map(|idx| SharedString::from(input_state_str[idx + 1..].to_string()))
                .unwrap_or_default();

            state.search = if show_menu { Some(search_query) } else { None }
        });

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

                    cx.update_global::<DocumentState, _>(|state, app_cx| {
                        state.mark_changed(window, app_cx);
                    });
                }
            });
        } else {
            self.data.metadata.content = input_state_value;
            cx.update_global::<DocumentState, _>(|state, app_cx| {
                state.mark_changed(window, app_cx);
            });
        }
    }

    fn on_press_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value();
            state.set_value(value.trim().to_string(), window, cx);
        });

        self.is_focus = false;
        self.show_contextual_menu = false;
        self.menu.update(cx, |state, _| state.search = None);

        self.state.update(cx, |state, cx| {
            state.insert_node_after(
                self.data.id,
                &RemindrElement::create_node(
                    NodePayload::Text((TextMetadata::default(), true)),
                    &self.state,
                    window,
                    cx,
                ),
            );
        });

        cx.update_global::<DocumentState, _>(|state, app_cx| {
            state.mark_changed(window, app_cx);
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
            .child(self.menu.clone())
    }
}
