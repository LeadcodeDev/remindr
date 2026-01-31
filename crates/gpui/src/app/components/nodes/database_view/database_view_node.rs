use std::collections::HashMap;

use anyhow::Error;
use gpui::*;
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    popover::Popover,
    v_flex,
};
use serde_json::{Value, from_value};
use uuid::Uuid;

use crate::{
    app::{
        components::{
            nodes::{
                database_view::data::DatabaseViewNodeData,
                menu_provider::{NodeMenuItem, NodeMenuProvider},
            },
            table_view::{
                ColumnDragGhost, DraggableColumn, TableView, reorder_column,
                toggle_column_visibility,
            },
        },
        states::{
            database_state::{DatabaseState, DatabaseTabKind, LoadedDatabaseView},
            node_state::NodeState,
            repository_state::RepositoryState,
        },
    },
    domain::database::{
        database_column::DatabaseColumnModel, database_model::DatabaseModel,
        database_view::DatabaseViewModel,
    },
};

pub struct DatabaseViewNode {
    pub id: Uuid,
    pub data: DatabaseViewNodeData,
    pub state: Option<Entity<NodeState>>,
    // Runtime state
    table_views: HashMap<i32, Entity<TableView>>,
    loaded_views: Vec<DatabaseViewModel>,
    loaded_data: Option<LoadedDatabaseView>,
    loading: bool,
    selected_tab: usize,
    show_config_popover: bool,
    available_databases: Vec<DatabaseModel>,
    show_view_columns_popover: bool,
    columns_search_input: Option<Entity<InputState>>,
    renaming_tab_index: Option<usize>,
    tab_rename_input: Option<Entity<InputState>>,
    tab_rename_width: f32,
}

impl DatabaseViewNode {
    pub fn parse(
        data: &Value,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Self, Error> {
        let data = from_value::<DatabaseViewNodeData>(data.clone())?;

        // Register existing view IDs as embedded so sidebar hides them
        if !data.metadata.view_ids.is_empty() {
            let view_ids = data.metadata.view_ids.clone();
            cx.update_global::<DatabaseState, _>(|state, _| {
                for vid in &view_ids {
                    state.embedded_view_ids.insert(*vid);
                }
            });
        }

        Ok(Self {
            id: data.id,
            data: data.clone(),
            state: None,
            table_views: HashMap::new(),
            loaded_views: Vec::new(),
            loaded_data: None,
            loading: false,
            selected_tab: 0,
            show_config_popover: false,
            available_databases: Vec::new(),
            show_view_columns_popover: false,
            columns_search_input: None,
            renaming_tab_index: None,
            tab_rename_input: None,
            tab_rename_width: 0.0,
        })
    }

    fn is_configured(&self) -> bool {
        self.data.metadata.database_id > 0 && !self.data.metadata.view_ids.is_empty()
    }

    fn load_data(&mut self, cx: &mut Context<Self>) {
        if self.loading || !self.is_configured() {
            return;
        }
        self.loading = true;

        let db_repo = cx.global::<RepositoryState>().databases.clone();
        let database_id = self.data.metadata.database_id;
        let view_ids = self.data.metadata.view_ids.clone();

        cx.spawn(async move |this, cx| {
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

            // Load view metadata for each view_id
            let mut views = Vec::new();
            for vid in &view_ids {
                if let Ok(view) = db_repo.get_view_by_id(*vid).await {
                    views.push(view);
                }
            }

            // Load per-view column overrides
            let mut view_col_map: HashMap<i32, Option<Vec<i32>>> = HashMap::new();
            for vid in &view_ids {
                match db_repo.get_view_columns(*vid).await {
                    Ok(vc) if !vc.is_empty() => {
                        view_col_map.insert(*vid, Some(vc.iter().map(|v| v.column_id).collect()));
                    }
                    _ => {
                        view_col_map.insert(*vid, None);
                    }
                }
            }

            let _ = cx.update(|cx| {
                let _ = this.update(cx, |this, _cx| {
                    this.loaded_data = Some(loaded);
                    this.loaded_views = views;
                    this.loading = false;
                });
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn load_available_databases(&mut self, cx: &mut Context<Self>) {
        let db_repo = cx.global::<RepositoryState>().databases.clone();

        cx.spawn(async move |this, cx| {
            let databases = db_repo.get_databases().await?;
            let _ = cx.update(|cx| {
                let _ = this.update(cx, |this, cx| {
                    this.available_databases = databases;
                    cx.notify();
                });
            });
            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn create_view_and_configure(&mut self, database_id: i32, cx: &mut Context<Self>) {
        let db_repo = cx.global::<RepositoryState>().databases.clone();

        cx.spawn(async move |this, cx| {
            let view_id = db_repo
                .insert_view(database_id, "Table View".to_string(), "table", None)
                .await?;

            let _ = cx.update(|cx| {
                let _ = this.update(cx, |node, cx| {
                    node.data.metadata.database_id = database_id;
                    node.data.metadata.view_ids = vec![view_id];
                    node.show_config_popover = false;
                    node.table_views.clear();
                    node.loaded_data = None;
                    node.loading = false;

                    // Register as embedded so sidebar hides it
                    cx.update_global::<DatabaseState, _>(|state, _| {
                        state.embedded_view_ids.insert(view_id);
                    });

                    // Mark document as changed
                    if let Some(state) = &node.state {
                        state.update(cx, |_, _| {});
                    }

                    node.load_data(cx);
                    cx.notify();
                });
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn add_view(&mut self, cx: &mut Context<Self>) {
        let db_repo = cx.global::<RepositoryState>().databases.clone();
        let database_id = self.data.metadata.database_id;
        let view_count = self.data.metadata.view_ids.len() + 1;
        let view_name = format!("Table View {}", view_count);

        cx.spawn(async move |this, cx| {
            let view_id = db_repo
                .insert_view(database_id, view_name, "table", None)
                .await?;

            let _ = cx.update(|cx| {
                let _ = this.update(cx, |node, cx| {
                    node.data.metadata.view_ids.push(view_id);
                    node.selected_tab = node.data.metadata.view_ids.len() - 1;

                    // Reload views metadata
                    node.loaded_views.clear();
                    node.loaded_data = None;
                    node.loading = false;

                    // Register as embedded so sidebar hides it
                    cx.update_global::<DatabaseState, _>(|state, _| {
                        state.embedded_view_ids.insert(view_id);
                    });

                    // Mark document as changed
                    if let Some(state) = &node.state {
                        state.update(cx, |_, _| {});
                    }

                    node.load_data(cx);
                    cx.notify();
                });
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }

    fn finish_tab_rename(
        &mut self,
        tab_index: usize,
        _view_id: i32,
        new_name: &str,
        cx: &mut Context<Self>,
    ) {
        let new_name = new_name.trim().to_string();
        if !new_name.is_empty() {
            if let Some(view) = self.loaded_views.get_mut(tab_index) {
                view.name = new_name.clone();
            }

            let db_repo = cx.global::<RepositoryState>().databases.clone();
            let mut model = self.loaded_views.get(tab_index).cloned();
            if let Some(ref mut m) = model {
                m.name = new_name;
                let m = m.clone();
                cx.spawn(async move |_, _cx| {
                    db_repo.update_view(m).await?;
                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
        }

        self.renaming_tab_index = None;
        self.tab_rename_input = None;
        cx.notify();
    }

    fn get_or_create_table_view(
        &mut self,
        view_id: i32,
        _view_name: &str,
        cx: &mut Context<Self>,
    ) -> Entity<TableView> {
        if let Some(entity) = self.table_views.get(&view_id) {
            return entity.clone();
        }

        let key = view_id as i64;
        let entity = cx.new(|_| {
            TableView::new(
                key,
                self.data.metadata.database_id,
                view_id,
                DatabaseTabKind::View,
            )
        });
        self.table_views.insert(view_id, entity.clone());
        entity
    }

    fn render_unconfigured(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let border_color = cx.theme().border;
        let muted_color = cx.theme().foreground.opacity(0.5);

        if self.show_config_popover {
            let available_databases = self.available_databases.clone();
            let this = cx.entity().clone();

            Popover::new("db-view-config-popover")
                .trigger(
                    Button::new("db-view-config-btn")
                        .label("Configure Database View")
                        .icon(Icon::default().path("icons/database.svg"))
                        .ghost()
                        .small()
                        .cursor_pointer(),
                )
                .open(true)
                .on_open_change(cx.listener(|this, open: &bool, _, cx| {
                    if !*open {
                        this.show_config_popover = false;
                        cx.notify();
                    }
                }))
                .content({
                    let this = this.clone();
                    move |_, _, cx| {
                        let mut content = v_flex().w(px(300.0)).p_2().gap_2();
                        let muted_fg = cx.theme().foreground.opacity(0.5);

                        content = content.child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Select a database source"),
                        );

                        if available_databases.is_empty() {
                            content = content.child(
                                div()
                                    .text_sm()
                                    .text_color(muted_fg)
                                    .child("No databases available"),
                            );
                        } else {
                            for db in &available_databases {
                                let db_id = db.id;
                                let db_name = db.name.clone();
                                let this = this.clone();

                                content = content.child(
                                    Button::new(SharedString::from(format!("db-select-{}", db_id)))
                                        .label(db_name)
                                        .icon(Icon::default().path("icons/database.svg"))
                                        .ghost()
                                        .small()
                                        .w_full()
                                        .on_click(move |_, _, cx| {
                                            this.update(cx, |node, cx| {
                                                node.create_view_and_configure(db_id, cx);
                                            });
                                        }),
                                );
                            }
                        }

                        content
                    }
                })
                .into_any_element()
        } else {
            div()
                .id("db-view-unconfigured")
                .w_full()
                .py_4()
                .px_4()
                .flex()
                .gap_2()
                .items_center()
                .justify_center()
                .border_1()
                .border_color(border_color)
                .rounded_lg()
                .cursor_pointer()
                .child(
                    Icon::default()
                        .path("icons/database.svg")
                        .size_5()
                        .text_color(muted_color),
                )
                .child(
                    Button::new("configure-db-view")
                        .label("Configure Database View")
                        .ghost()
                        .small()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.show_config_popover = true;
                            this.load_available_databases(cx);
                            cx.notify();
                        })),
                )
                .into_any_element()
        }
    }

    fn render_configured(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border_color = cx.theme().border;

        if self.loaded_data.is_none() && !self.loading {
            self.load_data(cx);
        }

        if self.loaded_data.is_none() {
            return div()
                .w_full()
                .py_8()
                .items_center()
                .justify_center()
                .flex()
                .child("Loading...")
                .into_any_element();
        }

        let views = &self.loaded_views;
        if views.is_empty() {
            return div()
                .w_full()
                .py_4()
                .items_center()
                .justify_center()
                .flex()
                .child("No views loaded")
                .into_any_element();
        }

        let selected = self.selected_tab.min(views.len().saturating_sub(1));

        // Get selected view
        let selected_view = views.get(selected).cloned();

        let mut container = v_flex().w_full().gap_2();

        // Build the settings icon (+ columns popover) for the TabBar suffix
        let settings_el: AnyElement = if let Some(ref view) = selected_view {
            let table_view = self.table_views.get(&view.id).cloned();
            let view_key = view.id as i64;

            if self.show_view_columns_popover {
                // Create search input if not yet created
                if self.columns_search_input.is_none() {
                    let input = cx.new(|cx| {
                        InputState::new(_window, cx).placeholder("Rechercher une propriete...")
                    });
                    self.columns_search_input = Some(input);
                }
                let search_input = self.columns_search_input.clone().unwrap();
                let search_query = search_input.read(cx).value().to_lowercase();

                let all_columns: Vec<DatabaseColumnModel> = self
                    .loaded_data
                    .as_ref()
                    .map(|d| d.columns.clone())
                    .unwrap_or_default();

                Popover::new("view-columns-popover")
                    .trigger(
                        Button::new("view-settings-btn")
                            .icon(Icon::default().path("icons/settings.svg"))
                            .ghost()
                            .xsmall()
                            .cursor_pointer(),
                    )
                    .open(true)
                    .on_open_change(cx.listener(|this, open: &bool, window, cx| {
                        if !*open {
                            this.show_view_columns_popover = false;
                            if let Some(input) = &this.columns_search_input {
                                input.update(cx, |state, cx| {
                                    state.set_value("".to_string(), window, cx);
                                });
                            }
                            cx.notify();
                        }
                    }))
                    .content({
                        let all_cols = all_columns.clone();
                        let search_input_for_content = search_input.clone();
                        let search_q = search_query.clone();
                        let table_view = table_view.clone();
                        move |_, _window, cx| {
                            let show_id = table_view
                                .as_ref()
                                .map(|tv| tv.read(cx).show_id_column())
                                .unwrap_or(true);
                            let text_color = cx.theme().foreground;
                            let muted_color = cx.theme().foreground.opacity(0.5);
                            let border = cx.theme().border;
                            let accent = cx.theme().accent_foreground;

                            let fresh_vc_ids = cx.read_global::<DatabaseState, _>(|state, _| {
                                state
                                    .opened_views
                                    .iter()
                                    .find(|v| v.unique_key() == view_key)
                                    .and_then(|v| v.view_column_ids.clone())
                            });

                            let mut visible_cols: Vec<&DatabaseColumnModel> = Vec::new();
                            let mut hidden_cols: Vec<&DatabaseColumnModel> = Vec::new();

                            let matches_search = |col: &DatabaseColumnModel| -> bool {
                                search_q.is_empty() || col.label.to_lowercase().contains(&search_q)
                            };

                            match &fresh_vc_ids {
                                Some(ids) => {
                                    for id in ids {
                                        if let Some(col) = all_cols.iter().find(|c| c.id == *id) {
                                            if matches_search(col) {
                                                visible_cols.push(col);
                                            }
                                        }
                                    }
                                    for col in &all_cols {
                                        if !ids.contains(&col.id) && matches_search(col) {
                                            hidden_cols.push(col);
                                        }
                                    }
                                }
                                None => {
                                    for col in &all_cols {
                                        if matches_search(col) {
                                            visible_cols.push(col);
                                        }
                                    }
                                }
                            }

                            let mut content = v_flex().w(px(280.0));

                            // Search input
                            content = content.child(
                                div().p_1().border_b_1().border_color(border).child(
                                    Input::new(&search_input_for_content)
                                        .small()
                                        .appearance(false)
                                        .prefix(
                                            Icon::default()
                                                .path("icons/search.svg")
                                                .size_3()
                                                .text_color(muted_color),
                                        ),
                                ),
                            );

                            let id_matches_search = search_q.is_empty() || "id".contains(&search_q);
                            let has_visible =
                                !visible_cols.is_empty() || (show_id && id_matches_search);
                            let has_hidden =
                                !hidden_cols.is_empty() || (!show_id && id_matches_search);

                            // "Affiche dans la table" section
                            if has_visible {
                                content = content.child(
                                    v_flex().py_1().px_2().gap_0p5().child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_color)
                                                    .child("Affiche dans la table"),
                                            )
                                            .child({
                                                let table_view = table_view.clone();
                                                div()
                                                    .id("hide-all-btn")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(accent)
                                                    .child("Tout masquer")
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(tv) = &table_view {
                                                            tv.update(cx, |state, _| {
                                                                state.set_show_id_column(false);
                                                            });
                                                        }
                                                        let db_repo = cx
                                                            .global::<RepositoryState>()
                                                            .databases
                                                            .clone();
                                                        cx.update_global::<DatabaseState, _>(
                                                            |state, _| {
                                                                if let Some(v) = state
                                                                    .opened_views
                                                                    .iter_mut()
                                                                    .find(|v| {
                                                                        v.unique_key() == view_key
                                                                    })
                                                                {
                                                                    v.view_column_ids =
                                                                        Some(Vec::new());
                                                                }
                                                            },
                                                        );
                                                        let vid = cx
                                                            .read_global::<DatabaseState, _>(
                                                                |state, _| {
                                                                    state
                                                                        .opened_views
                                                                        .iter()
                                                                        .find(|v| {
                                                                            v.unique_key()
                                                                                == view_key
                                                                        })
                                                                        .map(|v| v.view_id)
                                                                        .unwrap_or(0)
                                                                },
                                                            );
                                                        let empty: Vec<i32> = Vec::new();
                                                        cx.spawn(async move |_cx| {
                                                            db_repo
                                                                .set_view_columns(vid, &empty)
                                                                .await?;
                                                            Ok::<_, anyhow::Error>(())
                                                        })
                                                        .detach();
                                                    })
                                            }),
                                    ),
                                );

                                // "id" column row (visible)
                                if show_id && id_matches_search {
                                    let table_view = table_view.clone();
                                    content = content.child(
                                        h_flex()
                                            .id("id-col-visible-row")
                                            .px_2()
                                            .py_0p5()
                                            .gap_2()
                                            .items_center()
                                            .rounded_md()
                                            .hover(|el| el.bg(cx.theme().secondary))
                                            .child(
                                                Icon::default()
                                                    .path("icons/hash.svg")
                                                    .size_4()
                                                    .text_color(muted_color),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_sm()
                                                    .text_color(text_color)
                                                    .child("id"),
                                            )
                                            .child(
                                                div()
                                                    .id("eye-btn-id")
                                                    .cursor_pointer()
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/eye.svg")
                                                            .size_4()
                                                            .text_color(muted_color),
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(tv) = &table_view {
                                                            tv.update(cx, |state, _| {
                                                                state.set_show_id_column(false);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    );
                                }

                                for col in &visible_cols {
                                    let col_id = col.id;
                                    let col_label = col.label.clone();
                                    let icon_path = col.column_type.icon_path();
                                    let drag_label = col.label.clone();
                                    let drag_icon: SharedString = icon_path.into();
                                    let all_col_ids: Vec<i32> =
                                        all_cols.iter().map(|c| c.id).collect();
                                    content = content.child(
                                        h_flex()
                                            .id(("col-drag-row", col_id as usize))
                                            .px_2()
                                            .py_0p5()
                                            .gap_1()
                                            .items_center()
                                            .rounded_md()
                                            .hover(|el| el.bg(cx.theme().secondary))
                                            .on_drag(
                                                DraggableColumn {
                                                    id: col_id,
                                                    label: drag_label.clone(),
                                                    icon_path: drag_icon.clone(),
                                                },
                                                {
                                                    let label = drag_label.clone();
                                                    let icon = drag_icon.clone();
                                                    move |_, _, _, cx| {
                                                        cx.new(|_| ColumnDragGhost {
                                                            label: label.clone(),
                                                            icon_path: icon.clone(),
                                                        })
                                                    }
                                                },
                                            )
                                            .on_drop({
                                                let all_ids = all_col_ids.clone();
                                                move |dragged: &DraggableColumn, _, cx| {
                                                    reorder_column(
                                                        cx, view_key, dragged.id, col_id, &all_ids,
                                                    );
                                                }
                                            })
                                            .child(
                                                Icon::default()
                                                    .path("icons/grip-vertical.svg")
                                                    .size_3()
                                                    .text_color(muted_color.opacity(0.5)),
                                            )
                                            .child(
                                                Icon::default()
                                                    .path(icon_path)
                                                    .size_4()
                                                    .text_color(muted_color),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_sm()
                                                    .text_color(text_color)
                                                    .text_ellipsis()
                                                    .overflow_hidden()
                                                    .child(col_label),
                                            )
                                            .child(
                                                div()
                                                    .id(("eye-btn", col_id as usize))
                                                    .cursor_pointer()
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/eye.svg")
                                                            .size_4()
                                                            .text_color(muted_color),
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        toggle_column_visibility(
                                                            cx, view_key, col_id, false,
                                                        );
                                                    }),
                                            ),
                                    );
                                }
                            }

                            // Separator
                            if has_visible && has_hidden {
                                content = content.child(div().h(px(1.0)).mx_2().bg(border));
                            }

                            // "Masque dans la table" section
                            if has_hidden {
                                content = content.child(
                                    v_flex().py_1().px_2().gap_0p5().child(
                                        h_flex()
                                            .justify_between()
                                            .items_center()
                                            .child(
                                                div()
                                                    .text_xs()
                                                    .text_color(muted_color)
                                                    .child("Masque dans la table"),
                                            )
                                            .child({
                                                let table_view = table_view.clone();
                                                div()
                                                    .id("show-all-btn")
                                                    .cursor_pointer()
                                                    .text_xs()
                                                    .text_color(accent)
                                                    .child("Tout afficher")
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(tv) = &table_view {
                                                            tv.update(cx, |state, _| {
                                                                state.set_show_id_column(true);
                                                            });
                                                        }
                                                        let db_repo = cx
                                                            .global::<RepositoryState>()
                                                            .databases
                                                            .clone();
                                                        cx.update_global::<DatabaseState, _>(
                                                            |state, _| {
                                                                if let Some(v) = state
                                                                    .opened_views
                                                                    .iter_mut()
                                                                    .find(|v| {
                                                                        v.unique_key() == view_key
                                                                    })
                                                                {
                                                                    v.view_column_ids = None;
                                                                }
                                                            },
                                                        );
                                                        let vid = cx
                                                            .read_global::<DatabaseState, _>(
                                                                |state, _| {
                                                                    state
                                                                        .opened_views
                                                                        .iter()
                                                                        .find(|v| {
                                                                            v.unique_key()
                                                                                == view_key
                                                                        })
                                                                        .map(|v| v.view_id)
                                                                        .unwrap_or(0)
                                                                },
                                                            );
                                                        let empty: Vec<i32> = Vec::new();
                                                        cx.spawn(async move |_cx| {
                                                            db_repo
                                                                .set_view_columns(vid, &empty)
                                                                .await?;
                                                            Ok::<_, anyhow::Error>(())
                                                        })
                                                        .detach();
                                                    })
                                            }),
                                    ),
                                );

                                // "id" column row (hidden)
                                if !show_id && id_matches_search {
                                    let table_view = table_view.clone();
                                    content = content.child(
                                        h_flex()
                                            .id("id-col-hidden-row")
                                            .px_2()
                                            .py_0p5()
                                            .gap_2()
                                            .items_center()
                                            .rounded_md()
                                            .hover(|el| el.bg(cx.theme().secondary))
                                            .child(
                                                Icon::default()
                                                    .path("icons/hash.svg")
                                                    .size_4()
                                                    .text_color(muted_color),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_sm()
                                                    .text_color(text_color.opacity(0.5))
                                                    .child("id"),
                                            )
                                            .child(
                                                div()
                                                    .id("eye-off-btn-id")
                                                    .cursor_pointer()
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/eye-off.svg")
                                                            .size_4()
                                                            .text_color(muted_color.opacity(0.5)),
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        if let Some(tv) = &table_view {
                                                            tv.update(cx, |state, _| {
                                                                state.set_show_id_column(true);
                                                            });
                                                        }
                                                    }),
                                            ),
                                    );
                                }

                                for col in &hidden_cols {
                                    let col_id = col.id;
                                    let col_label = col.label.clone();
                                    let icon_path = col.column_type.icon_path();

                                    content = content.child(
                                        h_flex()
                                            .px_2()
                                            .py_0p5()
                                            .gap_2()
                                            .items_center()
                                            .rounded_md()
                                            .hover(|el| el.bg(cx.theme().secondary))
                                            .child(
                                                Icon::default()
                                                    .path(icon_path)
                                                    .size_4()
                                                    .text_color(muted_color),
                                            )
                                            .child(
                                                div()
                                                    .flex_1()
                                                    .text_sm()
                                                    .text_color(text_color.opacity(0.5))
                                                    .text_ellipsis()
                                                    .overflow_hidden()
                                                    .child(col_label),
                                            )
                                            .child(
                                                div()
                                                    .id(("eye-off-btn", col_id as usize))
                                                    .cursor_pointer()
                                                    .child(
                                                        Icon::default()
                                                            .path("icons/eye-off.svg")
                                                            .size_4()
                                                            .text_color(muted_color.opacity(0.5)),
                                                    )
                                                    .on_click(move |_, _, cx| {
                                                        toggle_column_visibility(
                                                            cx, view_key, col_id, true,
                                                        );
                                                    }),
                                            ),
                                    );
                                }
                            }

                            content = content.child(div().h(px(4.0)));
                            content
                        }
                    })
                    .into_any_element()
            } else {
                // Reset search input when popover is closed
                self.columns_search_input = None;

                Button::new("view-settings-btn")
                    .icon(Icon::default().path("icons/settings.svg"))
                    .ghost()
                    .xsmall()
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_view_columns_popover = true;
                        cx.notify();
                    }))
                    .into_any_element()
            }
        } else {
            div().into_any_element()
        };

        // View tabs as pill badges + settings/add on the right
        let accent = cx.theme().accent;
        let tab_text = cx.theme().foreground;
        let tab_border = cx.theme().border;

        let mut tabs_row = h_flex().w_full().items_center().gap_2();

        // Pill-shaped view buttons
        let mut pills = h_flex().gap_1p5();

        for (i, v) in views.iter().enumerate() {
            let name = v.name.clone();
            let is_renaming = self.renaming_tab_index == Some(i);

            if is_renaming {
                // Render inline input for renaming
                if self.tab_rename_input.is_none() {
                    let input = cx.new(|cx| {
                        let mut state = InputState::new(_window, cx);
                        state.set_value(name.clone(), _window, cx);
                        state
                    });

                    let view_id = v.id;
                    cx.subscribe_in(
                        &input,
                        _window,
                        move |this, input_entity, event: &InputEvent, window, cx| match event {
                            InputEvent::PressEnter { .. } => {
                                let new_name = input_entity.read(cx).value().to_string();
                                this.finish_tab_rename(i, view_id, &new_name, cx);
                                window.blur();
                            }
                            InputEvent::Blur => {
                                let new_name = input_entity.read(cx).value().to_string();
                                this.finish_tab_rename(i, view_id, &new_name, cx);
                            }
                            _ => {}
                        },
                    )
                    .detach();

                    input.update(cx, |state, cx| {
                        state.focus(_window, cx);
                    });

                    self.tab_rename_input = Some(input);
                }

                if let Some(ref input) = self.tab_rename_input {
                    let input_width = self.tab_rename_width;

                    pills = pills.child(
                        div()
                            .id(("view-pill", i))
                            .px_3()
                            .py_1()
                            .rounded(px(20.0))
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .border_1()
                            .border_color(accent)
                            .child(
                                div().w(px(input_width)).child(
                                    Input::new(input)
                                        .xsmall()
                                        .appearance(false)
                                        .text_sm()
                                        .cleanable(false)
                                        .w_full(),
                                ),
                            ),
                    );
                }
            } else {
                pills = pills.child(
                    div()
                        .id(("view-pill", i))
                        .cursor_pointer()
                        .px_3()
                        .py_1()
                        .rounded(px(20.0))
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .border_1()
                        .border_color(tab_border)
                        .text_color(tab_text)
                        .hover(|el| el.bg(cx.theme().secondary))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener({
                                let name = name.clone();
                                move |this, event: &MouseDownEvent, _window, cx| {
                                    if event.click_count == 2 {
                                        cx.stop_propagation();
                                        this.renaming_tab_index = Some(i);
                                        this.tab_rename_input = None;
                                        this.tab_rename_width =
                                            (name.len().max(4) as f32) * 8.0 + 16.0;
                                        cx.notify();
                                    }
                                }
                            }),
                        )
                        .child(name)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.selected_tab = i;
                            this.show_view_columns_popover = false;
                            cx.notify();
                        })),
                );
            }
        }

        tabs_row = tabs_row.child(pills);

        // Push settings + add button to the right
        tabs_row = tabs_row.child(
            h_flex()
                .ml_auto()
                .gap_1()
                .items_center()
                .child(settings_el)
                .child(
                    Button::new("add-view-tab")
                        .icon(Icon::default().path("icons/plus.svg"))
                        .ghost()
                        .xsmall()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.add_view(cx);
                        })),
                ),
        );

        container = container.child(tabs_row);

        // Table view for selected tab
        if let Some(view) = selected_view {
            let table_view = self.get_or_create_table_view(view.id, &view.name, cx);

            // Ensure the table view has loaded data in the global DatabaseState
            self.ensure_view_in_global_state(&view, cx);

            container = container.child(
                v_flex()
                    .w_full()
                    .border_1()
                    .border_color(border_color)
                    .rounded_lg()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(400.0))
                            .w_full()
                            .overflow_hidden()
                            .child(table_view),
                    ),
            );
        }

        container.into_any_element()
    }

    fn ensure_view_in_global_state(&self, view: &DatabaseViewModel, cx: &mut Context<Self>) {
        use crate::LoadingState;
        use crate::app::states::database_state::DatabaseState;

        let key = view.id as i64;

        let already_loaded = cx
            .try_global::<DatabaseState>()
            .map(|state| state.opened_views.iter().any(|v| v.unique_key() == key))
            .unwrap_or(false);

        if !already_loaded {
            if let Some(loaded) = &self.loaded_data {
                cx.update_global::<DatabaseState, _>(|state, _| {
                    use crate::app::states::database_state::OpenedDatabaseView;

                    let entry = OpenedDatabaseView {
                        view_id: view.id,
                        view_name: view.name.clone(),
                        database_id: view.database_id,
                        kind: DatabaseTabKind::View,
                        state: LoadingState::Loaded(loaded.clone()),
                        loading_in_progress: false,
                        view_column_ids: None,
                    };

                    if !state.opened_views.iter().any(|v| v.unique_key() == key) {
                        state.opened_views.push(entry);
                    }
                });
            }
        }
    }
}

impl NodeMenuProvider for DatabaseViewNode {
    fn menu_items(&self, _cx: &App) -> Vec<NodeMenuItem> {
        vec![]
    }
}

impl Render for DatabaseViewNode {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = if self.is_configured() {
            self.render_configured(window, cx).into_any_element()
        } else {
            self.render_unconfigured(cx).into_any_element()
        };

        div().w_full().py_2().child(content)
    }
}
