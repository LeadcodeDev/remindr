CREATE TABLE IF NOT EXISTS database_view_columns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    view_id INTEGER NOT NULL REFERENCES database_views(id) ON DELETE CASCADE,
    column_id INTEGER NOT NULL REFERENCES database_columns(id) ON DELETE CASCADE,
    position INTEGER NOT NULL DEFAULT 0,
    UNIQUE(view_id, column_id)
);
