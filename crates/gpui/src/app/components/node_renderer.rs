use gpui::{prelude::FluentBuilder, *};
use gpui_component::{ActiveTheme, Icon, IconName};
use serde_json::Value;
use uuid::Uuid;

use crate::app::{
    components::{
        node_config_menu::NodeConfigMenu,
        nodes::{
            element::{NodePayload, RemindrElement},
            text::data::TextMetadata,
        },
        slash_menu::{SlashMenu, SlashMenuMode},
    },
    states::node_state::{MovingElement, NodeState},
};

pub struct NodeRenderer {
    pub state: Entity<NodeState>,
    insert_menu: Entity<SlashMenu>,
    config_menus: Vec<Entity<NodeConfigMenu>>,
}

#[derive(Clone)]
pub struct DraggableInfo {
    pub id: Uuid,
}

impl NodeRenderer {
    pub fn new(nodes: Vec<Value>, window: &mut Window, cx: &mut App) -> Self {
        let state = cx.new(|_| NodeState::default());

        state.update(cx, |this, cx| {
            for value in nodes.into_iter() {
                let node = this.parse_node(&value, &state, window, cx);
                this.push_node(&node);
            }
        });

        let insert_menu = cx.new(|cx| {
            SlashMenu::new(Uuid::nil(), &state, window, cx).with_mode(SlashMenuMode::InsertAfter)
        });

        Self {
            state,
            insert_menu,
            config_menus: Vec::new(),
        }
    }

    fn get_or_create_config_menu(
        &mut self,
        node_id: Uuid,
        cx: &mut Context<Self>,
    ) -> Entity<NodeConfigMenu> {
        if let Some(menu) = self
            .config_menus
            .iter()
            .find(|m| m.read(cx).related_id == node_id)
        {
            return menu.clone();
        }

        let menu = cx.new(|cx| NodeConfigMenu::new(node_id, &self.state, cx));
        self.config_menus.push(menu.clone());
        menu
    }

    fn open_insert_menu(&mut self, node_id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        self.insert_menu.update(cx, |menu, cx| {
            menu.set_related_id(node_id);
            menu.set_open(true, window, cx);
        });
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

    fn on_create_text_zone(
        this: &mut Self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        this.state.update(cx, |state, cx| {
            state.push_node(&RemindrElement::create_node(
                NodePayload::Text((TextMetadata::default(), true)),
                &this.state,
                window,
                cx,
            ));
        })
    }
}

impl Render for NodeRenderer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let nodes = {
            let state = self.state.read(cx);
            state.get_nodes().clone()
        };

        let (is_dragging, hovered_drop_zone) = {
            let state = self.state.read(cx);
            (state.is_dragging.clone(), state.hovered_drop_zone.clone())
        };

        let children = nodes.into_iter().map(|node| {
            div()
                .group("drag_element")
                .on_drag_move(cx.listener(
                    move |this: &mut Self, event: &DragMoveEvent<DraggableInfo>, _, cx| {
                        Self::on_drag_move(node.id.clone(), this, event, cx);
                    },
                ))
                .relative()
                .flex()
                .items_start()
                .child(
                    div()
                        .invisible()
                        .group_hover("drag_element", |this| this.visible())
                        .absolute()
                        .left_0()
                        .top_3()
                        .h_6()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child({
                            let menu_related_id = self.insert_menu.read(cx).related_id();
                            let show_menu_here = menu_related_id == node.id;

                            div()
                                .id(SharedString::from(format!("plus-btn-{}", node.id)))
                                .size_6()
                                .hover(|this| this.bg(cx.theme().background.opacity(0.3)))
                                .cursor_pointer()
                                .flex()
                                .justify_center()
                                .items_center()
                                .child(
                                    Icon::new(IconName::Plus)
                                        .size_5()
                                        .text_color(cx.theme().accent_foreground.opacity(0.5)),
                                )
                                .on_click(cx.listener({
                                    let node_id = node.id;
                                    move |this, _, window, cx| {
                                        this.open_insert_menu(node_id, window, cx);
                                    }
                                }))
                                .when(show_menu_here, |el| el.child(self.insert_menu.clone()))
                        })
                        .child(self.get_or_create_config_menu(node.id, cx)),
                )
                .child(
                    div()
                        .relative()
                        .ml_12()
                        .w_full()
                        .child(node.element.clone())
                        .tab_index(0)
                        .when_some(
                            match hovered_drop_zone {
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
                .when(is_dragging, |this| {
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

        div().size_full().children(children).child(
            div()
                .id("add_element")
                .cursor_pointer()
                .ml_12()
                .h_20()
                .w_full()
                .on_click(cx.listener(Self::on_create_text_zone)),
        )
    }
}
