use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::input::{InputEvent, InputState, Position, TextInput};
use serde_json::{Value, from_value};
use uuid::Uuid;

use crate::{
    Utils,
    controllers::drag_controller::DragElement,
    entities::ui::{
        menu::Menu,
        nodes::{
            ElementNode, ElementNodeParser, RemindrElement,
            heading::data::{HeadingNodeData, Metadata},
        },
    },
    states::document_state::ViewState,
};

#[derive(Debug)]
pub struct HeadingNode {
    pub data: HeadingNodeData,
    input_state: Entity<InputState>,
    show_contextual_menu: bool,
    menu: Entity<Menu>,
    is_focus: bool,
}

impl ElementNodeParser for HeadingNode {
    fn parse(data: &Value, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error> {
        let data = from_value::<HeadingNodeData>(data.clone())?;

        let input_state = Self::init(data.metadata.content.clone(), window, cx);
        let menu = cx.new(|cx| Menu::new(window, cx));

        Ok(Self {
            data,
            input_state,
            show_contextual_menu: false,
            menu,
            is_focus: false,
        })
    }
}

impl HeadingNode {
    pub fn new(id: Uuid, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error> {
        let content = SharedString::new("");
        let input_state = Self::init(content.clone(), window, cx);
        let menu = cx.new(|cx| Menu::new(window, cx));

        Ok(Self {
            data: HeadingNodeData {
                id,
                metadata: Metadata {
                    content,
                    ..Default::default()
                },
            },
            input_state,
            show_contextual_menu: false,
            menu,
            is_focus: false,
        })
    }

    fn init(
        content: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(content)
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

        input_state
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
            cx.update_global::<ViewState, _>(|view_state, cx| {
                if let Some(current_doc_state) = view_state.current.as_mut() {
                    let elements = &mut current_doc_state.elements;
                    if !elements.is_empty() {
                        let index = {
                            elements
                                .iter()
                                .position(|e| e.id == self.data.id)
                                .unwrap_or_default()
                        };

                        self.on_remove_element_and_navigate_previous(index, elements, window, cx);
                    }
                }
            });
        } else {
            self.data.metadata.content = input_state_value;
        }
    }

    fn on_remove_element_and_navigate_previous(
        &self,
        index: usize,
        elements: &mut Vec<ElementNode>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        elements.remove(index);

        let previous_element = elements.get(index.saturating_sub(1));
        if let Some(node) = previous_element {
            match node.element.read(cx).child.clone() {
                RemindrElement::Text(element) => {
                    element.update(cx, |this, cx| {
                        this.focus(window, cx);
                        this.move_cursor_end(window, cx);
                    });
                }
                _ => {}
            }
        }
    }

    fn on_press_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value();
            state.set_value(value.trim().to_string(), window, cx);
        });

        let id = Utils::generate_uuid();
        let state = cx.global::<ViewState>().current.as_ref().unwrap();

        let insertion_index = state
            .elements
            .iter()
            .position(|e| e.id == self.data.id)
            .map(|idx| idx + 1)
            .unwrap_or_default();

        let text_element = cx.new(|cx| HeadingNode::new(id, window, cx).unwrap());
        let element = RemindrElement::Title(text_element.clone());
        let drag_element = cx.new(|cx| DragElement::new(id, element, cx));
        let element_node = ElementNode::with_id(id, drag_element);

        cx.update_global::<ViewState, _>(|this, _| {
            this.current
                .as_mut()
                .unwrap()
                .elements
                .insert(insertion_index, element_node);
        });

        self.is_focus = false;
        self.show_contextual_menu = false;
        self.menu.update(cx, |state, _| state.search = None);

        text_element.update(cx, |this, cx| {
            this.focus(window, cx);
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
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .child(
                TextInput::new(&self.input_state)
                    .bordered(false)
                    .text_3xl()
                    .bg(transparent_white()),
            )
            .when(self.show_contextual_menu, |this| {
                this.child(self.menu.clone())
            })
    }
}
