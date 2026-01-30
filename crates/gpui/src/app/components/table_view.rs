use std::collections::HashMap;

use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    checkbox::Checkbox,
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, PopupMenuItem},
    scroll::ScrollableElement,
    v_flex,
};

use crate::{
    LoadingState,
    app::states::{
        database_state::{DatabaseState, LoadedDatabaseView},
        repository_state::RepositoryState,
    },
    domain::database::database_column::{ColumnType, DatabaseColumnModel},
};

pub struct TableView {
    pub key: i64,
    pub database_id: i32,
    cell_inputs: HashMap<(i32, i32), Entity<InputState>>,
    editing_column_id: Option<i32>,
    column_label_input: Option<Entity<InputState>>,
}

impl TableView {
    pub fn new(key: i64, database_id: i32) -> Self {
        Self {
            key,
            database_id,
            cell_inputs: HashMap::new(),
            editing_column_id: None,
            column_label_input: None,
        }
    }

    /// Fetch fresh data from DB and update ALL opened tabs sharing this database_id
    pub fn reload_into_global(database_id: i32, cx: &mut App) {
        let db_repo = cx.global::<RepositoryState>().databases.clone();

        cx.spawn(async move |cx| {
            let database = db_repo.get_database_by_id(database_id).await?;
            let columns = db_repo.get_columns(database_id).await?;
            let rows = db_repo.get_rows(database_id).await?;
            let row_ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
            let cells = db_repo.get_cells(&row_ids).await?;

            let mut cells_map = HashMap::new();
            for cell in cells {
                cells_map.insert((cell.row_id, cell.column_id), cell.value);
            }

            let loaded = LoadedDatabaseView {
                database,
                columns,
                rows,
                cells: cells_map,
            };

            let _ = cx.update(|cx| {
                cx.update_global::<DatabaseState, _>(|state, _| {
                    state.set_all_views_loaded_for_database(database_id, loaded);
                });
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn start_column_rename(
        &mut self,
        column_id: i32,
        current_label: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(current_label.to_string(), window, cx);
            state
        });

        let key = self.key;
        let database_id = self.database_id;

        cx.subscribe_in(
            &input,
            window,
            move |this, _input, event: &InputEvent, _window, cx| {
                match event {
                    InputEvent::PressEnter { .. } => {
                        if let Some(input) = &this.column_label_input {
                            let new_label = input.read(cx).value().to_string();
                            if !new_label.is_empty() {
                                let db_repo = cx.global::<RepositoryState>().databases.clone();
                                let k = key;
                                let did = database_id;

                                // Read current column data from global state
                                let column_model =
                                    cx.read_global::<DatabaseState, _>(|state, _| {
                                        state
                                            .opened_views
                                            .iter()
                                            .find(|v| v.unique_key() == k)
                                            .and_then(|v| {
                                                if let LoadingState::Loaded(data) = &v.state {
                                                    data.columns
                                                        .iter()
                                                        .find(|c| c.id == column_id)
                                                        .cloned()
                                                } else {
                                                    None
                                                }
                                            })
                                    });

                                if let Some(mut col) = column_model {
                                    col.label = new_label;
                                    cx.spawn(async move |_, cx| {
                                        db_repo.update_column(col).await?;
                                        let _ = cx.update(|cx| {
                                            TableView::reload_into_global(did, cx);
                                        });
                                        Ok::<_, anyhow::Error>(())
                                    })
                                    .detach();
                                }
                            }
                        }
                        this.editing_column_id = None;
                        this.column_label_input = None;
                    }
                    InputEvent::Blur => {
                        this.editing_column_id = None;
                        this.column_label_input = None;
                    }
                    _ => {}
                }
            },
        )
        .detach();

        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        self.editing_column_id = Some(column_id);
        self.column_label_input = Some(input);
    }

    fn get_or_create_cell_input(
        &mut self,
        row_id: i32,
        column_id: i32,
        value: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.cell_inputs.get(&(row_id, column_id)) {
            return input.clone();
        }

        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(value.to_string(), window, cx);
            state
        });

        let did = self.database_id;

        cx.subscribe_in(
            &input,
            window,
            move |_this, input_entity, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    let new_value = input_entity.read(cx).value().to_string();
                    let db_repo = cx.global::<RepositoryState>().databases.clone();

                    cx.spawn(async move |_, cx| {
                        db_repo.upsert_cell(row_id, column_id, &new_value).await?;
                        // Update cells in ALL opened tabs sharing the same database_id
                        let _ = cx.update(|cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                for v in state
                                    .opened_views
                                    .iter_mut()
                                    .filter(|v| v.database_id == did)
                                {
                                    if let LoadingState::Loaded(data) = &mut v.state {
                                        data.cells.insert((row_id, column_id), new_value.clone());
                                    }
                                }
                            });
                        });
                        Ok::<_, anyhow::Error>(())
                    })
                    .detach();
                }
            },
        )
        .detach();

        self.cell_inputs.insert((row_id, column_id), input.clone());
        input
    }

    fn render_header(
        &self,
        columns: &[DatabaseColumnModel],
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let header_bg = cx.theme().table_head;
        let border_color = cx.theme().border;
        let text_color = cx.theme().foreground;
        let icon_color = cx.theme().foreground.opacity(0.5);

        let view_key = self.key;
        let database_id = self.database_id;

        let mut header = h_flex()
            .w_full()
            .min_h_9()
            .bg(header_bg)
            .border_b_1()
            .border_color(border_color);

        for col in columns {
            let col_id = col.id;
            let col_label = col.label.clone();
            let col_type = col.column_type.clone();
            let is_editing = self.editing_column_id == Some(col_id);

            let type_icon_path = col_type.icon_path();

            let mut cell = h_flex()
                .w(px(180.0))
                .min_w(px(180.0))
                .h_full()
                .px_2()
                .gap_1p5()
                .items_center()
                .border_r_1()
                .border_color(border_color);

            // Type icon with context menu to change type
            let vid = view_key;
            let did = database_id;
            cell = cell.child(
                div()
                    .id(("col-type-icon", col_id as usize))
                    .cursor_pointer()
                    .child(
                        Icon::default()
                            .path(type_icon_path)
                            .size_4()
                            .text_color(icon_color),
                    )
                    .context_menu({
                        let col_type = col_type.clone();
                        move |menu, _, _| {
                            let types = [
                                (ColumnType::String, "Text", "icons/type.svg"),
                                (ColumnType::Int, "Number", "icons/hash.svg"),
                                (ColumnType::Bool, "Checkbox", "icons/check-square.svg"),
                                (ColumnType::Select, "Select", "icons/list.svg"),
                            ];

                            let mut m = menu;
                            for (ct, label, icon) in &types {
                                if *ct == col_type {
                                    continue;
                                }
                                let new_type = ct.clone();
                                m = m.item(
                                    PopupMenuItem::new(*label)
                                        .icon(Icon::default().path(*icon))
                                        .on_click({
                                            let new_type = new_type.clone();
                                            move |_, _, cx| {
                                                let db_repo = cx
                                                    .global::<RepositoryState>()
                                                    .databases
                                                    .clone();
                                                let new_type = new_type.clone();

                                                // Read current column
                                                let column_model = cx
                                                    .read_global::<DatabaseState, _>(|state, _| {
                                                        state
                                                            .opened_views
                                                            .iter()
                                                            .find(|v| v.unique_key() == vid)
                                                            .and_then(|v| {
                                                                if let LoadingState::Loaded(data) =
                                                                    &v.state
                                                                {
                                                                    data.columns
                                                                        .iter()
                                                                        .find(|c| c.id == col_id)
                                                                        .cloned()
                                                                } else {
                                                                    None
                                                                }
                                                            })
                                                    });

                                                if let Some(mut col) = column_model {
                                                    col.column_type = new_type;
                                                    cx.spawn(async move |cx| {
                                                        db_repo.update_column(col).await?;
                                                        let _ = cx.update(|cx| {
                                                            TableView::reload_into_global(did, cx);
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();
                                                }
                                            }
                                        }),
                                );
                            }

                            m.separator().item(
                                PopupMenuItem::new("Delete column")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click(move |_, _, cx| {
                                        let db_repo =
                                            cx.global::<RepositoryState>().databases.clone();
                                        cx.spawn(async move |cx| {
                                            db_repo.delete_column(col_id).await?;
                                            let _ = cx.update(|cx| {
                                                TableView::reload_into_global(did, cx);
                                            });
                                            Ok::<_, anyhow::Error>(())
                                        })
                                        .detach();
                                    }),
                            )
                        }
                    }),
            );

            // Column label (editable on double-click)
            if is_editing {
                if let Some(input) = &self.column_label_input {
                    cell = cell.child(
                        div()
                            .flex_1()
                            .child(Input::new(input).xsmall().appearance(false).text_sm()),
                    );
                }
            } else {
                cell = cell.child({
                    let label = col_label.clone();
                    div()
                        .id(("col-label", col_id as usize))
                        .flex_1()
                        .text_sm()
                        .text_color(text_color.opacity(0.7))
                        .text_ellipsis()
                        .overflow_hidden()
                        .child(col_label)
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                                if event.click_count == 2 {
                                    cx.stop_propagation();
                                    this.start_column_rename(col_id, &label, window, cx);
                                }
                            }),
                        )
                });
            }

            header = header.child(cell);
        }

        // "+" button to add a new column
        header = header.child(
            div()
                .w(px(40.0))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    Button::new("add-column-btn")
                        .icon(Icon::new(IconName::Plus))
                        .ghost()
                        .xsmall()
                        .cursor_pointer()
                        .tooltip("Add column")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let db_repo = cx.global::<RepositoryState>().databases.clone();
                            let did = this.database_id;
                            let vid = this.key;

                            // Get next position
                            let next_pos = cx.read_global::<DatabaseState, _>(|state, _| {
                                state
                                    .opened_views
                                    .iter()
                                    .find(|v| v.unique_key() == vid)
                                    .and_then(|v| {
                                        if let LoadingState::Loaded(data) = &v.state {
                                            Some(data.columns.len() as i32)
                                        } else {
                                            None
                                        }
                                    })
                                    .unwrap_or(0)
                            });

                            cx.spawn(async move |_, cx| {
                                db_repo
                                    .insert_column(
                                        did,
                                        "Column".to_string(),
                                        "string",
                                        "{}",
                                        next_pos,
                                    )
                                    .await?;
                                let _ = cx.update(|cx| {
                                    TableView::reload_into_global(did, cx);
                                });
                                Ok::<_, anyhow::Error>(())
                            })
                            .detach();
                        })),
                ),
        );

        header
    }

    fn render_rows(
        &mut self,
        columns: &[DatabaseColumnModel],
        rows: &[(i32, HashMap<i32, String>)],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let border_color = cx.theme().border;
        let database_id = self.database_id;

        let mut row_elements = Vec::new();

        for (row_id, row_cells) in rows {
            let row_id = *row_id;

            let mut row_el = h_flex()
                .w_full()
                .min_h_9()
                .border_b_1()
                .border_color(border_color);

            for col in columns {
                let col_id = col.id;
                let value = row_cells.get(&col_id).cloned().unwrap_or_default();

                let cell_el = match &col.column_type {
                    ColumnType::Bool => {
                        let is_checked = value == "true";
                        let did = database_id;

                        div()
                            .w(px(180.0))
                            .min_w(px(180.0))
                            .h_full()
                            .px_2()
                            .flex()
                            .items_center()
                            .border_r_1()
                            .border_color(border_color)
                            .child(
                                Checkbox::new(ElementId::Name(
                                    format!("cell-check-{}-{}", row_id, col_id).into(),
                                ))
                                .checked(is_checked)
                                .on_click(cx.listener(
                                    move |_this, checked: &bool, _, cx| {
                                        let new_val = if *checked { "true" } else { "false" };
                                        let db_repo =
                                            cx.global::<RepositoryState>().databases.clone();

                                        // Update ALL opened tabs sharing the same database_id
                                        cx.update_global::<DatabaseState, _>(|state, _| {
                                            for v in state
                                                .opened_views
                                                .iter_mut()
                                                .filter(|v| v.database_id == did)
                                            {
                                                if let LoadingState::Loaded(data) = &mut v.state {
                                                    data.cells.insert(
                                                        (row_id, col_id),
                                                        new_val.to_string(),
                                                    );
                                                }
                                            }
                                        });

                                        let val = new_val.to_string();
                                        cx.spawn(async move |_, _cx| {
                                            db_repo.upsert_cell(row_id, col_id, &val).await?;
                                            Ok::<_, anyhow::Error>(())
                                        })
                                        .detach();
                                    },
                                )),
                            )
                    }
                    _ => {
                        // Text input for String, Int, Select
                        let input =
                            self.get_or_create_cell_input(row_id, col_id, &value, window, cx);
                        div()
                            .w(px(180.0))
                            .min_w(px(180.0))
                            .h_full()
                            .px_1()
                            .flex()
                            .items_center()
                            .border_r_1()
                            .border_color(border_color)
                            .child(Input::new(&input).xsmall().appearance(false).text_sm())
                    }
                };

                row_el = row_el.child(cell_el);
            }

            // Delete row button at end
            row_el = row_el.child(
                div()
                    .w(px(40.0))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .opacity(0.0)
                    .hover(|el| el.opacity(1.0))
                    .child(
                        Button::new(("delete-row", row_id as usize))
                            .icon(Icon::default().path("icons/trash-2.svg"))
                            .danger()
                            .xsmall()
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let db_repo = cx.global::<RepositoryState>().databases.clone();
                                let did = this.database_id;

                                cx.spawn(async move |_, cx| {
                                    db_repo.delete_row(row_id).await?;
                                    let _ = cx.update(|cx| {
                                        TableView::reload_into_global(did, cx);
                                    });
                                    Ok::<_, anyhow::Error>(())
                                })
                                .detach();

                                // Clean up cell inputs for this row
                                this.cell_inputs.retain(|(r, _), _| *r != row_id);
                            })),
                    ),
            );

            row_elements.push(row_el);
        }

        row_elements
    }
}

impl Render for TableView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view_key = self.key;

        // Read data from global state
        let data = cx.read_global::<DatabaseState, _>(|state, _| {
            state
                .opened_views
                .iter()
                .find(|v| v.unique_key() == view_key)
                .and_then(|v| {
                    if let LoadingState::Loaded(data) = &v.state {
                        Some(data.clone())
                    } else {
                        None
                    }
                })
        });

        let Some(data) = data else {
            return v_flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .child("Loading...")
                .into_any_element();
        };

        let columns = data.columns.clone();
        let rows_data: Vec<(i32, HashMap<i32, String>)> = data
            .rows
            .iter()
            .map(|row| {
                let row_cells: HashMap<i32, String> = columns
                    .iter()
                    .map(|col| {
                        let val = data
                            .cells
                            .get(&(row.id, col.id))
                            .cloned()
                            .unwrap_or_default();
                        (col.id, val)
                    })
                    .collect();
                (row.id, row_cells)
            })
            .collect();

        let border_color = cx.theme().border;

        let header = self.render_header(&columns, cx).into_any_element();
        let row_elements = self.render_rows(&columns, &rows_data, window, cx);

        v_flex()
            .w_full()
            .h_full()
            .overflow_hidden()
            .child(
                div().flex_1().min_h_0().overflow_scrollbar().child(
                    v_flex()
                        .min_w_full()
                        .child(header)
                        .children(row_elements)
                        .child(
                            // "+ New row" button
                            h_flex()
                                .w_full()
                                .min_h_9()
                                .px_2()
                                .items_center()
                                .border_b_1()
                                .border_color(border_color)
                                .child(
                                    Button::new("add-row-btn")
                                        .label("+ New row")
                                        .ghost()
                                        .xsmall()
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            let db_repo =
                                                cx.global::<RepositoryState>().databases.clone();
                                            let did = this.database_id;

                                            cx.spawn(async move |_, cx| {
                                                db_repo.insert_row(did).await?;
                                                let _ = cx.update(|cx| {
                                                    TableView::reload_into_global(did, cx);
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        })),
                                ),
                        ),
                ),
            )
            .into_any_element()
    }
}
