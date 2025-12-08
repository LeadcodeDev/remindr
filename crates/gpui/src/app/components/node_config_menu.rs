use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Icon, Selectable, StyledExt,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    label::Label,
    popover::Popover,
};
use uuid::Uuid;

use crate::app::{components::node_renderer::DraggableInfo, states::node_state::NodeState};

pub struct NodeConfigMenu {
    related_id: Uuid,
    pub state: Entity<NodeState>,
    pub dragged_info: DraggableInfo,
    pub open: bool,
}

impl NodeConfigMenu {
    pub fn new(related_id: Uuid, state: &Entity<NodeState>) -> Self {
        let dragged_info = DraggableInfo {
            id: related_id.clone(),
        };

        Self {
            related_id,
            state: state.clone(),
            open: false,
            dragged_info,
        }
    }

    fn render_item(
        &self,
        label: &'static str,
        icon: Icon,
        on_click: impl Fn(&mut Self, &ClickEvent, &mut Window, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) -> Button {
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
            .child(icon)
            .child(SharedString::new(label))
            .on_click(cx.listener(on_click))
    }
}

impl Render for NodeConfigMenu {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let drag_element = DragButtonTrigger {
            id: self.related_id,
            state: self.state.clone(),
            info: self.dragged_info.clone(),
            selected: false,
        };

        div().relative().child(
            Popover::new("contextual-node-popover")
                .anchor(Corner::TopRight)
                .trigger(drag_element)
                .open(!self.state.read(cx).is_dragging && self.open)
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    this.open = *open;
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
                        .children([self.render_item(
                            "Paragraph",
                            Icon::default().path("icons/pilcrow.svg"),
                            |_, _, _, _| {},
                            cx,
                        )]),
                ),
        )
    }
}

#[derive(IntoElement)]
struct DragButtonTrigger {
    pub id: Uuid,
    pub selected: bool,
    pub info: DraggableInfo,
    pub state: Entity<NodeState>,
}

impl Selectable for DragButtonTrigger {
    fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

impl RenderOnce for DragButtonTrigger {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        div()
            .id(self.id)
            .size_6()
            .flex()
            .justify_center()
            .items_center()
            .child(
                Icon::default()
                    .path("icons/grip-vertical.svg")
                    .size_5()
                    .text_color(cx.theme().accent_foreground.opacity(0.5)),
            )
            .when(self.state.read(cx).dragging_id.is_some(), |this| {
                this.cursor_move()
            })
            .on_drag(self.info, {
                let state = self.state.clone();
                move |element, _, _window: &mut Window, cx: &mut App| {
                    state.update(cx, |state, _| state.start_drag(element.id));
                    cx.new(|_| EmptyView)
                }
            })
    }
}
