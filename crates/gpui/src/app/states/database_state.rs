use std::collections::HashMap;

use gpui::Global;

use crate::{
    LoadingState,
    domain::database::{
        database_column::DatabaseColumnModel, database_model::DatabaseModel,
        database_row::DatabaseRowModel,
    },
};

/// Whether this opened tab is a database source or a named view
#[derive(Clone, PartialEq)]
pub enum DatabaseTabKind {
    /// Direct database source (opened from the Databases section)
    Source,
    /// A named view (from database_views table)
    View,
}

#[derive(Clone)]
pub struct OpenedDatabaseView {
    pub view_id: i32,
    pub view_name: String,
    pub database_id: i32,
    pub kind: DatabaseTabKind,
    pub state: LoadingState<LoadedDatabaseView>,
    pub loading_in_progress: bool,
    /// Column IDs visible in this view. `None` = show all (Source tabs or no overrides).
    pub view_column_ids: Option<Vec<i32>>,
}

impl OpenedDatabaseView {
    /// Unique key combining kind + id to distinguish source tabs from view tabs
    pub fn unique_key(&self) -> i64 {
        match self.kind {
            DatabaseTabKind::Source => -(self.database_id as i64) - 1,
            DatabaseTabKind::View => self.view_id as i64,
        }
    }
}

#[derive(Clone)]
pub struct LoadedDatabaseView {
    pub database: DatabaseModel,
    pub columns: Vec<DatabaseColumnModel>,
    pub rows: Vec<DatabaseRowModel>,
    pub cells: HashMap<(i32, i32), String>,
}

pub struct DatabaseState {
    pub opened_views: Vec<OpenedDatabaseView>,
    /// Unique key of the currently active tab
    pub current_opened_view: Option<i64>,
}

impl DatabaseState {
    pub fn get_current_view(&self) -> Option<&OpenedDatabaseView> {
        self.current_opened_view
            .and_then(|key| self.opened_views.iter().find(|v| v.unique_key() == key))
    }

    pub fn get_current_view_index(&self) -> Option<usize> {
        self.current_opened_view
            .and_then(|key| self.opened_views.iter().position(|v| v.unique_key() == key))
    }

    pub fn get_previous_view(&self, key: i64) -> Option<&OpenedDatabaseView> {
        let current_index = self.opened_views.iter().position(|v| v.unique_key() == key);
        current_index.and_then(|index| {
            if index > 0 {
                Some(&self.opened_views[index - 1])
            } else if self.opened_views.len() > 1 {
                Some(&self.opened_views[1])
            } else {
                None
            }
        })
    }

    /// Open a named view (from database_views table)
    pub fn open_view(&mut self, view_id: i32, view_name: String, database_id: i32) {
        let entry = OpenedDatabaseView {
            view_id,
            view_name: view_name.clone(),
            database_id,
            kind: DatabaseTabKind::View,
            state: LoadingState::Loading,
            loading_in_progress: false,
            view_column_ids: None,
        };
        let key = entry.unique_key();
        let already_exists = self.opened_views.iter().any(|v| v.unique_key() == key);
        if !already_exists {
            self.opened_views.push(entry);
        }
        self.current_opened_view = Some(key);
    }

    /// Open a database source directly (from the Databases section)
    pub fn open_source(&mut self, database_id: i32, database_name: String) {
        let entry = OpenedDatabaseView {
            view_id: 0, // no view row
            view_name: database_name,
            database_id,
            kind: DatabaseTabKind::Source,
            state: LoadingState::Loading,
            loading_in_progress: false,
            view_column_ids: None,
        };
        let key = entry.unique_key();
        let already_exists = self.opened_views.iter().any(|v| v.unique_key() == key);
        if !already_exists {
            self.opened_views.push(entry);
        }
        self.current_opened_view = Some(key);
    }

    pub fn needs_loading(&self, key: i64) -> bool {
        self.opened_views
            .iter()
            .find(|v| v.unique_key() == key)
            .map(|v| matches!(v.state, LoadingState::Loading) && !v.loading_in_progress)
            .unwrap_or(false)
    }

    pub fn set_loading_in_progress(&mut self, key: i64, in_progress: bool) {
        if let Some(v) = self.opened_views.iter_mut().find(|v| v.unique_key() == key) {
            v.loading_in_progress = in_progress;
        }
    }

    pub fn set_view_loaded(&mut self, key: i64, data: LoadedDatabaseView) {
        if let Some(v) = self.opened_views.iter_mut().find(|v| v.unique_key() == key) {
            v.state = LoadingState::Loaded(data);
            v.loading_in_progress = false;
        }
    }

    /// Update all opened tabs that share the same database_id
    pub fn set_all_views_loaded_for_database(
        &mut self,
        database_id: i32,
        data: LoadedDatabaseView,
    ) {
        for v in self
            .opened_views
            .iter_mut()
            .filter(|v| v.database_id == database_id)
        {
            v.state = LoadingState::Loaded(data.clone());
            v.loading_in_progress = false;
        }
    }

    pub fn set_view_error(&mut self, key: i64, error: String) {
        if let Some(v) = self.opened_views.iter_mut().find(|v| v.unique_key() == key) {
            v.state = LoadingState::Error(error);
            v.loading_in_progress = false;
        }
    }

    pub fn remove_view(&mut self, key: i64) {
        self.opened_views.retain(|v| v.unique_key() != key);
        if self.current_opened_view == Some(key) {
            self.current_opened_view = None;
        }
    }
}

impl Default for DatabaseState {
    fn default() -> Self {
        Self {
            opened_views: Vec::new(),
            current_opened_view: None,
        }
    }
}

impl Global for DatabaseState {}
