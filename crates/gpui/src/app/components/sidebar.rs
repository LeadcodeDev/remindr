use std::collections::HashSet;
use std::time::Duration;

use gpui::{prelude::FluentBuilder as _, *};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable, WindowExt,
    avatar::Avatar,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt as _, DropdownMenu as _, PopupMenuItem},
    scroll::ScrollableElement,
    sidebar::SidebarHeader,
    v_flex,
};

use crate::{
    LoadingState,
    app::{
        components::{confirm_dialog::ConfirmDialog, settings_dialog::SettingsDialog},
        screens::{database_view_screen::DatabaseViewScreen, document_screen::DocumentScreen},
        states::{
            app_state::AppState, database_state::DatabaseState, document_state::DocumentState,
            repository_state::RepositoryState,
        },
    },
    domain::database::{
        database_model::DatabaseModel as DbModel, database_view::DatabaseViewModel,
        document::DocumentModel, folder::FolderModel,
    },
};

/// Drag data for a document being dragged in the sidebar
#[derive(Clone)]
struct DraggableDocument {
    pub id: i32,
}

/// Ghost view displayed while dragging a document
struct DragGhost {
    title: String,
}

impl Render for DragGhost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let icon_color = cx.theme().sidebar_foreground.opacity(0.6);
        let text_color = cx.theme().sidebar_foreground.opacity(0.9);
        let bg = cx.theme().sidebar_accent;

        h_flex()
            .px_2()
            .py_1()
            .gap_2()
            .items_center()
            .rounded_md()
            .bg(bg)
            .child(
                Icon::default()
                    .path("icons/file-text.svg")
                    .size_4()
                    .text_color(icon_color),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(text_color)
                    .child(self.title.clone()),
            )
    }
}

/// Identifies which item is being renamed inline
#[derive(Clone, Copy, PartialEq)]
enum EditingItem {
    Folder(i32),
    Document(i32),
    DatabaseView(i32),
    Database(i32),
}

/// Represents a tree item in the sidebar
#[derive(Clone)]
enum SidebarItem {
    Folder {
        model: FolderModel,
        children: Vec<SidebarItem>,
    },
    Document(DocumentModel),
    DatabaseView(DatabaseViewModel),
}

pub struct AppSidebar {
    document_state: LoadingState<Vec<DocumentModel>>,
    folder_state: LoadingState<Vec<FolderModel>>,
    database_list_state: LoadingState<Vec<DbModel>>,
    database_views_state: LoadingState<Vec<DatabaseViewModel>>,
    expanded_folders: HashSet<i32>,
    drop_target_folder: Option<i32>,
    editing_item: Option<EditingItem>,
    rename_input: Option<Entity<InputState>>,
    app_state: Entity<AppState>,
}

impl AppSidebar {
    pub fn new(app_state: Entity<AppState>, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| {
            let repository_state = cx.global::<RepositoryState>();
            let document_repository = repository_state.documents.clone();
            let folder_repository = repository_state.folders.clone();
            let database_repository = repository_state.databases.clone();

            // Initial fetch
            cx.spawn({
                let doc_repo = document_repository.clone();
                let folder_repo = folder_repository.clone();
                let db_repo = database_repository.clone();
                async move |this, cx| {
                    let documents = doc_repo.get_documents().await;
                    let folders = folder_repo.get_folders().await;
                    let databases = db_repo.get_databases().await;
                    let views = db_repo.get_views().await;
                    if let (Ok(documents), Ok(folders)) = (documents, folders) {
                        let _ = this.update(cx, |state: &mut Self, _| {
                            state.document_state = LoadingState::Loaded(documents);
                            state.folder_state = LoadingState::Loaded(folders);
                            if let Ok(databases) = databases {
                                state.database_list_state = LoadingState::Loaded(databases);
                            }
                            if let Ok(views) = views {
                                state.database_views_state = LoadingState::Loaded(views);
                            }
                        });
                    }
                }
            })
            .detach();

            // Poll every 5 seconds
            cx.spawn({
                let doc_repo = document_repository.clone();
                let folder_repo = folder_repository.clone();
                let db_repo = database_repository.clone();
                async move |this, cx| {
                    loop {
                        smol::Timer::after(Duration::from_secs(5)).await;
                        let documents = doc_repo.get_documents().await;
                        let folders = folder_repo.get_folders().await;
                        let databases = db_repo.get_databases().await;
                        let views = db_repo.get_views().await;
                        if let (Ok(documents), Ok(folders)) = (documents, folders) {
                            let result = this.update(cx, |state: &mut Self, _| {
                                state.document_state = LoadingState::Loaded(documents);
                                state.folder_state = LoadingState::Loaded(folders);
                                if let Ok(databases) = databases {
                                    state.database_list_state = LoadingState::Loaded(databases);
                                }
                                if let Ok(views) = views {
                                    state.database_views_state = LoadingState::Loaded(views);
                                }
                            });
                            if result.is_err() {
                                break;
                            }
                        }
                    }
                }
            })
            .detach();

            Self {
                document_state: LoadingState::Loading,
                folder_state: LoadingState::Loading,
                database_list_state: LoadingState::Loading,
                database_views_state: LoadingState::Loading,
                expanded_folders: HashSet::new(),
                drop_target_folder: None,
                editing_item: None,
                rename_input: None,
                app_state,
            }
        })
    }

    fn get_username() -> String {
        std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .unwrap_or_else(|_| "User".to_string())
    }

    fn render_user_dropdown(&self, cx: &Context<Self>) -> impl IntoElement {
        let username = Self::get_username();
        let sidebar_fg = cx.theme().sidebar_foreground;

        SidebarHeader::new()
            .p_1()
            .child(Avatar::new().name(username.clone()).small())
            .child(
                div()
                    .flex_1()
                    .text_sm()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(username),
            )
            .child(
                h_flex()
                    .gap_0p5()
                    .child(
                        Icon::default()
                            .path("icons/square-pen.svg")
                            .size_4()
                            .text_color(sidebar_fg.opacity(0.6)),
                    )
                    .child(
                        Icon::new(IconName::ChevronDown)
                            .size_4()
                            .text_color(sidebar_fg.opacity(0.6)),
                    ),
            )
            .dropdown_menu(|menu, _, _| {
                menu.min_w(px(220.)).item(
                    PopupMenuItem::new("Settings")
                        .icon(Icon::new(IconName::Settings))
                        .on_click(|_, window, cx| {
                            SettingsDialog::open(window, cx);
                        }),
                )
            })
    }

    fn start_rename(
        &mut self,
        item: EditingItem,
        current_name: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_value(current_name.to_string(), window, cx);
            state
        });

        // Subscribe to input events
        cx.subscribe_in(&input, window, {
            move |this, _input, event: &InputEvent, _window, cx| match event {
                InputEvent::PressEnter { .. } => {
                    this.commit_rename(cx);
                }
                InputEvent::Blur => {
                    this.cancel_rename(cx);
                }
                _ => {}
            }
        })
        .detach();

        // Focus the input
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });

        self.editing_item = Some(item);
        self.rename_input = Some(input);
    }

    fn commit_rename(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.editing_item.take() else {
            return;
        };
        let Some(input) = self.rename_input.take() else {
            return;
        };

        let new_name = input.read(cx).value().to_string();
        if new_name.is_empty() {
            return;
        }

        let doc_repo = cx.global::<RepositoryState>().documents.clone();
        let folder_repo = cx.global::<RepositoryState>().folders.clone();
        let db_repo = cx.global::<RepositoryState>().databases.clone();
        let this = cx.entity().clone();

        match editing {
            EditingItem::Folder(id) => {
                cx.spawn(async move |_this, cx| {
                    let folder = folder_repo.get_folder_by_id(id).await?;
                    let updated = FolderModel {
                        name: new_name,
                        ..folder
                    };
                    folder_repo.update_folder(updated).await?;
                    let _ = cx.update(|cx| {
                        AppSidebar::refresh_data(&this, cx);
                    });
                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
            EditingItem::Document(id) => {
                cx.spawn(async move |_this, cx| {
                    let doc = doc_repo.get_document_by_id(id).await?;
                    let updated = DocumentModel {
                        title: new_name,
                        ..doc
                    };
                    doc_repo.update_document(updated).await?;
                    let _ = cx.update(|cx| {
                        AppSidebar::refresh_data(&this, cx);
                    });
                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
            EditingItem::DatabaseView(id) => {
                cx.spawn(async move |_this, cx| {
                    let view = db_repo.get_view_by_id(id).await?;
                    let updated = DatabaseViewModel {
                        name: new_name,
                        ..view
                    };
                    db_repo.update_view(updated).await?;
                    let _ = cx.update(|cx| {
                        AppSidebar::refresh_data(&this, cx);
                    });
                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
            EditingItem::Database(id) => {
                cx.spawn(async move |_this, cx| {
                    let db = db_repo.get_database_by_id(id).await?;
                    let updated = DbModel {
                        name: new_name,
                        ..db
                    };
                    db_repo.update_database(updated).await?;
                    let _ = cx.update(|cx| {
                        AppSidebar::refresh_data(&this, cx);
                    });
                    Ok::<_, anyhow::Error>(())
                })
                .detach();
            }
        }
    }

    fn cancel_rename(&mut self, _cx: &mut Context<Self>) {
        self.editing_item = None;
        self.rename_input = None;
    }

    /// Build a tree structure from flat lists of folders, documents, and database views
    fn build_tree(
        folders: &[FolderModel],
        documents: &[DocumentModel],
        views: &[DatabaseViewModel],
    ) -> Vec<SidebarItem> {
        fn build_children(
            parent_id: Option<i32>,
            folders: &[FolderModel],
            documents: &[DocumentModel],
            views: &[DatabaseViewModel],
        ) -> Vec<SidebarItem> {
            let mut items = Vec::new();

            // Add child folders first
            for folder in folders.iter().filter(|f| f.parent_id == parent_id) {
                let children = build_children(Some(folder.id), folders, documents, views);
                items.push(SidebarItem::Folder {
                    model: folder.clone(),
                    children,
                });
            }

            // Then add documents
            for doc in documents.iter().filter(|d| d.folder_id == parent_id) {
                items.push(SidebarItem::Document(doc.clone()));
            }

            // Then add database views
            for view in views.iter().filter(|v| v.folder_id == parent_id) {
                items.push(SidebarItem::DatabaseView(view.clone()));
            }

            items
        }

        build_children(None, folders, documents, views)
    }

    fn refresh_data(this: &Entity<Self>, cx: &mut App) {
        let doc_repo = cx.global::<RepositoryState>().documents.clone();
        let folder_repo = cx.global::<RepositoryState>().folders.clone();
        let db_repo = cx.global::<RepositoryState>().databases.clone();
        let this = this.clone();

        cx.spawn(async move |cx| {
            let documents = doc_repo.get_documents().await?;
            let folders = folder_repo.get_folders().await?;
            let databases = db_repo.get_databases().await;
            let views = db_repo.get_views().await;

            let _ = this.update(cx, |state, _| {
                state.document_state = LoadingState::Loaded(documents);
                state.folder_state = LoadingState::Loaded(folders);
                if let Ok(databases) = databases {
                    state.database_list_state = LoadingState::Loaded(databases);
                }
                if let Ok(views) = views {
                    state.database_views_state = LoadingState::Loaded(views);
                }
            });

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    }
}

impl Render for AppSidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sidebar_bg = cx.theme().sidebar;
        let border_color = cx.theme().border;
        let header_text_color = cx.theme().sidebar_foreground.opacity(0.5);
        let item_text_color = cx.theme().sidebar_foreground.opacity(0.9);
        let icon_color = cx.theme().sidebar_foreground.opacity(0.6);
        let accent_bg = cx.theme().sidebar_accent;
        let radius = cx.theme().radius;

        let this = cx.entity().clone();
        let app_state = self.app_state.clone();

        let documents = match &self.document_state {
            LoadingState::Loaded(docs) => docs.clone(),
            _ => vec![],
        };

        let folders = match &self.folder_state {
            LoadingState::Loaded(folders) => folders.clone(),
            _ => vec![],
        };

        let views = match &self.database_views_state {
            LoadingState::Loaded(views) => views.clone(),
            _ => vec![],
        };

        let databases = match &self.database_list_state {
            LoadingState::Loaded(dbs) => dbs.clone(),
            _ => vec![],
        };

        let tree = Self::build_tree(&folders, &documents, &views);
        let expanded_folders = self.expanded_folders.clone();
        let drop_target_folder = self.drop_target_folder;
        let editing_item = self.editing_item;
        let rename_input = self.rename_input.clone();

        // Header
        let header = h_flex()
            .flex_shrink_0()
            .px_2()
            .rounded(radius)
            .text_xs()
            .text_color(header_text_color)
            .h_8()
            .justify_between()
            .items_center()
            .child("Documents")
            .child(
                h_flex()
                    .gap_1()
                    .child(
                        Button::new("refresh-documents")
                            .icon(Icon::default().path("icons/refresh-cw.svg"))
                            .ghost()
                            .xsmall()
                            .cursor_pointer()
                            .tooltip("Refresh documents")
                            .on_click({
                                let this = this.clone();
                                move |_, _, cx| {
                                    Self::refresh_data(&this, cx);
                                }
                            }),
                    )
                    .child(
                        Button::new("create-folder")
                            .icon(Icon::new(IconName::Folder))
                            .ghost()
                            .xsmall()
                            .cursor_pointer()
                            .tooltip("New folder")
                            .on_click({
                                let this = this.clone();
                                move |_, _, cx| {
                                    let folder_repo =
                                        cx.global::<RepositoryState>().folders.clone();
                                    let this_clone = this.clone();

                                    cx.spawn(async move |cx| {
                                        folder_repo
                                            .insert_folder("Untitled".to_string(), None)
                                            .await?;

                                        let _ = cx.update(|cx| {
                                            Self::refresh_data(&this_clone, cx);
                                        });

                                        Ok::<_, anyhow::Error>(())
                                    })
                                    .detach();
                                }
                            }),
                    )
                    .child(
                        Button::new("create-document")
                            .icon(Icon::new(IconName::Plus))
                            .ghost()
                            .xsmall()
                            .cursor_pointer()
                            .tooltip("New document")
                            .on_click({
                                let this = this.clone();
                                let app_state = app_state.clone();
                                move |_, _, cx| {
                                    let repository =
                                        cx.global::<RepositoryState>().documents.clone();
                                    let this_clone = this.clone();
                                    let app_state = app_state.clone();

                                    cx.spawn(async move |cx| {
                                        let new_document = DocumentModel {
                                            id: 0,
                                            title: "Untitled".to_string(),
                                            content: serde_json::json!([]),
                                            folder_id: None,
                                        };

                                        let new_id =
                                            repository.insert_document(new_document).await?;

                                        let _ = cx.update(|cx: &mut App| {
                                            Self::refresh_data(&this_clone, cx);

                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                state.open_document(new_id, "Untitled".to_string());
                                            });

                                            app_state.update(cx, |app_state, cx| {
                                                let document_screen =
                                                    DocumentScreen::new(cx.weak_entity());
                                                app_state.navigator.push(document_screen, cx);
                                            });
                                        });

                                        Ok::<_, anyhow::Error>(())
                                    })
                                    .detach();
                                }
                            }),
                    ),
            );

        // Build tree items recursively
        let items = render_tree_items(
            tree,
            0,
            &expanded_folders,
            drop_target_folder,
            editing_item,
            &rename_input,
            &this,
            &app_state,
            &folders,
            &databases,
            item_text_color,
            icon_color,
            accent_bg,
        );

        // Root drop zone: drop a document here to move it to root
        let root_drop_zone = {
            let this = this.clone();
            let this_ctx = this.clone();
            let this_cancel = this.clone();
            let app_state_ctx = app_state.clone();
            let databases_for_ctx = databases.clone();
            div()
                .id("root-drop-zone")
                .w_full()
                .min_h_8()
                .flex_1()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                    this_cancel.update(cx, |state, cx| {
                        if state.editing_item.is_some() {
                            state.cancel_rename(cx);
                        }
                    });
                })
                .on_drop(move |dragged: &DraggableDocument, _, cx| {
                    let doc_id = dragged.id;
                    let doc_repo = cx.global::<RepositoryState>().documents.clone();
                    let this_clone = this.clone();

                    this.update(cx, |state, _| {
                        state.drop_target_folder = None;
                    });

                    cx.spawn(async move |cx| {
                        doc_repo.move_document(doc_id, None).await?;
                        let _ = cx.update(|cx| {
                            AppSidebar::refresh_data(&this_clone, cx);
                        });
                        Ok::<_, anyhow::Error>(())
                    })
                    .detach();
                })
                .context_menu({
                    move |mut menu, _window, _cx| {
                        menu = menu
                            .item(
                                PopupMenuItem::new("New document")
                                    .icon(Icon::default().path("icons/file-text.svg"))
                                    .on_click({
                                        let this = this_ctx.clone();
                                        let app_state = app_state_ctx.clone();
                                        move |_, _, cx| {
                                            let repository =
                                                cx.global::<RepositoryState>().documents.clone();
                                            let this_clone = this.clone();
                                            let app_state = app_state.clone();

                                            cx.spawn(async move |cx| {
                                                let new_document = DocumentModel {
                                                    id: 0,
                                                    title: "Untitled".to_string(),
                                                    content: serde_json::json!([]),
                                                    folder_id: None,
                                                };
                                                let new_id = repository
                                                    .insert_document(new_document)
                                                    .await?;
                                                let _ = cx.update(|cx: &mut App| {
                                                    AppSidebar::refresh_data(&this_clone, cx);
                                                    cx.update_global::<DocumentState, _>(
                                                        |state, _| {
                                                            state.open_document(
                                                                new_id,
                                                                "Untitled".to_string(),
                                                            );
                                                        },
                                                    );
                                                    app_state.update(cx, |app_state, cx| {
                                                        let document_screen =
                                                            DocumentScreen::new(cx.weak_entity());
                                                        app_state
                                                            .navigator
                                                            .push(document_screen, cx);
                                                    });
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("New folder")
                                    .icon(Icon::new(IconName::Folder))
                                    .on_click({
                                        let this = this_ctx.clone();
                                        move |_, _, cx| {
                                            let folder_repo =
                                                cx.global::<RepositoryState>().folders.clone();
                                            let this_clone = this.clone();

                                            cx.spawn(async move |cx| {
                                                folder_repo
                                                    .insert_folder("Untitled".to_string(), None)
                                                    .await?;
                                                let _ = cx.update(|cx| {
                                                    AppSidebar::refresh_data(&this_clone, cx);
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        }
                                    }),
                            );

                        // Add "New database view" submenu if databases exist
                        if !databases_for_ctx.is_empty() {
                            menu = menu.separator();
                            for db in &databases_for_ctx {
                                let db_id = db.id;
                                let db_name = db.name.clone();
                                let this = this_ctx.clone();
                                let app_state = app_state_ctx.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(format!("New view: {}", db_name))
                                        .icon(Icon::default().path("icons/table.svg"))
                                        .on_click(move |_, _, cx| {
                                            let db_repo =
                                                cx.global::<RepositoryState>().databases.clone();
                                            let this_clone = this.clone();
                                            let app_state = app_state.clone();

                                            cx.spawn(async move |cx| {
                                                let view_id = db_repo
                                                    .insert_view(
                                                        db_id,
                                                        "Table View".to_string(),
                                                        "table",
                                                        None,
                                                    )
                                                    .await?;

                                                let _ = cx.update(|cx: &mut App| {
                                                    AppSidebar::refresh_data(&this_clone, cx);
                                                    cx.update_global::<DatabaseState, _>(
                                                        |state, _| {
                                                            state.open_view(
                                                                view_id,
                                                                "Table View".to_string(),
                                                                db_id,
                                                            );
                                                        },
                                                    );
                                                    app_state.update(cx, |app_state, cx| {
                                                        let screen = DatabaseViewScreen::new(
                                                            cx.weak_entity(),
                                                        );
                                                        app_state.navigator.push(screen, cx);
                                                    });
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        }),
                                );
                            }
                        }

                        menu
                    }
                })
        };

        // Databases section at the bottom
        let databases_header = h_flex()
            .flex_shrink_0()
            .px_2()
            .rounded(radius)
            .text_xs()
            .text_color(header_text_color)
            .h_8()
            .justify_between()
            .items_center()
            .child("Databases")
            .child(
                h_flex().gap_1().child(
                    Button::new("create-database")
                        .icon(Icon::new(IconName::Plus))
                        .ghost()
                        .xsmall()
                        .cursor_pointer()
                        .tooltip("New database")
                        .on_click({
                            let this = this.clone();
                            move |_, _, cx| {
                                let db_repo = cx.global::<RepositoryState>().databases.clone();
                                let this_clone = this.clone();

                                cx.spawn(async move |cx| {
                                    db_repo.insert_database("Untitled".to_string()).await?;
                                    let _ = cx.update(|cx| {
                                        AppSidebar::refresh_data(&this_clone, cx);
                                    });
                                    Ok::<_, anyhow::Error>(())
                                })
                                .detach();
                            }
                        }),
                ),
            );

        let database_items: Vec<_> = databases
            .iter()
            .map(|db| {
                let db_id = db.id;
                let db_name = db.name.clone();
                let delete_name = db.name.clone();
                let this_item = this.clone();
                let app_state_db = app_state.clone();

                h_flex()
                    .id(("database-item", db_id as usize))
                    .w_full()
                    .h_7()
                    .px_2()
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(accent_bg))
                    .on_click({
                        let app_state = app_state_db.clone();
                        let db_name = db_name.clone();
                        move |_, _, cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                state.open_source(db_id, db_name.clone());
                            });

                            app_state.update(cx, |app_state, cx| {
                                let screen = DatabaseViewScreen::new(cx.weak_entity());
                                app_state.navigator.push(screen, cx);
                            });
                        }
                    })
                    .child(
                        Icon::default()
                            .path("icons/database.svg")
                            .size_4()
                            .text_color(icon_color),
                    )
                    .child({
                        let is_editing = editing_item == Some(EditingItem::Database(db_id));
                        if is_editing {
                            let this_esc = this_item.clone();
                            div()
                                .flex_1()
                                .mx_neg_1()
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key.as_str() == "escape" {
                                        cx.stop_propagation();
                                        this_esc.update(cx, |state, cx| {
                                            state.cancel_rename(cx);
                                        });
                                    }
                                })
                                .child(
                                    Input::new(rename_input.as_ref().unwrap())
                                        .xsmall()
                                        .appearance(false)
                                        .text_sm(),
                                )
                        } else {
                            let this_dbl = this_item.clone();
                            let name_for_dbl = db_name.clone();
                            div()
                                .flex_1()
                                .text_sm()
                                .text_ellipsis()
                                .overflow_hidden()
                                .text_color(item_text_color)
                                .child(db_name.clone())
                                .on_mouse_down(
                                    MouseButton::Left,
                                    move |event, window, cx| {
                                        if event.click_count == 2 {
                                            cx.stop_propagation();
                                            this_dbl.update(cx, |state, cx| {
                                                state.start_rename(
                                                    EditingItem::Database(db_id),
                                                    &name_for_dbl,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    },
                                )
                        }
                    })
                    .child(
                        div()
                            .opacity(0.0)
                            .hover(|el| el.opacity(1.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                Button::new(("delete-db", db_id as usize))
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .danger()
                                    .xsmall()
                                    .cursor_pointer()
                                    .on_click({
                                        let this_del = this_item.clone();
                                        move |_, window, cx| {
                                            let this_clone = this_del.clone();
                                            let delete_name = delete_name.clone();

                                            ConfirmDialog::new("Delete Database")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"? All views and data will be lost.",
                                                    delete_name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let db_repo = cx.global::<RepositoryState>().databases.clone();
                                                    let this_spawn = this_clone.clone();
                                                    window.push_notification(
                                                        format!("\"{}\" has been deleted", delete_name),
                                                        cx,
                                                    );
                                                    cx.spawn(async move |cx| {
                                                        let _ = db_repo.delete_database(db_id).await;
                                                        let _ = cx.update(|cx| {
                                                            AppSidebar::refresh_data(&this_spawn, cx);
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            ),
                    )
                    .context_menu({
                        let this_ctx = this_item.clone();
                        let db_name_ctx = db.name.clone();
                        move |menu, _window, _cx| {
                            menu.item(
                                PopupMenuItem::new("Rename")
                                    .icon(Icon::default().path("icons/pencil-line.svg"))
                                    .on_click({
                                        let this = this_ctx.clone();
                                        let name = db_name_ctx.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |state, cx| {
                                                state.start_rename(
                                                    EditingItem::Database(db_id),
                                                    &name,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Delete")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click({
                                        let this = this_ctx.clone();
                                        let name = db_name_ctx.clone();
                                        move |_, window, cx| {
                                            let this_clone = this.clone();
                                            let delete_name = name.clone();

                                            ConfirmDialog::new("Delete Database")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"? All views and data will be lost.",
                                                    delete_name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let db_repo = cx.global::<RepositoryState>().databases.clone();
                                                    let this_spawn = this_clone.clone();
                                                    window.push_notification(
                                                        format!("\"{}\" has been deleted", delete_name),
                                                        cx,
                                                    );
                                                    cx.spawn(async move |cx| {
                                                        let _ = db_repo.delete_database(db_id).await;
                                                        let _ = cx.update(|cx| {
                                                            AppSidebar::refresh_data(&this_spawn, cx);
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            )
                        }
                    })
            })
            .collect();

        v_flex()
            .h_full()
            .w(px(240.0))
            .bg(sidebar_bg)
            .border_r_1()
            .border_color(border_color)
            .child(div().px_2().py_2().child(self.render_user_dropdown(cx)))
            .child(header)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .px_1()
                    .overflow_y_scrollbar()
                    .flex_1()
                    .children(items)
                    .child(root_drop_zone),
            )
            .child(
                div()
                    .border_t_1()
                    .border_color(border_color)
                    .child(databases_header)
                    .child(
                        div()
                            .px_1()
                            .max_h(px(200.0))
                            .overflow_y_scrollbar()
                            .children(database_items),
                    ),
            )
    }
}

#[allow(clippy::too_many_arguments)]
fn render_tree_items(
    items: Vec<SidebarItem>,
    depth: u32,
    expanded_folders: &HashSet<i32>,
    drop_target_folder: Option<i32>,
    editing_item: Option<EditingItem>,
    rename_input: &Option<Entity<InputState>>,
    this: &Entity<AppSidebar>,
    app_state: &Entity<AppState>,
    all_folders: &[FolderModel],
    all_databases: &[DbModel],
    item_text_color: Hsla,
    icon_color: Hsla,
    accent_bg: Hsla,
) -> Vec<AnyElement> {
    let mut elements = Vec::new();

    for item in items {
        match item {
            SidebarItem::Folder { model, children } => {
                let folder_id = model.id;
                let folder_name = model.name.clone();
                let is_expanded = expanded_folders.contains(&folder_id);

                // Folder row
                let chevron_icon = if is_expanded {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronRight
                };

                let is_drop_target = drop_target_folder == Some(folder_id);

                let folder_row = h_flex()
                    .id(("folder-item", folder_id as usize))
                    .w_full()
                    .h_7()
                    .px_2()
                    .pl(px(8.0 + (depth as f32) * 16.0))
                    .gap_1()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(accent_bg))
                    .when(is_drop_target, |el| {
                        el.bg(accent_bg).border_1().border_color(accent_bg)
                    })
                    .on_click({
                        let this = this.clone();
                        move |_, _, cx| {
                            this.update(cx, |state, _| {
                                if state.expanded_folders.contains(&folder_id) {
                                    state.expanded_folders.remove(&folder_id);
                                } else {
                                    state.expanded_folders.insert(folder_id);
                                }
                            });
                        }
                    })
                    .on_drop({
                        let this = this.clone();
                        move |dragged: &DraggableDocument, _, cx| {
                            let doc_id = dragged.id;
                            let doc_repo = cx.global::<RepositoryState>().documents.clone();
                            let this_clone = this.clone();

                            this.update(cx, |state, _| {
                                state.drop_target_folder = None;
                                // Auto-expand the folder on drop
                                state.expanded_folders.insert(folder_id);
                            });

                            cx.spawn(async move |cx| {
                                doc_repo.move_document(doc_id, Some(folder_id)).await?;
                                let _ = cx.update(|cx| {
                                    AppSidebar::refresh_data(&this_clone, cx);
                                });
                                Ok::<_, anyhow::Error>(())
                            })
                            .detach();
                        }
                    })
                    .on_drag_move({
                        let this = this.clone();
                        move |_: &DragMoveEvent<DraggableDocument>, _, cx| {
                            this.update(cx, |state, _| {
                                state.drop_target_folder = Some(folder_id);
                            });
                        }
                    })
                    .child(
                        Icon::new(chevron_icon).size_3().text_color(icon_color),
                    )
                    .child(
                        Icon::new(if is_expanded {
                            IconName::FolderOpen
                        } else {
                            IconName::Folder
                        })
                        .size_4()
                        .text_color(icon_color),
                    )
                    .child({
                        let is_editing = editing_item == Some(EditingItem::Folder(folder_id));
                        if is_editing {
                            let this_esc = this.clone();
                            div().flex_1().mx_neg_1()
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key.as_str() == "escape" {
                                        cx.stop_propagation();
                                        this_esc.update(cx, |state, cx| {
                                            state.cancel_rename(cx);
                                        });
                                    }
                                })
                                .child(
                                    Input::new(rename_input.as_ref().unwrap())
                                        .xsmall()
                                        .appearance(false)
                                        .text_sm(),
                                )
                        } else {
                            let this = this.clone();
                            let name = folder_name.clone();
                            div()
                                .flex_1()
                                .text_sm()
                                .text_ellipsis()
                                .overflow_hidden()
                                .text_color(item_text_color)
                                .child(folder_name.clone())
                                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                    if event.click_count == 2 {
                                        cx.stop_propagation();
                                        this.update(cx, |state, cx| {
                                            state.start_rename(
                                                EditingItem::Folder(folder_id),
                                                &name,
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                        }
                    })
                    .child(
                        div()
                            .opacity(0.0)
                            .hover(|el| el.opacity(1.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                h_flex()
                                    .gap_0p5()
                                    .child({
                                        let this = this.clone();
                                        let app_state = app_state.clone();
                                        Button::new(("new-doc-in-folder", folder_id as usize))
                                            .icon(Icon::new(IconName::Plus))
                                            .ghost()
                                            .xsmall()
                                            .cursor_pointer()
                                            .on_click(move |_, _, cx| {
                                                let repository =
                                                    cx.global::<RepositoryState>().documents.clone();
                                                let this_clone = this.clone();
                                                let app_state = app_state.clone();

                                                cx.spawn(async move |cx| {
                                                    let new_document = DocumentModel {
                                                        id: 0,
                                                        title: "Untitled".to_string(),
                                                        content: serde_json::json!([]),
                                                        folder_id: Some(folder_id),
                                                    };

                                                    let new_id = repository
                                                        .insert_document(new_document)
                                                        .await?;

                                                    let _ = cx.update(|cx: &mut App| {
                                                        AppSidebar::refresh_data(&this_clone, cx);

                                                        cx.update_global::<DocumentState, _>(
                                                            |state, _| {
                                                                state.open_document_in_folder(
                                                                    new_id,
                                                                    "Untitled".to_string(),
                                                                    Some(folder_id),
                                                                );
                                                            },
                                                        );

                                                        app_state.update(cx, |app_state, cx| {
                                                            let document_screen =
                                                                DocumentScreen::new(
                                                                    cx.weak_entity(),
                                                                );
                                                            app_state
                                                                .navigator
                                                                .push(document_screen, cx);
                                                        });
                                                    });

                                                    Ok::<_, anyhow::Error>(())
                                                })
                                                .detach();
                                            })
                                    })
                                    .child({
                                        let this = this.clone();
                                        Button::new(("delete-folder", folder_id as usize))
                                            .icon(Icon::default().path("icons/trash-2.svg"))
                                            .danger()
                                            .xsmall()
                                            .cursor_pointer()
                                            .on_click(move |_, window, cx| {
                                                let this_clone = this.clone();
                                                let delete_name = folder_name.clone();

                                                ConfirmDialog::new("Delete Folder")
                                                    .message(format!(
                                                        "Are you sure you want to delete \"{}\" and all its contents? This action cannot be undone.",
                                                        delete_name
                                                    ))
                                                    .confirm_text("Delete")
                                                    .cancel_text("Cancel")
                                                    .danger()
                                                    .on_confirm(move |window, cx| {
                                                        let folder_repo = cx.global::<RepositoryState>().folders.clone();
                                                        let this_for_spawn = this_clone.clone();

                                                        window.push_notification(
                                                            format!("\"{}\" has been deleted", delete_name),
                                                            cx,
                                                        );

                                                        cx.spawn(async move |cx| {
                                                            let _ = folder_repo.delete_folder(folder_id).await;

                                                            let _ = cx.update(|cx| {
                                                                AppSidebar::refresh_data(&this_for_spawn, cx);
                                                            });

                                                            Ok::<_, anyhow::Error>(())
                                                        })
                                                        .detach();

                                                        true
                                                    })
                                                    .open(window, cx);
                                            })
                                    }),
                            ),
                    )
                    .context_menu({
                        let this = this.clone();
                        let app_state = app_state.clone();
                        let folder_name = model.name.clone();
                        let _folder_parent_id = model.parent_id;
                        let databases_for_folder = all_databases.to_vec();
                        move |menu, _window, _cx| {
                            let mut menu = menu.item(
                                PopupMenuItem::new("New document")
                                    .icon(Icon::default().path("icons/file-text.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let app_state = app_state.clone();
                                        move |_, _, cx| {
                                            let repository = cx.global::<RepositoryState>().documents.clone();
                                            let this_clone = this.clone();
                                            let app_state = app_state.clone();

                                            cx.spawn(async move |cx| {
                                                let new_document = DocumentModel {
                                                    id: 0,
                                                    title: "Untitled".to_string(),
                                                    content: serde_json::json!([]),
                                                    folder_id: Some(folder_id),
                                                };
                                                let new_id = repository.insert_document(new_document).await?;
                                                let _ = cx.update(|cx: &mut App| {
                                                    AppSidebar::refresh_data(&this_clone, cx);
                                                    cx.update_global::<DocumentState, _>(|state, _| {
                                                        state.open_document_in_folder(new_id, "Untitled".to_string(), Some(folder_id));
                                                    });
                                                    app_state.update(cx, |app_state, cx| {
                                                        let document_screen = DocumentScreen::new(cx.weak_entity());
                                                        app_state.navigator.push(document_screen, cx);
                                                    });
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("New folder")
                                    .icon(Icon::new(IconName::Folder))
                                    .on_click({
                                        let this = this.clone();
                                        move |_, _, cx| {
                                            let folder_repo = cx.global::<RepositoryState>().folders.clone();
                                            let this_clone = this.clone();

                                            cx.spawn(async move |cx| {
                                                folder_repo.insert_folder("Untitled".to_string(), Some(folder_id)).await?;
                                                let _ = cx.update(|cx| {
                                                    AppSidebar::refresh_data(&this_clone, cx);
                                                });
                                                Ok::<_, anyhow::Error>(())
                                            })
                                            .detach();
                                        }
                                    }),
                            );

                            // Add "New database view" submenu for databases
                            if !databases_for_folder.is_empty() {
                                menu = menu.separator();
                                for db in &databases_for_folder {
                                    let db_id = db.id;
                                    let db_name = db.name.clone();
                                    let this = this.clone();
                                    let app_state = app_state.clone();
                                    menu = menu.item(
                                        PopupMenuItem::new(format!("New view: {}", db_name))
                                            .icon(Icon::default().path("icons/table.svg"))
                                            .on_click(move |_, _, cx| {
                                                let db_repo = cx.global::<RepositoryState>().databases.clone();
                                                let this_clone = this.clone();
                                                let app_state = app_state.clone();

                                                cx.spawn(async move |cx| {
                                                    let view_id = db_repo.insert_view(
                                                        db_id,
                                                        "Table View".to_string(),
                                                        "table",
                                                        Some(folder_id),
                                                    ).await?;

                                                    let _ = cx.update(|cx: &mut App| {
                                                        AppSidebar::refresh_data(&this_clone, cx);
                                                        cx.update_global::<DatabaseState, _>(|state, _| {
                                                            state.open_view(view_id, "Table View".to_string(), db_id);
                                                        });
                                                        app_state.update(cx, |app_state, cx| {
                                                            let screen = DatabaseViewScreen::new(cx.weak_entity());
                                                            app_state.navigator.push(screen, cx);
                                                        });
                                                    });
                                                    Ok::<_, anyhow::Error>(())
                                                }).detach();
                                            }),
                                    );
                                }
                            }

                            menu
                            .separator()
                            .item(
                                PopupMenuItem::new("Rename")
                                    .icon(Icon::default().path("icons/pencil-line.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let name = folder_name.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |state, cx| {
                                                state.start_rename(EditingItem::Folder(folder_id), &name, window, cx);
                                            });
                                        }
                                    })
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Bulk delete")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let folder_name = folder_name.clone();
                                        move |_, window, cx| {
                                            let this_clone = this.clone();
                                            let name = folder_name.clone();

                                            ConfirmDialog::new("Delete Folder")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\" and all its contents? This action cannot be undone.",
                                                    name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let folder_repo = cx.global::<RepositoryState>().folders.clone();
                                                    let this_spawn = this_clone.clone();
                                                    let name = name.clone();
                                                    window.push_notification(format!("\"{}\" has been deleted", name), cx);
                                                    cx.spawn(async move |cx| {
                                                        let _ = folder_repo.delete_folder(folder_id).await;
                                                        let _ = cx.update(|cx| { AppSidebar::refresh_data(&this_spawn, cx); });
                                                        Ok::<_, anyhow::Error>(())
                                                    }).detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            )
                            .item(
                                PopupMenuItem::new("Delete")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let folder_name = folder_name.clone();
                                        move |_, window, cx| {
                                            let this_clone = this.clone();
                                            let name = folder_name.clone();

                                            ConfirmDialog::new("Delete Folder")
                                                .message(format!(
                                                    "Delete \"{}\" but keep its documents and sub-folders?",
                                                    name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let folder_repo = cx.global::<RepositoryState>().folders.clone();
                                                    let this_spawn = this_clone.clone();
                                                    let name = name.clone();
                                                    window.push_notification(format!("\"{}\" has been deleted", name), cx);
                                                    cx.spawn(async move |cx| {
                                                        let _ = folder_repo.delete_folder_keep_children(folder_id).await;
                                                        let _ = cx.update(|cx| { AppSidebar::refresh_data(&this_spawn, cx); });
                                                        Ok::<_, anyhow::Error>(())
                                                    }).detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            )
                        }
                    });

                elements.push(folder_row.into_any_element());

                // Render children if expanded
                if is_expanded {
                    let child_elements = render_tree_items(
                        children,
                        depth + 1,
                        expanded_folders,
                        drop_target_folder,
                        editing_item,
                        rename_input,
                        this,
                        app_state,
                        all_folders,
                        all_databases,
                        item_text_color,
                        icon_color,
                        accent_bg,
                    );
                    elements.extend(child_elements);
                }
            }
            SidebarItem::Document(document) => {
                let document_id = document.id;
                let document_title = document.title.clone();
                let delete_title = document.title.clone();
                let document_folder_id = document.folder_id;
                let this_clone = this.clone();
                let app_state_clone = app_state.clone();

                let drag_title = document_title.clone();

                let doc_row = h_flex()
                    .id(("document-item", document_id as usize))
                    .w_full()
                    .h_7()
                    .px_2()
                    .pl(px(8.0 + (depth as f32) * 16.0))
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(accent_bg))
                    .on_drag(
                        DraggableDocument { id: document_id },
                        move |_, _, _, cx| {
                            cx.new(|_| DragGhost {
                                title: drag_title.clone(),
                            })
                        },
                    )
                    .on_click({
                        let document_title = document_title.clone();
                        let app_state = app_state_clone.clone();
                        move |_, _, cx| {
                            cx.update_global::<DocumentState, _>(|state, _| {
                                state.open_document_in_folder(
                                    document_id,
                                    document_title.clone(),
                                    document_folder_id,
                                );
                            });

                            app_state.update(cx, |app_state, cx| {
                                let document_screen = DocumentScreen::new(cx.weak_entity());
                                app_state.navigator.push(document_screen, cx);
                            });
                        }
                    })
                    .child(
                        Icon::default()
                            .path("icons/file-text.svg")
                            .size_4()
                            .text_color(icon_color),
                    )
                    .child({
                        let is_editing = editing_item == Some(EditingItem::Document(document_id));
                        if is_editing {
                            let this_esc = this.clone();
                            div().flex_1().mx_neg_1()
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key.as_str() == "escape" {
                                        cx.stop_propagation();
                                        this_esc.update(cx, |state, cx| {
                                            state.cancel_rename(cx);
                                        });
                                    }
                                })
                                .child(
                                    Input::new(rename_input.as_ref().unwrap())
                                        .xsmall()
                                        .appearance(false)
                                        .text_sm(),
                                )
                        } else {
                            let this = this.clone();
                            let name = document_title.clone();
                            div()
                                .flex_1()
                                .text_sm()
                                .text_ellipsis()
                                .overflow_hidden()
                                .text_color(item_text_color)
                                .child(document.title.clone())
                                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                    if event.click_count == 2 {
                                        cx.stop_propagation();
                                        this.update(cx, |state, cx| {
                                            state.start_rename(
                                                EditingItem::Document(document_id),
                                                &name,
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                        }
                    })
                    .child(
                        div()
                            .opacity(0.0)
                            .hover(|el| el.opacity(1.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                h_flex()
                                    .gap_0p5()
                                    .child({
                                        let this = this.clone();
                                        let all_folders_for_menu: Vec<FolderModel> =
                                            all_folders.to_vec();
                                        Button::new(("move-doc", document_id as usize))
                                            .icon(Icon::new(IconName::Folder))
                                            .ghost()
                                            .xsmall()
                                            .cursor_pointer()
                                            .dropdown_menu(move |menu, _, _| {
                                                let mut menu = menu.min_w(px(180.));

                                                // "Move to root" option if document is in a folder
                                                if document_folder_id.is_some() {
                                                    let this_root = this.clone();
                                                    menu = menu.item(
                                                        PopupMenuItem::new("Root")
                                                            .on_click(move |_, _, cx| {
                                                                let doc_repo = cx
                                                                    .global::<RepositoryState>()
                                                                    .documents
                                                                    .clone();
                                                                let this_move = this_root.clone();

                                                                cx.spawn(async move |cx| {
                                                                    doc_repo
                                                                        .move_document(
                                                                            document_id,
                                                                            None,
                                                                        )
                                                                        .await?;
                                                                    let _ = cx.update(|cx| {
                                                                        AppSidebar::refresh_data(
                                                                            &this_move,
                                                                            cx,
                                                                        );
                                                                    });
                                                                    Ok::<_, anyhow::Error>(())
                                                                })
                                                                .detach();
                                                            }),
                                                    );
                                                    menu = menu.separator();
                                                }

                                                // Add folder options
                                                for folder in &all_folders_for_menu {
                                                    if Some(folder.id) == document_folder_id {
                                                        continue; // Skip current folder
                                                    }
                                                    let folder_id = folder.id;
                                                    let folder_name = folder.name.clone();
                                                    let this_folder = this.clone();
                                                    menu = menu.item(
                                                        PopupMenuItem::new(folder_name)
                                                            .icon(Icon::new(IconName::Folder))
                                                            .on_click(move |_, _, cx| {
                                                                let doc_repo = cx
                                                                    .global::<RepositoryState>()
                                                                    .documents
                                                                    .clone();
                                                                let this_move =
                                                                    this_folder.clone();

                                                                cx.spawn(async move |cx| {
                                                                    doc_repo
                                                                        .move_document(
                                                                            document_id,
                                                                            Some(folder_id),
                                                                        )
                                                                        .await?;
                                                                    let _ = cx.update(|cx| {
                                                                        AppSidebar::refresh_data(
                                                                            &this_move,
                                                                            cx,
                                                                        );
                                                                    });
                                                                    Ok::<_, anyhow::Error>(())
                                                                })
                                                                .detach();
                                                            }),
                                                    );
                                                }

                                                menu
                                            })
                                    })
                                    .child(
                                        Button::new(("delete-doc", document_id as usize))
                                            .icon(Icon::default().path("icons/trash-2.svg"))
                                            .danger()
                                            .xsmall()
                                            .cursor_pointer()
                                            .on_click({
                                                let this_clone = this_clone.clone();
                                                move |_, window, cx| {
                                                    let this_clone = this_clone.clone();
                                                    let delete_title = delete_title.clone();

                                                    ConfirmDialog::new("Delete Page")
                                                        .message(format!(
                                                            "Are you sure you want to delete \"{}\"? This action cannot be undone.",
                                                            delete_title
                                                        ))
                                                        .confirm_text("Delete")
                                                        .cancel_text("Cancel")
                                                        .danger()
                                                        .on_confirm(move |window, cx| {
                                                            let repository = cx.global::<RepositoryState>().documents.clone();
                                                            let this_for_spawn = this_clone.clone();
                                                            let deleted_title = delete_title.clone();

                                                            cx.update_global::<DocumentState, _>(|state, _| {
                                                                state.remove_document(document_id);
                                                                if state.current_opened_document == Some(document_id) {
                                                                    state.current_opened_document = None;
                                                                }
                                                            });

                                                            window.push_notification(
                                                                format!("\"{}\" has been deleted", deleted_title),
                                                                cx,
                                                            );

                                                            cx.spawn(async move |cx| {
                                                                let _ = repository.delete_document(document_id).await;

                                                                let _ = cx.update(|cx| {
                                                                    AppSidebar::refresh_data(&this_for_spawn, cx);
                                                                });

                                                                Ok::<_, anyhow::Error>(())
                                                            })
                                                            .detach();

                                                            true
                                                        })
                                                        .open(window, cx);
                                                }
                                            }),
                                    ),
                            ),
                    )
                    .context_menu({
                        let this = this.clone();
                        let doc_title = document_title.clone();
                        let delete_title2 = document.title.clone();
                        let this_clone2 = this_clone.clone();
                        move |menu, _window, _cx| {
                            menu.item(
                                PopupMenuItem::new("Rename")
                                  .icon(Icon::default().path("icons/pencil-line.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let name = doc_title.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |state, cx| {
                                                state.start_rename(EditingItem::Document(document_id), &name, window, cx);
                                            });
                                        }
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Delete")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click({
                                        let this_clone = this_clone2.clone();
                                        let delete_title = delete_title2.clone();
                                        move |_, window, cx| {
                                            let this_clone = this_clone.clone();
                                            let delete_title = delete_title.clone();

                                            ConfirmDialog::new("Delete Page")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"? This action cannot be undone.",
                                                    delete_title
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let repository = cx.global::<RepositoryState>().documents.clone();
                                                    let this_for_spawn = this_clone.clone();
                                                    let deleted_title = delete_title.clone();

                                                    cx.update_global::<DocumentState, _>(|state, _| {
                                                        state.remove_document(document_id);
                                                        if state.current_opened_document == Some(document_id) {
                                                            state.current_opened_document = None;
                                                        }
                                                    });

                                                    window.push_notification(
                                                        format!("\"{}\" has been deleted", deleted_title),
                                                        cx,
                                                    );

                                                    cx.spawn(async move |cx| {
                                                        let _ = repository.delete_document(document_id).await;
                                                        let _ = cx.update(|cx| {
                                                            AppSidebar::refresh_data(&this_for_spawn, cx);
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    }).detach();

                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            )
                        }
                    });

                elements.push(doc_row.into_any_element());
            }
            SidebarItem::DatabaseView(view) => {
                let view_id = view.id;
                let view_name = view.name.clone();
                let view_db_id = view.database_id;
                let delete_name = view.name.clone();
                let this_clone = this.clone();
                let app_state_clone = app_state.clone();

                let view_row = h_flex()
                    .id(("db-view-item", view_id as usize))
                    .w_full()
                    .h_7()
                    .px_2()
                    .pl(px(8.0 + (depth as f32) * 16.0))
                    .gap_2()
                    .items_center()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|el| el.bg(accent_bg))
                    .on_click({
                        let view_name = view_name.clone();
                        let app_state = app_state_clone.clone();
                        move |_, _, cx| {
                            cx.update_global::<DatabaseState, _>(|state, _| {
                                state.open_view(view_id, view_name.clone(), view_db_id);
                            });

                            app_state.update(cx, |app_state, cx| {
                                let screen = DatabaseViewScreen::new(cx.weak_entity());
                                app_state.navigator.push(screen, cx);
                            });
                        }
                    })
                    .child(
                        Icon::default()
                            .path("icons/table.svg")
                            .size_4()
                            .text_color(icon_color),
                    )
                    .child({
                        let is_editing = editing_item == Some(EditingItem::DatabaseView(view_id));
                        if is_editing {
                            let this_esc = this.clone();
                            div()
                                .flex_1()
                                .mx_neg_1()
                                .on_key_down(move |event, _, cx| {
                                    if event.keystroke.key.as_str() == "escape" {
                                        cx.stop_propagation();
                                        this_esc.update(cx, |state, cx| {
                                            state.cancel_rename(cx);
                                        });
                                    }
                                })
                                .child(
                                    Input::new(rename_input.as_ref().unwrap())
                                        .xsmall()
                                        .appearance(false)
                                        .text_sm(),
                                )
                        } else {
                            let this_dbl = this.clone();
                            let name_for_dbl = view_name.clone();
                            div()
                                .flex_1()
                                .text_sm()
                                .text_ellipsis()
                                .overflow_hidden()
                                .text_color(item_text_color)
                                .child(view.name.clone())
                                .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                                    if event.click_count == 2 {
                                        cx.stop_propagation();
                                        this_dbl.update(cx, |state, cx| {
                                            state.start_rename(
                                                EditingItem::DatabaseView(view_id),
                                                &name_for_dbl,
                                                window,
                                                cx,
                                            );
                                        });
                                    }
                                })
                        }
                    })
                    .child(
                        div()
                            .opacity(0.0)
                            .hover(|el| el.opacity(1.0))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                cx.stop_propagation();
                            })
                            .child(
                                Button::new(("delete-view", view_id as usize))
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .danger()
                                    .xsmall()
                                    .cursor_pointer()
                                    .on_click({
                                        let this = this_clone.clone();
                                        move |_, window, cx| {
                                            let this_clone = this.clone();
                                            let delete_name = delete_name.clone();

                                            ConfirmDialog::new("Delete View")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"?",
                                                    delete_name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let db_repo = cx
                                                        .global::<RepositoryState>()
                                                        .databases
                                                        .clone();
                                                    let this_spawn = this_clone.clone();
                                                    window.push_notification(
                                                        format!(
                                                            "\"{}\" has been deleted",
                                                            delete_name
                                                        ),
                                                        cx,
                                                    );
                                                    cx.spawn(async move |cx| {
                                                        let _ = db_repo.delete_view(view_id).await;
                                                        let _ = cx.update(|cx| {
                                                            AppSidebar::refresh_data(
                                                                &this_spawn,
                                                                cx,
                                                            );
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            ),
                    )
                    .context_menu({
                        let this = this.clone();
                        let view_name_ctx = view_name.clone();
                        let delete_name2 = view.name.clone();
                        let this_clone2 = this_clone.clone();
                        move |menu, _window, _cx| {
                            menu.item(
                                PopupMenuItem::new("Rename")
                                    .icon(Icon::default().path("icons/pencil-line.svg"))
                                    .on_click({
                                        let this = this.clone();
                                        let name = view_name_ctx.clone();
                                        move |_, window, cx| {
                                            this.update(cx, |state, cx| {
                                                state.start_rename(
                                                    EditingItem::DatabaseView(view_id),
                                                    &name,
                                                    window,
                                                    cx,
                                                );
                                            });
                                        }
                                    }),
                            )
                            .separator()
                            .item(
                                PopupMenuItem::new("Delete")
                                    .icon(Icon::default().path("icons/trash-2.svg"))
                                    .on_click({
                                        let this = this_clone2.clone();
                                        let name = delete_name2.clone();
                                        move |_, window, cx| {
                                            let this_clone = this.clone();
                                            let delete_name = name.clone();

                                            ConfirmDialog::new("Delete View")
                                                .message(format!(
                                                    "Are you sure you want to delete \"{}\"?",
                                                    delete_name
                                                ))
                                                .confirm_text("Delete")
                                                .cancel_text("Cancel")
                                                .danger()
                                                .on_confirm(move |window, cx| {
                                                    let db_repo = cx
                                                        .global::<RepositoryState>()
                                                        .databases
                                                        .clone();
                                                    let this_spawn = this_clone.clone();
                                                    window.push_notification(
                                                        format!(
                                                            "\"{}\" has been deleted",
                                                            delete_name
                                                        ),
                                                        cx,
                                                    );
                                                    cx.spawn(async move |cx| {
                                                        let _ = db_repo.delete_view(view_id).await;
                                                        let _ = cx.update(|cx| {
                                                            AppSidebar::refresh_data(
                                                                &this_spawn,
                                                                cx,
                                                            );
                                                        });
                                                        Ok::<_, anyhow::Error>(())
                                                    })
                                                    .detach();
                                                    true
                                                })
                                                .open(window, cx);
                                        }
                                    }),
                            )
                        }
                    });

                elements.push(view_row.into_any_element());
            }
        }
    }

    elements
}
