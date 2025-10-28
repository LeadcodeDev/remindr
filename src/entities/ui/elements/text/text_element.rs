use std::f64::INFINITY;

use anyhow::{Error, Ok};
use gpui::{
    AppContext, BorrowAppContext, Context, Entity, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div, prelude::FluentBuilder, px, transparent_white,
};
use gpui_component::{
    StyledExt,
    input::{InputEvent, InputState, TextInput},
};
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
                InputEvent::Change => this.on_change(window, cx),
                InputEvent::PressEnter { .. } => this.on_press_enter(window, cx),
                _ => {}
            }
        })];

        (input_state, _subscriptions)
    }

    fn on_change(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input_state = self.input_state.read(cx);

        self._show_contextual_menu = if input_state.value().ends_with("/") {
            true
        } else {
            false
        };

        if self.data.metadata.content.is_empty() && input_state.value().is_empty() {
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
                                    element.update(cx, |this, cx| this.focus(window, cx));
                                }
                                _ => {}
                            }
                        }
                    }
                }
            });
        } else {
            self.data.metadata.content = input_state.value();
        }
    }

    fn on_press_enter(&self, window: &mut Window, cx: &mut Context<Self>) {
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
        let drag_element = cx.new(|_| DragElement::new(id, element));
        let element_node = ElementNode::with_id(id, drag_element);

        cx.update_global::<ViewState, _>(|this, _| {
            this.current
                .as_mut()
                .unwrap()
                .elements
                .insert(insertion_index, element_node);
        });

        text_element.update(cx, |this, cx| this.focus(window, cx));
    }

    pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |element, cx| {
            element.focus(window, cx);
        });
    }
}

impl Render for TextElement {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .child(
                TextInput::new(&self.input_state)
                    .bordered(false)
                    .bg(transparent_white()),
            )
            .when(self._show_contextual_menu, |this| {
                this.child(self.menu.clone())
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextElementData {
    pub id: Uuid,
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    content: SharedString,
}

impl Default for Metadata {
    fn default() -> Self {
        Self {
            content: SharedString::new(""),
        }
    }
}
