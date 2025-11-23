use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt};
use uuid::Uuid;

use crate::{
    controllers::drag_controller::MovingElement,
    entities::ui::nodes::{RemindrElement, node::RemindrNode},
    states::node_state::NodeState,
};

pub struct Draggable {
    id: Uuid,
    state: Entity<NodeState>,
    child: RemindrNode,
}

impl Draggable {
    pub fn new(state: Entity<NodeState>, child: RemindrNode, cx: &mut Context<Self>) -> Self {
        Self {
            id: child.id,
            child: child,
            state,
        }
    }

    fn on_drop(&self, id: Uuid, direction: MovingElement, cx: &mut Context<Self>) {
        self.state.update(cx, |state, _| {
            if let Some(dragging_id) = state.dragging_id {
                let elements = state.get_nodes();
                let from_index = elements
                    .iter()
                    .position(|e| e.id == dragging_id.clone())
                    .unwrap();

                let target_index = elements.iter().position(|e| e.id == id).unwrap();
                state.drop_element_by_index(from_index, target_index, direction);
            }
        });
    }

    fn on_drag_move(
        this: &mut Self,
        event: &DragMoveEvent<RemindrNode>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        this.state.update(cx, |state, _| {
            let bounds = event.bounds;
            let middle_y = bounds.origin.y + bounds.size.height / 2.0;
            let mouse_y = event.event.position.y;

            let is_in_bounds =
                mouse_y >= bounds.origin.y && mouse_y <= bounds.origin.y + bounds.size.height;

            if is_in_bounds {
                let zone = if mouse_y < middle_y {
                    MovingElement::After
                } else {
                    MovingElement::Before
                };

                if state.hovered_drop_zone != Some((this.id, zone.clone())) {
                    state.hovered_drop_zone = Some((this.id, zone.clone()));
                }
            } else {
                if let Some((i, _)) = state.hovered_drop_zone.clone() {
                    if i == this.id {
                        state.hovered_drop_zone = None;
                    }
                }
            }
        });
    }
}

impl Render for Draggable {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = self.state.read(cx);

        let id = self.id;
        let id_for_closure = id.clone();
        let state_entity = self.state.clone();
        let state_entity_for_closure = state_entity.clone();

        let drag_child = self.child.element.clone();
        let display_child = self.child.element.clone();

        div()
            .group("drag_element")
            .w_full()
            .bg(cx.theme().background)
            .relative()
            .on_drag_move(cx.listener(Self::on_drag_move))
            .child(
                div()
                    .invisible()
                    .group_hover("drag_element", |this| this.visible())
                    .absolute()
                    .left_0()
                    .flex()
                    .gap_1()
                    .child(
                        div()
                            .id("add_button")
                            .size_6()
                            .hover(|this| this.bg(cx.theme().background.opacity(0.3)))
                            .flex()
                            .justify_center()
                            .items_center()
                            .child(
                                Icon::new(IconName::Plus)
                                    .size_5()
                                    .text_color(cx.theme().accent_foreground.opacity(0.5)),
                            ),
                    )
                    .child(
                        div()
                            .id("drag_button")
                            .size_6()
                            .hover(|this| this.bg(cx.theme().background.opacity(0.3)).cursor_grab())
                            .flex()
                            .justify_center()
                            .items_center()
                            .child(
                                Icon::default()
                                    .path("icons/grip-vertical.svg")
                                    .size_5()
                                    .text_color(cx.theme().accent_foreground.opacity(0.5)),
                            )
                            .when(state.dragging_id.is_some(), |this| this.cursor_move())
                            .on_drag(
                                drag_child,
                                move |element: &RemindrElement,
                                      _point: Point<Pixels>,
                                      _window: &mut Window,
                                      cx: &mut App| {
                                    state_entity_for_closure.update(cx, |state, _| {
                                        state.dragging_id = Some(id_for_closure.clone());
                                        state.is_dragging = true;
                                    });

                                    cx.new(|_| element.clone())
                                },
                            ),
                    ),
            )
            .child(
                div()
                    .relative()
                    .ml_12()
                    .w_full()
                    .child(display_child.clone())
                    .tab_index(0)
                    .when_some(
                        match state_entity.read(cx).hovered_drop_zone {
                            Some((i, MovingElement::After)) if i == self.id => Some(
                                div()
                                    .absolute()
                                    .top(px(-2.0))
                                    .h(px(4.0))
                                    .debug_blue()
                                    .w_full()
                                    .border_color(cx.theme().accent_foreground.opacity(0.5))
                                    .tab_index(10),
                            ),
                            Some((i, MovingElement::Before)) if i == self.id => Some(
                                div()
                                    .absolute()
                                    .bottom(px(-2.0))
                                    .h(px(4.0))
                                    .debug_blue()
                                    .w_full()
                                    .bg(cx.theme().accent_foreground.opacity(0.5))
                                    .tab_index(10),
                            ),
                            _ => None,
                        },
                        |this, bar| this.child(bar),
                    ),
            )
            .when(state_entity.read(cx).is_dragging, |this| {
                let top_dropable_zone_element = div()
                    .absolute()
                    .tab_index(2)
                    .w_full()
                    .h_1_2()
                    .top_0()
                    .on_drop(cx.listener(move |this, _: &RemindrElement, _, cx| {
                        this.on_drop(this.id, MovingElement::After, cx);
                        cx.notify();
                    }));

                let bottom_dropable_zone_element = div()
                    .absolute()
                    .tab_index(2)
                    .w_full()
                    .h_1_2()
                    .bottom_0()
                    .on_drop(cx.listener(move |this, _: &RemindrElement, _, cx| {
                        this.on_drop(this.id, MovingElement::Before, cx);
                        cx.notify();
                    }));

                this.child(top_dropable_zone_element)
                    .child(bottom_dropable_zone_element)
            })
    }
}
