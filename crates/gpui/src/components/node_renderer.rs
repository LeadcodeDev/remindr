use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName};
use serde_json::Value;
use uuid::Uuid;

use crate::states::node_state::{MovingElement, NodeState};

pub struct NodeRenderer {
    state: Entity<NodeState>,
}

pub struct DraggableInfo {
    pub id: Uuid,
}

impl NodeRenderer {
    pub fn new(nodes: Vec<Value>, window: &mut Window, app: &mut App) -> Self {
        let mut state = NodeState::default();

        for value in nodes.into_iter() {
            let node = state.parse_node(&value, window, app);
            state.push_node(&node);
        }

        let state = app.new(|_| state);
        Self { state }
    }

    fn on_drop(this: &mut Self, node_id: Uuid, direction: MovingElement, cx: &mut Context<Self>) {
        this.state.update(cx, |state, _| {
            if let Some(dragging_id) = state.dragging_id {
                let elements = state.get_nodes();
                let from_index = elements
                    .iter()
                    .position(|e| e.id == dragging_id.clone())
                    .unwrap();

                let target_index = elements.iter().position(|e| e.id == node_id).unwrap();
                state.drop_element_by_index(from_index, target_index, direction);
            }
        });
    }

    fn on_drag_move(
        node_id: Uuid,
        this: &mut Self,
        event: &DragMoveEvent<DraggableInfo>,
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

                state.hovered_drop_zone = Some((node_id, zone.clone()));
            } else {
                if let Some((i, _)) = state.hovered_drop_zone.clone() {
                    if i == node_id {
                        state.hovered_drop_zone = None;
                    }
                }
            }
        });
    }
}

impl Render for NodeRenderer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        {
            self.state
                .update(cx, |state, cx| state.register_state(self.state.clone(), cx));
        }

        let state = self.state.read(cx);
        let nodes = state.get_nodes().clone();

        let children = nodes.into_iter().map(|node| {
            let node_drag_info = DraggableInfo {
                id: node.id.clone(),
            };

            div()
                .group("drag_element")
                .on_drag_move(cx.listener(
                    move |this: &mut Self, event: &DragMoveEvent<DraggableInfo>, _, cx| {
                        Self::on_drag_move(node.id.clone(), this, event, cx);
                    },
                ))
                .relative()
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
                                .id(node.id)
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
                                .id(node.id)
                                .size_6()
                                .hover(|this| {
                                    this.bg(cx.theme().background.opacity(0.3)).cursor_grab()
                                })
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
                                .on_drag(node_drag_info, {
                                    let state = self.state.clone();
                                    move |element, _, _window: &mut Window, cx: &mut App| {
                                        state.update(cx, |state, _| state.start_drag(element.id));
                                        cx.new(|_| EmptyView)
                                    }
                                }),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .ml_12()
                        .w_full()
                        .child(node.element.clone())
                        .tab_index(0)
                        .when_some(
                            match state.hovered_drop_zone {
                                Some((i, MovingElement::After)) if i == node.id => Some(
                                    div()
                                        .absolute()
                                        .top(px(-2.0))
                                        .h(px(4.0))
                                        .w_full()
                                        .border_color(cx.theme().accent_foreground.opacity(0.5))
                                        .tab_index(10),
                                ),
                                Some((i, MovingElement::Before)) if i == node.id => Some(
                                    div()
                                        .absolute()
                                        .bottom(px(-2.0))
                                        .h(px(4.0))
                                        .w_full()
                                        .bg(cx.theme().accent_foreground.opacity(0.5))
                                        .tab_index(10),
                                ),
                                _ => None,
                            },
                            |this, bar| this.child(bar),
                        ),
                )
                .when(state.is_dragging, |this| {
                    let top_dropable_zone_element = div()
                        .absolute()
                        .tab_index(2)
                        .w_full()
                        .h_1_2()
                        .top_0()
                        .on_drop(
                            cx.listener(move |this: &mut Self, _: &DraggableInfo, _, cx| {
                                Self::on_drop(this, node.id.clone(), MovingElement::After, cx)
                            }),
                        );

                    let bottom_dropable_zone_element = div()
                        .absolute()
                        .tab_index(2)
                        .w_full()
                        .h_1_2()
                        .bottom_0()
                        .on_drop(
                            cx.listener(move |this: &mut Self, _: &DraggableInfo, _, cx| {
                                Self::on_drop(this, node.id.clone(), MovingElement::Before, cx)
                            }),
                        );

                    this.child(top_dropable_zone_element)
                        .child(bottom_dropable_zone_element)
                })
        });

        div()
            .size_full()
            .bg(cx.theme().background)
            .children(children)
    }
}
