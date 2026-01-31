use gpui::*;
use gpui_component::{
    Root,
    input::{Input, InputState, TabSize},
};
use serde_json::to_string_pretty;

use crate::{
    LoadingState,
    app::{
        components::nodes::{element::RemindrElement, node::RemindrNode},
        states::document_state::DocumentState,
    },
};

pub struct CodeWindow {
    editor_state: Entity<InputState>,
    document_id: i32,
    last_buffer: String,
}

impl CodeWindow {
    fn new(editor_state: Entity<InputState>, document_id: i32, initial_buffer: String) -> Self {
        Self {
            editor_state,
            document_id,
            last_buffer: initial_buffer,
        }
    }

    fn build_code_buffer(nodes: &[RemindrNode], cx: &App) -> String {
        let mut buffer = String::new();

        for node in nodes {
            let node_json = match &node.element {
                RemindrElement::Text(node) => to_string_pretty(&node.read(cx).data).unwrap(),
                RemindrElement::Heading(node) => to_string_pretty(&node.read(cx).data).unwrap(),
                RemindrElement::Divider(node) => to_string_pretty(&node.read(cx).data).unwrap(),
                RemindrElement::DatabaseView(node) => {
                    to_string_pretty(&node.read(cx).data).unwrap()
                }
            };
            buffer.push_str(&node_json);
            buffer.push('\n');
        }

        buffer
    }

    fn get_current_buffer(&self, cx: &App) -> Option<String> {
        cx.read_global::<DocumentState, _>(|state, cx| {
            state
                .documents
                .iter()
                .find(|d| d.uid == self.document_id)
                .and_then(|doc| {
                    if let LoadingState::Loaded(content) = &doc.state {
                        Some(Self::build_code_buffer(
                            &content.renderer.read(cx).state.read(cx).get_nodes(),
                            cx,
                        ))
                    } else {
                        None
                    }
                })
        })
    }

    pub fn open(title: String, document_id: i32, nodes: Vec<RemindrNode>, cx: &mut App) {
        let window_size = size(px(600.), px(500.));
        let window_bounds = Bounds::centered(None, window_size, cx);

        let window_title = format!("Code - {}", title);
        let title_clone = window_title.clone();

        cx.spawn(async move |cx| {
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(window_bounds)),
                window_min_size: Some(Size {
                    width: px(400.),
                    height: px(300.),
                }),
                kind: WindowKind::Normal,
                titlebar: Some(TitlebarOptions {
                    appears_transparent: true,
                    title: Some(title_clone.clone().into()),
                    traffic_light_position: Some(point(px(9.0), px(9.0))),
                    ..Default::default()
                }),
                ..Default::default()
            };

            let window = cx
                .open_window(options, |window, cx| {
                    let editor_buffer = Self::build_code_buffer(&nodes, cx);
                    let editor_state = cx.new(|cx| {
                        InputState::new(window, cx)
                            .code_editor("json")
                            .line_number(true)
                            .searchable(true)
                            .tab_size(TabSize {
                                tab_size: 2,
                                hard_tabs: false,
                            })
                            .default_value(editor_buffer.clone())
                    });
                    let code_window = cx.new(|cx| {
                        cx.observe_global::<DocumentState>(|_this: &mut CodeWindow, cx| {
                            cx.notify();
                        })
                        .detach();

                        CodeWindow::new(editor_state, document_id, editor_buffer)
                    });
                    cx.new(|cx| Root::new(code_window, window, cx))
                })
                .expect("failed to open code window");

            window
                .update(cx, |_, window, _| {
                    window.activate_window();
                    window.set_window_title(&title_clone);
                })
                .expect("failed to update code window");

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}

impl Render for CodeWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Only update if content has changed
        if let Some(new_buffer) = self.get_current_buffer(cx) {
            if new_buffer != self.last_buffer {
                self.last_buffer = new_buffer.clone();
                self.editor_state.update(cx, |state, cx| {
                    state.set_value(new_buffer, window, cx);
                });
            }
        }

        div().pt_8().size_full().child(
            Input::new(&self.editor_state)
                .disabled(true)
                .appearance(false)
                .size_full(),
        )
    }
}
