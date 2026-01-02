use anyhow::{Error, Ok};
use gpui::*;
use serde_json::{Value, from_value};

use crate::app::{
    components::{
        nodes::{
            element::{NodePayload, RemindrElement},
            menu_provider::{NodeMenuItem, NodeMenuProvider},
            text::data::{TextMetadata, TextNodeData},
        },
        rich_text::{RichTextEvent, RichTextState, RichTextView},
        slash_menu::{SlashMenu, SlashMenuDismissEvent},
    },
    states::{document_state::DocumentState, node_state::NodeState},
};

pub struct TextNode {
    pub state: Entity<NodeState>,
    pub data: TextNodeData,
    pub rich_text_state: Entity<RichTextState>,
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

        let rich_text_state = cx.new(|cx| {
            let mut state = RichTextState::new(window, cx);
            if !data.metadata.content.is_empty() {
                state.set_content(data.metadata.content.to_string(), cx);
            }
            state
        });

        cx.subscribe_in(&rich_text_state, window, {
            move |this, _, ev: &RichTextEvent, window, cx| match ev {
                RichTextEvent::Focus => this.handle_focus(window, cx),
                RichTextEvent::Blur => this.handle_blur(window, cx),
                RichTextEvent::Change(content) => {
                    this.handle_content_change(content.clone(), window, cx)
                }
                RichTextEvent::Enter => this.handle_enter(window, cx),
                RichTextEvent::Backspace => this.handle_backspace(window, cx),
                RichTextEvent::Delete => this.handle_delete(window, cx),
                RichTextEvent::Slash => this.handle_slash(window, cx),
                RichTextEvent::Tab | RichTextEvent::Space => {}
            }
        })
        .detach();

        let menu = cx.new(|cx| SlashMenu::new(data.id, state, window, cx));

        cx.subscribe_in(&menu, window, {
            move |this, _, event: &SlashMenuDismissEvent, window, cx| {
                if event.restore_focus {
                    let rich_text_state = this.rich_text_state.clone();
                    cx.defer_in(window, move |_, window, cx| {
                        rich_text_state.update(cx, |state, cx| {
                            state.focus(window, cx);
                        });
                    });
                }
            }
        })
        .detach();

        Ok(Self {
            state: state.clone(),
            data,
            rich_text_state,
            menu,
            is_focus: false,
        })
    }

    fn handle_focus(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_focus = true;
    }

    fn handle_blur(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_focus = false;
    }

    fn handle_content_change(
        &mut self,
        content: SharedString,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let old_content = self.data.metadata.content.clone();

        if old_content.is_empty() && content.is_empty() {
            self.handle_empty(window, cx);
        } else {
            self.data.metadata.content = content;
            cx.update_global::<DocumentState, _>(|state, app_cx| {
                state.mark_changed(window, app_cx);
            });
        }
    }

    fn handle_slash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_focus {
            let menu_open = self.menu.read(cx).open;
            if !menu_open {
                self.menu.update(cx, |menu, cx| {
                    menu.set_open(true, window, cx);
                });
            }
        }
    }

    fn handle_backspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.rich_text_state.read(cx).content().to_string();
        if content.is_empty() {
            self.handle_empty(window, cx);
        }
    }

    fn handle_delete(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // Reserved for future use
    }

    fn handle_empty(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let node_id = self.data.id;
        let state = self.state.clone();

        state.update(cx, |state, inner_cx| {
            if !state.get_nodes().is_empty() {
                let previous_element = state.get_previous_node(node_id);
                state.remove_node(node_id);

                if let Some(previous_element) = previous_element {
                    if let RemindrElement::Text(element) = previous_element.element.clone() {
                        let rich_text = element.read(inner_cx).rich_text_state.clone();
                        rich_text.update(inner_cx, |state, cx| {
                            state.focus(window, cx);
                            state.move_to_end(cx);
                        });
                    }

                    if let RemindrElement::Heading(element) = previous_element.element.clone() {
                        element.update(inner_cx, |heading, inner_cx| {
                            heading.input_state.update(inner_cx, |input, inner_cx| {
                                input.focus(window, inner_cx);
                                input.set_cursor_position(
                                    gpui_component::input::Position::new(u32::MAX, u32::MAX),
                                    window,
                                    inner_cx,
                                );
                            });
                        });
                    }
                }

                inner_cx.update_global::<DocumentState, _>(|state, app_cx| {
                    state.mark_changed(window, app_cx);
                });
            }
        });
    }

    fn handle_enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.menu.read(cx).open {
            return;
        }

        // Trim the content
        let content = self.rich_text_state.read(cx).content().trim().to_string();
        self.data.metadata.content = SharedString::from(content);

        self.is_focus = false;

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

    pub fn rich_text_state(&self) -> &Entity<RichTextState> {
        &self.rich_text_state
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        self.rich_text_state.update(cx, |state, cx| {
            state.focus(window, cx);
        });
    }

    pub fn move_cursor_end(&self, cx: &mut App) {
        self.rich_text_state.update(cx, |state, cx| {
            state.move_to_end(cx);
        });
    }
}

impl NodeMenuProvider for TextNode {
    fn menu_items(&self, _cx: &App) -> Vec<NodeMenuItem> {
        vec![]
    }
}

impl Render for TextNode {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .min_w(px(820.0))
            .w_full()
            .my_2()
            .child(RichTextView::new(self.rich_text_state.clone()).ml_3())
            .child(self.menu.clone())
    }
}
