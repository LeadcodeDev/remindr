CREATE TABLE block (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL,
    kind_data TEXT,
    parent_table TEXT NOT NULL CHECK (parent_table IN ('root', 'block', 'database')),
    parent_id TEXT,
    sort_key TEXT NOT NULL,
    text TEXT NOT NULL,
    alive INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE INDEX block_parent ON block (parent_table, parent_id, sort_key);
CREATE TABLE database (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    icon TEXT,
    sort_key TEXT NOT NULL,
    alive INTEGER NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE property (
    id TEXT PRIMARY KEY,
    database_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    kind_data TEXT
);
CREATE INDEX property_database ON property (database_id, position);
CREATE TABLE property_value (
    row_id TEXT NOT NULL,
    property_id TEXT NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (row_id, property_id)
);
CREATE INDEX property_value_property ON property_value (property_id);
CREATE TABLE view (
    id TEXT PRIMARY KEY,
    block_id TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL,
    sort_key TEXT NOT NULL,
    config TEXT NOT NULL
);
CREATE INDEX view_block ON view (block_id);
