use gpui::{prelude::FluentBuilder, *};
use gpui_component::{
    ActiveTheme, Colorize, Disableable, Icon, Sizable,
    button::{Button, ButtonVariants},
    tab::{Tab, TabBar},
};
use gpui_nav::{Screen, ScreenContext};
use std::collections::HashMap;

use crate::{
    LoadingState,
    app::{
        components::table_view::TableView,
        states::{
            app_state::AppState,
            database_state::{
                DatabaseState, DatabaseTabKind, LoadedDatabaseView, OpenedDatabaseView,
            },
            repository_state::RepositoryState,
        },
    },
};

pub struct DatabaseViewScreen {
    _ctx: ScreenContext<AppState>,
    initialized: bool,
    table_views: HashMap<i64, Entity<TableView>>,
}

impl Screen for DatabaseViewScreen {
    fn id(&self) -> &'static str {
        "DatabaseViews"
    }
}

impl DatabaseViewScreen {
    pub fn new(app_state: WeakEntity<AppState>) -> Self {
        Self {
            _ctx: ScreenContext::new(app_state),
            initialized: false,
            table_views: HashMap::new(),
        }
    }

    fn ensure_initialized(&mut self, cx: &mut Context<Self>) {
        if !self.initialized {
            self.initialized = true;
            cx.observe_global::<DatabaseState>(|_, cx| {
                cx.notify();
            })
            .detach();
        }
    }

    fn load_view_if_needed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let (needs_loading, key, database_id, view_id, kind) =
            cx.read_global::<DatabaseState, _>(|state, _| {
                let id = state.current_opened_view;
                if let Some(key) = id {
                    let needs = state.needs_loading(key);
                    let view_info = state
                        .opened_views
                        .iter()
                        .find(|v| v.unique_key() == key)
                        .map(|v| (v.database_id, v.view_id, v.kind.clone()));
                    if let Some((db_id, view_id, kind)) = view_info {
                        (needs, Some(key), Some(db_id), Some(view_id), Some(kind))
                    } else {
                        (false, None, None, None, None)
                    }
                } else {
                    (false, None, None, None, None)
                }
            });

        if needs_loading {
            if let (Some(key), Some(database_id), Some(view_id), Some(kind)) =
                (key, database_id, view_id, kind)
            {
                cx.update_global::<DatabaseState, _>(|state, _| {
                    state.set_loading_in_progress(key, true);
                });

                let db_repo = cx.global::<RepositoryState>().databases.clone();

                cx.spawn(async move |_, cx| {
                    let database = db_repo.get_database_by_id(database_id).await;
                    let columns = db_repo.get_columns(database_id).await;
                    let rows = db_repo.get_rows(database_id).await;

                    let result = match (database, columns, rows) {
                        (Ok(database), Ok(columns), Ok(rows)) => {
                            let row_ids: Vec<i32> = rows.iter().map(|r| r.id).collect();
                            let cells_result = db_repo.get_cells(&row_ids).await;

                            let cells_map = match cells_result {
                                Ok(cells) => {
                                    let mut map = HashMap::new();
                                    for cell in cells {
                                        map.insert((cell.row_id, cell.column_id), cell.value);
                                    }
                                    map
                                }
                                Err(_) => HashMap::new(),
                            };

                            // Fetch per-view column overrides for View tabs
                            let view_column_ids = if kind == DatabaseTabKind::View {
                                match db_repo.get_view_columns(view_id).await {
                                    Ok(vc) if !vc.is_empty() => {
                                        Some(vc.iter().map(|v| v.column_id).collect::<Vec<_>>())
                                    }
                                    _ => None,
                                }
                            } else {
                                None
                            };

                            let loaded = LoadedDatabaseView {
                                database,
                                columns,
                                rows,
                                cells: cells_map,
                            };

                            cx.update(|cx| {
                                cx.update_global::<DatabaseState, _>(|state, _| {
                                    state.set_view_loaded(key, loaded);
                                    if let Some(v) = state
                                        .opened_views
                                        .iter_mut()
                                        .find(|v| v.unique_key() == key)
                                    {
                                        v.view_column_ids = view_column_ids;
                                    }
                                });
                            })
                        }
                        _ => cx.update(|cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                state.set_view_error(
                                    key,
                                    "Failed to load database view".to_string(),
                                );
                            });
                        }),
                    };

                    // If cx.update() failed (context lost), reset loading_in_progress
                    // so a retry can happen on the next render cycle.
                    if result.is_err() {
                        let _ = cx.update(|cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                state.set_loading_in_progress(key, false);
                            });
                        });
                    }

                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
        }
    }

    fn get_or_create_table_view(
        &mut self,
        key: i64,
        database_id: i32,
        view_id: i32,
        kind: DatabaseTabKind,
        cx: &mut Context<Self>,
    ) -> Entity<TableView> {
        if let Some(entity) = self.table_views.get(&key) {
            return entity.clone();
        }

        let entity = cx.new(|_| TableView::new(key, database_id, view_id, kind));
        self.table_views.insert(key, entity.clone());
        entity
    }
}

impl Render for DatabaseViewScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_initialized(cx);
        self.load_view_if_needed(window, cx);

        let (views, current_view, current_index, can_go_previous, can_go_next) = cx
            .read_global::<DatabaseState, _>(|state, _| {
                let views: Vec<OpenedDatabaseView> = state.opened_views.clone();
                let current_view = state.get_current_view().cloned();
                let current_index = state.get_current_view_index();
                let can_go_previous = current_index.map(|i| i > 0).unwrap_or(false);
                let can_go_next = current_index
                    .map(|i| i < views.len().saturating_sub(1))
                    .unwrap_or(false);
                (
                    views,
                    current_view,
                    current_index,
                    can_go_previous,
                    can_go_next,
                )
            });

        // Clean up table_views for closed views
        let open_ids: Vec<i64> = views.iter().map(|v| v.unique_key()).collect();
        self.table_views.retain(|id, _| open_ids.contains(id));

        div()
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .relative()
            .when(!views.is_empty(), |this| {
                this.child(
                    TabBar::new("db-view-tabs")
                        .prefix(
                            div()
                                .px_1()
                                .flex()
                                .items_center()
                                .child(
                                    Button::new("db-nav-previous")
                                        .xsmall()
                                        .ghost()
                                        .when(can_go_previous, |this| this.cursor_pointer())
                                        .icon(Icon::default().path("icons/chevron-left.svg"))
                                        .disabled(!can_go_previous)
                                        .tooltip("Previous tab")
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.update_global::<DatabaseState, _>(|state, _| {
                                                if let Some(index) = state.get_current_view_index()
                                                {
                                                    if index > 0 {
                                                        if let Some(v) =
                                                            state.opened_views.get(index - 1)
                                                        {
                                                            state.current_opened_view =
                                                                Some(v.unique_key());
                                                        }
                                                    }
                                                }
                                            });
                                        })),
                                )
                                .child(
                                    Button::new("db-nav-next")
                                        .xsmall()
                                        .ghost()
                                        .when(can_go_next, |this| this.cursor_pointer())
                                        .icon(Icon::default().path("icons/chevron-right.svg"))
                                        .disabled(!can_go_next)
                                        .tooltip("Next tab")
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.update_global::<DatabaseState, _>(|state, _| {
                                                if let Some(index) = state.get_current_view_index()
                                                {
                                                    if index < state.opened_views.len() - 1 {
                                                        if let Some(v) =
                                                            state.opened_views.get(index + 1)
                                                        {
                                                            state.current_opened_view =
                                                                Some(v.unique_key());
                                                        }
                                                    }
                                                }
                                            });
                                        })),
                                ),
                        )
                        .selected_index(current_index.unwrap_or(0))
                        .on_click(cx.listener(|_, index: &usize, _, cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                if let Some(v) = state.opened_views.get(*index) {
                                    state.current_opened_view = Some(v.unique_key());
                                }
                            });
                        }))
                        .children(views.iter().map(|v| {
                            let key = v.unique_key();
                            Tab::new()
                                .bg(cx.theme().background.lighten(0.2))
                                .cursor_pointer()
                                .label(v.view_name.clone())
                                .suffix(
                                    Button::new("btn")
                                        .xsmall()
                                        .mr_2()
                                        .cursor_pointer()
                                        .icon(Icon::default().path("icons/x.svg"))
                                        .ghost()
                                        .tooltip("Close tab")
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            cx.update_global::<DatabaseState, _>(|state, _| {
                                                let prev = state.get_previous_view(key);
                                                state.current_opened_view =
                                                    prev.map(|v| v.unique_key());
                                                state.remove_view(key);
                                            });
                                        })),
                                )
                        })),
                )
                .child(self.render_view_content(current_view, cx))
            })
            .when(views.is_empty(), |this| {
                this.child(
                    div()
                        .bg(cx.theme().background.lighten(0.2))
                        .flex()
                        .w_full()
                        .h_full()
                        .items_center()
                        .justify_center()
                        .child("No database view selected"),
                )
            })
    }
}

impl DatabaseViewScreen {
    fn render_view_content(
        &mut self,
        current_view: Option<OpenedDatabaseView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match current_view {
            Some(view) => match &view.state {
                LoadingState::Loading => div()
                    .bg(cx.theme().background.lighten(0.2))
                    .flex()
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .child("Loading...")
                    .into_any_element(),
                LoadingState::Loaded(_) => {
                    let table_view = self.get_or_create_table_view(
                        view.unique_key(),
                        view.database_id,
                        view.view_id,
                        view.kind.clone(),
                        cx,
                    );
                    div()
                        .bg(cx.theme().background.lighten(0.2))
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .w_full()
                        .overflow_hidden()
                        .child(table_view)
                        .into_any_element()
                }
                LoadingState::Error(error) => div()
                    .bg(cx.theme().background.lighten(0.2))
                    .flex()
                    .w_full()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .child(error.clone())
                    .into_any_element(),
            },
            None => div()
                .bg(cx.theme().background.lighten(0.2))
                .flex()
                .w_full()
                .h_full()
                .items_center()
                .justify_center()
                .child("No database view selected")
                .into_any_element(),
        }
    }
}
