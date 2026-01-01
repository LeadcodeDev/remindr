use gpui::{prelude::FluentBuilder, *};
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

const MENU_ITEMS_COUNT: usize = 3;

pub struct SlashMenuDismissEvent {
    /// If true, the focus should be restored to the original input
    /// If false, a new element was inserted and has focus
    pub restore_focus: bool,
}

#[derive(Clone)]
pub struct SlashMenu {
    related_id: Uuid,
    pub state: Entity<NodeState>,
    pub open: bool,
    pub search: Option<SharedString>,
    pub selected_index: usize,
    pub focus_handle: FocusHandle,
}

impl SlashMenu {
    pub fn new(
        related_id: Uuid,
        state: &Entity<NodeState>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            related_id,
            state: state.clone(),
            open: false,
            search: None,
            selected_index: 0,
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else {
            self.selected_index = MENU_ITEMS_COUNT - 1;
        }
        cx.notify();
    }

    pub fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        if self.selected_index < MENU_ITEMS_COUNT - 1 {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
        cx.notify();
    }

    pub fn confirm_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.selected_index {
            0 => Self::on_insert_paragraph(self, &ClickEvent::default(), window, cx),
            1 => Self::on_insert_heading(self, &ClickEvent::default(), window, cx),
            2 => Self::on_insert_divider(self, &ClickEvent::default(), window, cx),
            _ => {}
        }
        self.selected_index = 0;
    }

    pub fn reset_selection(&mut self) {
        self.selected_index = 0;
    }

    fn render_item(
        &self,
        index: usize,
        label: &'static str,
        icon: Icon,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Button {
        let is_selected = self.selected_index == index;
        let bg_color = if is_selected {
            cx.theme().primary.opacity(0.15)
        } else {
            cx.theme().transparent
        };

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
            .bg(bg_color)
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
        cx.emit(SlashMenuDismissEvent {
            restore_focus: false,
        });
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
        cx.emit(SlashMenuDismissEvent {
            restore_focus: false,
        });
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "up" => {
                        this.move_selection_up(cx);
                        cx.stop_propagation();
                    }
                    "down" => {
                        this.move_selection_down(cx);
                        cx.stop_propagation();
                    }
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
                Popover::new("controlled-popover")
                    .anchor(Corner::TopLeft)
                    .trigger(Empty::default())
                    .open(self.open)
                    .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                        this.open = *open;
                        if *open {
                            this.focus_handle.focus(window);
                        }
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
                                    0,
                                    "Paragraph",
                                    Icon::default().path("icons/pilcrow.svg"),
                                    Self::on_insert_paragraph,
                                    cx,
                                ),
                                self.render_item(
                                    1,
                                    "Heading",
                                    Icon::default().path("icons/heading.svg"),
                                    Self::on_insert_heading,
                                    cx,
                                ),
                                self.render_item(
                                    2,
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
