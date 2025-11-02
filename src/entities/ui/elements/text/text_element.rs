use std::f32::INFINITY;

use anyhow::{Error, Ok};
use gpui::{prelude::FluentBuilder, *};
use gpui_component::input::{Input, InputEvent, InputState, Position};
use serde::{Deserialize, Serialize};
use serde_json::{Value, from_value};
use uuid::Uuid;

use crate::{
    Utils,
    controllers::drag_controller::DragElement,
    entities::ui::{
        elements::{ElementNode, ElementNodeParser, RemindrElement},
        menu::Menu,
    },
    states::document_state::ViewState,
};

#[derive(Debug)]
pub struct TextElement {
    pub data: TextElementData,
    input_state: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
    _show_contextual_menu: bool,
    menu: Entity<Menu>,
    is_focus: bool,
}

impl ElementNodeParser for TextElement {
    fn parse(data: &Value, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error> {
        let data = from_value::<TextElementData>(data.clone())?;

        let (input_state, _subscriptions) = Self::init(data.metadata.content.clone(), window, cx);
        let menu = cx.new(|cx| Menu::new(window, cx));

        Ok(Self {
            data,
            input_state,
            _subscriptions,
            _show_contextual_menu: false,
            menu,
            is_focus: false,
        })
    }
}

impl TextElement {
    pub fn new(id: Uuid, window: &mut Window, cx: &mut Context<Self>) -> Result<Self, Error> {
        let content = SharedString::new("");
        let (input_state, _subscriptions) = Self::init(content.clone(), window, cx);
        let menu = cx.new(|cx| Menu::new(window, cx));

        Ok(Self {
            data: TextElementData {
                id,
                metadata: Metadata { content },
            },
            input_state,
            _subscriptions,
            _show_contextual_menu: false,
            menu,
            is_focus: false,
        })
    }

    fn init(
        content: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Vec<Subscription>) {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(content)
                .auto_grow(1, INFINITY as usize)
                .soft_wrap(true)
        });

        let _subscriptions = vec![cx.subscribe_in(&input_state, window, {
            move |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Focus => this.is_focus = true,
                InputEvent::Change => this.on_change(window, cx),
                InputEvent::PressEnter { .. } => this.on_press_enter(window, cx),
                _ => {}
            }
        })];

        (input_state, _subscriptions)
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

        self._show_contextual_menu = show_menu && self.is_focus;

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
                    let elements_rc_clone = &mut current_doc_state.elements;
                    let index = {
                        elements_rc_clone
                            .iter()
                            .position(|e| e.id == self.data.id)
                            .unwrap_or_default()
                    };

                    if elements_rc_clone.len() > 1 {
                        elements_rc_clone.remove(index);

                        let previous_element = elements_rc_clone.get(index.saturating_sub(1));
                        if let Some(node) = previous_element {
                            match node.element.read(cx).child.clone() {
                                RemindrElement::Text(element) => {
                                    element.update(cx, |this, cx| {
                                        this.focus(window, cx);
                                        this.set_cursor_position(
                                            Position::new(INFINITY as u32, INFINITY as u32),
                                            window,
                                            cx,
                                        );
                                    });
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        } else {
            self.data.metadata.content = input_state_value;
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

        let text_element = cx.new(|cx| TextElement::new(id, window, cx).unwrap());
        let element = RemindrElement::Text(text_element.clone());
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
        self._show_contextual_menu = false;
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

    pub fn set_cursor_position(
        &self,
        position: Position,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input_state.update(cx, |element, cx| {
            element.set_cursor_position(position, window, cx);
        });
    }
}

impl Render for TextElement {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .child(
                Input::new(&self.input_state)
                    .bordered(false)
                    .bg(transparent_white()),
            )
            .when(self._show_contextual_menu, |this| {
                this.child(self.menu.clone())
            })
        // .when(true, |this| this.child(self.menu.clone()))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextElementData {
    pub id: Uuid,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub content: SharedString,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            content: SharedString::new(""),
        }
    }
}
