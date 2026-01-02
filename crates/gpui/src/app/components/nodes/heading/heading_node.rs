use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use serde_json::{Value, from_value, to_value};
use uuid::Uuid;

use crate::{
    Utils,
    app::{
        components::{
            nodes::{
                element::RemindrElement,
                heading::data::HeadingNodeData,
                menu_provider::{NodeMenuItem, NodeMenuProvider},
                node::RemindrNode,
                text::{
                    data::{TextMetadata, TextNodeData},
                    text_node::TextNode,
                },
                textual_node::{SlashMenuNode, TextualNode, TextualNodeDelegate, TextualNodeEvent},
            },
            slash_menu::{SlashMenu, SlashMenuDismissEvent},
        },
        states::{document_state::DocumentState, node_state::NodeState},
    },
};

pub struct HeadingNode {
    pub state: Entity<NodeState>,
    pub data: HeadingNodeData,
    pub input_state: Entity<InputState>,
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
                InputEvent::Focus => this.handle_focus(window, cx),
                InputEvent::Blur => this.handle_blur(window, cx),
                InputEvent::Change => this.handle_input_change(window, cx),
                InputEvent::PressEnter { .. } => {
                    this.on_textual_event(TextualNodeEvent::Enter, window, cx);
                }
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
            menu,
            is_focus: false,
        })
    }

    /// Handles input changes and emits appropriate events.
    fn handle_input_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_value = self.input_state.read(cx).value();
        let old_content = self.data.metadata.content.clone();

        if input_value.ends_with('/') && self.is_focus {
            self.on_textual_event(TextualNodeEvent::SlashTyped, window, cx);
        }

        if old_content.is_empty() && input_value.is_empty() {
            self.on_textual_event(TextualNodeEvent::Empty, window, cx);
        } else {
            self.data.metadata.content = input_value.clone();
            self.on_textual_event(TextualNodeEvent::Change(input_value), window, cx);
        }
    }

    pub fn set_level(&mut self, level: u32, window: &mut Window, cx: &mut Context<Self>) {
        self.data.metadata.level = level;
        cx.update_global::<DocumentState, _>(|state, app| {
            state.mark_changed(window, app);
        });
        cx.notify();
    }
}

impl TextualNode for HeadingNode {
    fn input_state(&self) -> &Entity<InputState> {
        &self.input_state
    }

    fn node_state(&self) -> &Entity<NodeState> {
        &self.state
    }

    fn node_id(&self) -> Uuid {
        self.data.id
    }

    fn content(&self) -> SharedString {
        self.data.metadata.content.clone()
    }

    fn set_content(&mut self, content: SharedString) {
        self.data.metadata.content = content;
    }

    fn is_focused(&self) -> bool {
        self.is_focus
    }

    fn set_focused(&mut self, focused: bool) {
        self.is_focus = focused;
    }
}

impl SlashMenuNode for HeadingNode {
    fn slash_menu(&self) -> &Entity<SlashMenu> {
        &self.menu
    }
}

impl TextualNodeDelegate for HeadingNode {
    fn on_textual_event(
        &mut self,
        event: TextualNodeEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            TextualNodeEvent::SlashTyped => {
                let menu_open = self.menu.read(cx).open;
                if !menu_open {
                    self.menu.update(cx, |menu, cx| {
                        menu.set_open(true, window, cx);
                    });
                }
            }
            TextualNodeEvent::Empty => {
                let node_id = self.data.id;
                let state = self.state.clone();

                state.update(cx, |state, inner_cx| {
                    if !state.get_nodes().is_empty() {
                        let previous_element = state.get_previous_node(node_id);
                        state.remove_node(node_id);

                        if let Some(previous_element) = previous_element {
                            if let RemindrElement::Text(element) = previous_element.element.clone()
                            {
                                let rich_text = element.read(inner_cx).rich_text_state().clone();
                                rich_text.update(inner_cx, |state, cx| {
                                    state.focus(window, cx);
                                    state.move_to_end(cx);
                                });
                            }

                            if let RemindrElement::Heading(element) =
                                previous_element.element.clone()
                            {
                                let input = element.read(inner_cx).input_state().clone();
                                input.update(inner_cx, |input, inner_cx| {
                                    input.focus(window, inner_cx);
                                    input.set_cursor_position(
                                        gpui_component::input::Position::new(u32::MAX, u32::MAX),
                                        window,
                                        inner_cx,
                                    );
                                });
                            }
                        }

                        inner_cx.update_global::<DocumentState, _>(|state, app_cx| {
                            state.mark_changed(window, app_cx);
                        });
                    }
                });
            }
            TextualNodeEvent::Enter => {
                if self.menu.read(cx).open {
                    return;
                }

                self.input_state.update(cx, |state, inner_cx| {
                    let value = state.value();
                    state.set_value(value.trim().to_string(), window, inner_cx);
                });

                self.is_focus = false;

                let node_id = self.data.id;
                let state_for_parse = self.state.clone();
                let state = self.state.clone();

                state.update(cx, |state, inner_cx| {
                    let id = Utils::generate_uuid();
                    let data = to_value(TextNodeData::new(
                        id,
                        "text".to_string(),
                        TextMetadata::default(),
                    ))
                    .unwrap();

                    let element = inner_cx
                        .new(|cx| TextNode::parse(&data, &state_for_parse, window, cx).unwrap());

                    let rich_text = element.read(inner_cx).rich_text_state().clone();
                    rich_text.update(inner_cx, |state, cx| {
                        state.focus(window, cx);
                    });

                    let node = RemindrNode::new(id, RemindrElement::Text(element));

                    state.insert_node_after(node_id, &node);
                    inner_cx.update_global::<DocumentState, _>(|state, app| {
                        state.mark_changed(window, app);
                    });
                });
            }
            TextualNodeEvent::Change(_) => {
                cx.update_global::<DocumentState, _>(|state, app_cx| {
                    state.mark_changed(window, app_cx);
                });
            }
            _ => {}
        }
    }
}

impl NodeMenuProvider for HeadingNode {
    fn menu_items(&self, _cx: &App) -> Vec<NodeMenuItem> {
        let node_id = self.data.id;

        let levels: Vec<(u32, &'static str)> =
            vec![(2, "icons/heading-2.svg"), (3, "icons/heading-3.svg")];

        levels
            .into_iter()
            .map(|(level, icon)| {
                NodeMenuItem::new(
                    format!("heading-level-{}", level),
                    format!("Heading {}", level),
                    icon,
                    move |state, window, cx| {
                        let heading_entity = {
                            let node = state.read(cx).get_current_nodes(node_id);
                            node.and_then(|n| {
                                if let RemindrElement::Heading(heading) = &n.element {
                                    Some(heading.clone())
                                } else {
                                    None
                                }
                            })
                        };

                        if let Some(heading) = heading_entity {
                            heading.update(cx, |this, cx| {
                                this.set_level(level, window, cx);
                            });
                        }
                    },
                )
            })
            .collect()
    }
}

impl Render for HeadingNode {
    fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let input = Input::new(&self.input_state)
            .bordered(false)
            .bg(transparent_white());

        let sized_input = match self.data.metadata.level {
            1 => input.text_3xl(),
            2 => input.text_2xl(),
            3 => input.text_xl(),
            4 => input.text_lg(),
            5 => input.text_base(),
            _ => input.text_sm(),
        };

        div()
            .min_w(px(820.0))
            .w_full()
            .child(sized_input)
            .child(self.menu.clone())
    }
}
