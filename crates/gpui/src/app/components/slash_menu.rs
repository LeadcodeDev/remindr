use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Selectable, Sizable,
    input::{Backspace, Escape, Input, InputEvent, InputState, MoveDown, MoveUp, Position},
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

pub struct SlashMenuDismissEvent {
    pub restore_focus: bool,
}

#[derive(Clone)]
struct MenuItem {
    label: &'static str,
    icon_path: &'static str,
    shortcut: Option<&'static str>,
    action: MenuAction,
}

#[derive(Clone, Copy)]
enum MenuAction {
    InsertText,
    InsertHeading2,
    InsertHeading3,
    InsertDivider,
}

#[derive(Clone, Copy, PartialEq)]
pub enum SlashMenuMode {
    /// Replace the current node with the selected item
    Replace,
    /// Insert the selected item after the related node
    InsertAfter,
}

pub struct SlashMenu {
    related_id: Uuid,
    pub state: Entity<NodeState>,
    pub open: bool,
    pub selected_index: usize,
    pub focus_handle: FocusHandle,
    search_input: Entity<InputState>,
    items: Vec<MenuItem>,
    mode: SlashMenuMode,
}

impl SlashMenu {
    pub fn new(
        related_id: Uuid,
        state: &Entity<NodeState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let items = vec![
            MenuItem {
                label: "Text",
                icon_path: "icons/pilcrow.svg",
                shortcut: None,
                action: MenuAction::InsertText,
            },
            MenuItem {
                label: "Heading 2",
                icon_path: "icons/heading-2.svg",
                shortcut: Some("##"),
                action: MenuAction::InsertHeading2,
            },
            MenuItem {
                label: "Heading 3",
                icon_path: "icons/heading-3.svg",
                shortcut: Some("###"),
                action: MenuAction::InsertHeading3,
            },
            MenuItem {
                label: "Divider",
                icon_path: "icons/separator-horizontal.svg",
                shortcut: Some("---"),
                action: MenuAction::InsertDivider,
            },
        ];

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search blocks..."));

        cx.subscribe_in(
            &search_input,
            window,
            |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    this.selected_index = 0;
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            related_id,
            state: state.clone(),
            open: false,
            selected_index: 0,
            focus_handle: cx.focus_handle(),
            search_input,
            items,
            mode: SlashMenuMode::Replace,
        }
    }

    pub fn with_mode(mut self, mode: SlashMenuMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn set_related_id(&mut self, id: Uuid) {
        self.related_id = id;
    }

    pub fn related_id(&self) -> Uuid {
        self.related_id
    }

    pub fn set_mode(&mut self, mode: SlashMenuMode) {
        self.mode = mode;
    }

    pub fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.open = open;
        if open {
            self.search_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
                input.focus(window, cx);
            });
            self.selected_index = 0;
        }
        cx.notify();
    }

    fn filtered_items(&self, cx: &App) -> Vec<(usize, MenuItem)> {
        let search = self.search_input.read(cx).value();
        let search = search.to_lowercase();

        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                if search.is_empty() {
                    return true;
                }
                item.label.to_lowercase().contains(search.as_str())
            })
            .map(|(i, item)| (i, item.clone()))
            .collect()
    }

    pub fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_items(cx);
        if filtered.is_empty() {
            return;
        }

        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = filtered.len() - 1;
        }
        cx.notify();
    }

    pub fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_items(cx);
        if filtered.is_empty() {
            return;
        }

        if self.selected_index < filtered.len() - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
        cx.notify();
    }

    pub fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let filtered = self.filtered_items(cx);
        if let Some((_, item)) = filtered.get(self.selected_index) {
            match item.action {
                MenuAction::InsertText => self.insert_text(window, cx),
                MenuAction::InsertHeading2 => self.insert_heading(2, window, cx),
                MenuAction::InsertHeading3 => self.insert_heading(3, window, cx),
                MenuAction::InsertDivider => self.insert_divider(window, cx),
            }
        }
        self.selected_index = 0;
    }

    fn render_section_label(
        &self,
        label: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div().px_2().py_1().child(
            Label::new(label)
                .text_xs()
                .text_color(cx.theme().muted_foreground),
        )
    }

    fn render_item(
        &self,
        visual_index: usize,
        item: &MenuItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.selected_index == visual_index;
        let bg_color = if is_selected {
            cx.theme().accent.opacity(0.5)
        } else {
            cx.theme().transparent
        };

        let text_color = if is_selected {
            cx.theme().accent_foreground
        } else {
            cx.theme().foreground
        };

        let action = item.action;
        let shortcut = item.shortcut;

        div()
            .id(SharedString::from(format!("menu-item-{}", visual_index)))
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_2()
            .py_0p5()
            .rounded_md()
            .cursor_pointer()
            .bg(bg_color)
            .hover(|this| {
                if !is_selected {
                    this.bg(cx.theme().accent.opacity(0.5))
                } else {
                    this
                }
            })
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(cx.listener(move |this, _, window, cx| match action {
                MenuAction::InsertText => this.insert_text(window, cx),
                MenuAction::InsertHeading2 => this.insert_heading(2, window, cx),
                MenuAction::InsertHeading3 => this.insert_heading(3, window, cx),
                MenuAction::InsertDivider => this.insert_divider(window, cx),
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::default()
                            .path(item.icon_path)
                            .size_4()
                            .text_color(text_color),
                    )
                    .child(Label::new(item.label).text_sm().text_color(text_color)),
            )
            .when_some(shortcut, |this, shortcut| {
                this.child(
                    Label::new(shortcut)
                        .text_xs()
                        .text_color(cx.theme().muted_foreground),
                )
            })
    }

    fn render_search_input(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div().w_full().py_0p5().child(
            Input::new(&self.search_input)
                .text_sm()
                .appearance(false)
                .prefix(
                    Icon::default()
                        .path("icons/search.svg")
                        .small()
                        .text_color(cx.theme().muted_foreground),
                ),
        )
    }

    fn render_menu_content(
        &mut self,
        filtered_items: &[(usize, MenuItem)],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let should_close_on_backspace =
            self.mode == SlashMenuMode::Replace && self.search_input.read(cx).value().is_empty();

        let mut content = div()
            .flex()
            .flex_col()
            .when(should_close_on_backspace, |el| {
                el.capture_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key.as_str() == "backspace" {
                        this.open = false;
                        cx.emit(SlashMenuDismissEvent {
                            restore_focus: true,
                        });
                        cx.notify();
                        cx.stop_propagation();
                    }
                }))
            });

        content = content.child(self.render_search_input(cx));
        content = content.child(self.render_section_label("Basic blocks", cx));

        if filtered_items.is_empty() {
            content = content.child(
                div().px_2().py_2().child(
                    Label::new("No results")
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                ),
            );
        } else {
            for (visual_idx, (_, item)) in filtered_items.iter().enumerate() {
                content = content.child(self.render_item(visual_idx, item, cx));
            }
        }

        content = content.child(self.render_footer(cx));
        content
    }

    fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .w_full()
            .px_2()
            .py_1p5()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(
                Label::new("Type '/' on the page")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new("esc")
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
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

    fn remove_slash(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let current_node = self.state.read(cx).get_current_nodes(self.related_id);
        if let Some(node) = current_node {
            match node.element.clone() {
                RemindrElement::Text(element) => element.update(cx, |element, cx| {
                    element.rich_text_state.update(cx, |state, cx| {
                        let value = self.remove_slash_command(state.value());
                        state.set_content(value.to_string(), cx);
                    })
                }),
                RemindrElement::Heading(element) => element.update(cx, |element, cx| {
                    element.input_state.update(cx, |element, cx| {
                        let value = self.remove_slash_command(element.value());
                        element.set_value(value, window, cx);
                    })
                }),
                _ => {}
            }
        }
    }

    fn insert_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == SlashMenuMode::Replace {
            self.remove_slash(window, cx);
        }

        self.state.update(cx, |state, cx| {
            state.insert_node_after(
                self.related_id,
                &RemindrElement::create_node(
                    NodePayload::Text((TextMetadata::default(), true)),
                    &self.state,
                    window,
                    cx,
                ),
            );
        });

        self.open = false;
        cx.emit(SlashMenuDismissEvent {
            restore_focus: self.mode == SlashMenuMode::InsertAfter,
        });
        cx.notify();
    }

    fn insert_heading(&mut self, level: u32, window: &mut Window, cx: &mut Context<Self>) {
        let current_id = self.related_id;

        if self.mode == SlashMenuMode::InsertAfter {
            // Insert new heading after current node
            self.state.update(cx, |state, cx| {
                let node = RemindrElement::create_node(
                    NodePayload::Heading((
                        HeadingMetadata {
                            level,
                            content: SharedString::default(),
                        },
                        true,
                    )),
                    &self.state,
                    window,
                    cx,
                );
                state.insert_node_after(current_id, &node);
            });
        } else {
            // Replace mode: get current content and cursor position before replacing
            let current_content = self.get_current_content(cx);
            let content_without_slash = self.remove_slash_command(current_content.clone());
            let cursor_position = self.get_current_cursor_position(cx);

            // Calculate new cursor position (subtract 1 for the removed slash)
            let new_cursor_char = cursor_position.character.saturating_sub(1);
            let new_position = Position::new(cursor_position.line, new_cursor_char);

            // Replace current block with heading using the same ID
            self.state.update(cx, |state, cx| {
                let node = RemindrElement::create_node_with_id(
                    current_id,
                    NodePayload::Heading((
                        HeadingMetadata {
                            level,
                            content: content_without_slash,
                        },
                        true,
                    )),
                    &self.state,
                    window,
                    cx,
                );

                // Restore cursor position on the new heading
                if let RemindrElement::Heading(heading) = &node.element {
                    heading.update(cx, |heading, cx| {
                        heading.input_state.update(cx, |input, cx| {
                            input.set_cursor_position(new_position, window, cx);
                        });
                    });
                }

                state.replace_node(current_id, &node);
            });
        }

        self.open = false;
        cx.emit(SlashMenuDismissEvent {
            restore_focus: false,
        });
        cx.notify();
    }

    fn get_current_content(&self, cx: &App) -> SharedString {
        let current_node = self.state.read(cx).get_current_nodes(self.related_id);
        if let Some(node) = current_node {
            match &node.element {
                RemindrElement::Text(element) => element.read(cx).rich_text_state.read(cx).value(),
                RemindrElement::Heading(element) => element.read(cx).input_state.read(cx).value(),
                _ => SharedString::default(),
            }
        } else {
            SharedString::default()
        }
    }

    fn get_current_cursor_position(&self, cx: &App) -> Position {
        let current_node = self.state.read(cx).get_current_nodes(self.related_id);
        if let Some(node) = current_node {
            match &node.element {
                RemindrElement::Text(element) => {
                    // RichText uses Selection instead of Position, return cursor at selection head
                    let selection = element.read(cx).rich_text_state.read(cx).selection();
                    Position::new(0, selection.head() as u32)
                }
                RemindrElement::Heading(element) => {
                    element.read(cx).input_state.read(cx).cursor_position()
                }
                _ => Position::default(),
            }
        } else {
            Position::default()
        }
    }

    fn insert_divider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.mode == SlashMenuMode::Replace {
            self.remove_slash(window, cx);
        }

        let current_slash_menu_id = self.related_id;

        self.state.update(cx, |state, cx| {
            let node = RemindrElement::create_node(NodePayload::Divider, &self.state, window, cx);

            state.insert_node_after(self.related_id, &node);
            self.related_id = node.id;
        });

        // Insert a text node after the divider
        self.state.update(cx, |state, cx| {
            state.insert_node_after(
                self.related_id,
                &RemindrElement::create_node(
                    NodePayload::Text((TextMetadata::default(), true)),
                    &self.state,
                    window,
                    cx,
                ),
            );
        });

        self.related_id = current_slash_menu_id;

        self.open = false;
        cx.emit(SlashMenuDismissEvent {
            restore_focus: false,
        });
        cx.notify();
    }
}

impl EventEmitter<SlashMenuDismissEvent> for SlashMenu {}

impl Focusable for SlashMenu {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SlashMenu {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered_items = self.filtered_items(cx);

        div()
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|this, _: &MoveUp, _, cx| {
                this.move_selection_up(cx);
            }))
            .on_action(cx.listener(|this, _: &MoveDown, _, cx| {
                this.move_selection_down(cx);
            }))
            .on_action(cx.listener(|this, _: &Escape, _, cx| {
                this.open = false;
                cx.emit(SlashMenuDismissEvent {
                    restore_focus: true,
                });
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &Backspace, _, cx| {
                if this.mode == SlashMenuMode::Replace
                    && this.search_input.read(cx).value().is_empty()
                {
                    this.open = false;
                    cx.emit(SlashMenuDismissEvent {
                        restore_focus: true,
                    });
                    cx.notify();
                }
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => {
                        this.confirm_selection(window, cx);
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.open = false;
                        cx.emit(SlashMenuDismissEvent {
                            restore_focus: true,
                        });
                        cx.notify();
                        cx.stop_propagation();
                    }
                    _ => {}
                }
            }))
            .child(
                Popover::new("slash-menu-popover")
                    .anchor(Corner::TopLeft)
                    .trigger(Empty::default())
                    .open(self.open)
                    .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                        this.set_open(*open, window, cx);
                    }))
                    .when(self.mode == SlashMenuMode::InsertAfter, |el| el.mt_5())
                    .p_1()
                    .w(px(280.0))
                    .bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_lg()
                    .shadow_lg()
                    .child(self.render_menu_content(&filtered_items, cx)),
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
