use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName, StyledExt};
use uuid::Uuid;

use crate::{entities::ui::nodes::RemindrElement, states::document_state::ViewState};

#[derive(Clone, PartialEq)]
pub enum MovingElement {
    After,
    Before,
}

#[derive(Clone)]
pub struct DragController {
    pub dragging_id: Option<Uuid>,
    pub hovered_drop_zone: Option<(Uuid, MovingElement)>,
    pub is_dragging: bool,
}

impl Default for DragController {
    fn default() -> Self {
        Self {
            dragging_id: None,
            hovered_drop_zone: None,
            is_dragging: false,
        }
    }
}

impl DragController {
    pub fn start_drag(&mut self, id: Uuid) {
        self.dragging_id = Some(id);
        self.is_dragging = true;
    }

    pub fn stop_drag(&mut self) {
        self.dragging_id = None;
        self.is_dragging = false;
        self.hovered_drop_zone = None;
    }

    pub fn update_hover_zone(
        &mut self,
        id: Uuid,
        mouse_y: f32,
        bounds_top: f32,
        bounds_height: f32,
    ) -> bool {
        let middle_y = bounds_top + bounds_height / 2.0;
        let zone = if mouse_y < middle_y {
            MovingElement::After
        } else {
            MovingElement::Before
        };

        if mouse_y >= bounds_top && mouse_y <= bounds_top + bounds_height {
            if self.hovered_drop_zone != Some((id, zone.clone())) {
                self.hovered_drop_zone = Some((id, zone.clone()));
                return true;
            }
        } else if let Some((i, _)) = self.hovered_drop_zone {
            if i == id {
                self.hovered_drop_zone = None;
                return true;
            }
        }

        false
    }

    pub fn drop_element_by_index<T>(
        &mut self,
        elements: &mut Vec<T>,
        from_index: usize,
        target_index: usize,
        position: MovingElement,
    ) {
        let element = elements.remove(from_index);

        let mut to_index = target_index;

        match position {
            MovingElement::After => {
                if from_index < target_index {
                    to_index = target_index.saturating_sub(1);
                }
            }
            MovingElement::Before => {
                if from_index >= target_index {
                    to_index = target_index + 1;
                }
            }
        }

        let final_index = to_index.clamp(0, elements.len());
        elements.insert(final_index, element);

        self.stop_drag();
    }

    pub fn on_outside<T>(&mut self, event: &DragMoveEvent<T>) -> bool {
        let mouse_position = event.event.position;
        let bounds = event.bounds;

        let is_outside = mouse_position.x < bounds.origin.x
            || mouse_position.y < bounds.origin.y
            || mouse_position.x > bounds.origin.x + bounds.size.width
            || mouse_position.y > bounds.origin.y + bounds.size.height;

        if is_outside.clone() {
            self.stop_drag();
        }

        is_outside
    }
}

pub struct DragElement {
    pub id: Uuid,
    pub child: RemindrElement,
    pub entity: Entity<RemindrElement>,
}

impl DragElement {
    pub fn new(id: Uuid, child: RemindrElement, cx: &mut Context<Self>) -> Self {
        let entity = cx.new(|_| child.clone());
        Self { id, child, entity }
    }

    fn on_drop(&self, id: Uuid, direction: MovingElement, cx: &mut Context<Self>) {
        cx.update_global::<ViewState, _>(|state, _| {
            let state = state.current.as_mut().unwrap();

            let from_id = state.drag_controller.dragging_id.unwrap();
            let from_index = state.elements.iter().position(|e| e.id == from_id).unwrap();
            let target_index = state.elements.iter().position(|e| e.id == id).unwrap();

            state.drag_controller.drop_element_by_index(
                &mut state.elements,
                from_index,
                target_index,
                direction,
            );
        });
    }
}

impl Render for DragElement {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let state = cx.global::<ViewState>();
        let document_state_entity_ref = state.current.as_ref().unwrap();

        let controller = document_state_entity_ref.drag_controller.clone();
        let is_dragging = controller.is_dragging;
        let hovered_drop_zone = controller.hovered_drop_zone.clone();

        let id = self.id;
        let child = self.child.clone();

        div()
            .group("drag_element")
            .w_full()
            .bg(cx.theme().background)
            .relative()
            .on_drag_move(
                cx.listener(move |this, event: &DragMoveEvent<RemindrElement>, _, cx| {
                    cx.update_global::<ViewState, _>(|view_state, _| {
                        let document_state_entity = view_state.current.as_mut().unwrap();
                        let controller = &mut document_state_entity.drag_controller;

                        let bounds = event.bounds;
                        let middle_y = bounds.origin.y + bounds.size.height / 2.0;
                        let mouse_y = event.event.position.y;

                        let is_in_bounds = mouse_y >= bounds.origin.y
                            && mouse_y <= bounds.origin.y + bounds.size.height;

                        if is_in_bounds {
                            let zone = if mouse_y < middle_y {
                                MovingElement::After
                            } else {
                                MovingElement::Before
                            };

                            if controller.hovered_drop_zone != Some((this.id, zone.clone())) {
                                controller.hovered_drop_zone = Some((this.id, zone.clone()));
                            }
                        } else {
                            if let Some((i, _)) = controller.hovered_drop_zone.clone() {
                                if i == this.id {
                                    controller.hovered_drop_zone = None;
                                }
                            }
                        }
                    });
                }),
            )
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
                            .when(is_dragging, |this| this.cursor_move())
                            .on_drag(
                                child.clone(),
                                move |element, _, _window: &mut Window, cx: &mut App| {
                                    cx.update_global::<ViewState, _>(|state, _| {
                                        let controller =
                                            &mut state.current.as_mut().unwrap().drag_controller;

                                        controller.dragging_id = Some(id);
                                        controller.is_dragging = true;
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
                    .child(self.entity.clone())
                    .tab_index(0)
                    .when_some(
                        match hovered_drop_zone {
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
            .when(is_dragging, |this| {
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
