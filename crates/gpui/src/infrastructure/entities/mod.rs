use serde_json::Value;
use sqlx::prelude::FromRow;

use crate::domain::database::database_cell::DatabaseCellModel;
use crate::domain::database::database_column::{ColumnConfig, ColumnType, DatabaseColumnModel};
use crate::domain::database::database_model::DatabaseModel;
use crate::domain::database::database_row::DatabaseRowModel;
use crate::domain::database::database_view::{
    DatabaseViewColumnModel, DatabaseViewModel, ViewType,
};
use crate::domain::database::document::DocumentModel;
use crate::domain::database::folder::FolderModel;

#[derive(Debug, FromRow)]
pub struct DocumentEntity {
    pub id: i32,
    pub title: String,
    pub content: Value,
    pub folder_id: Option<i32>,
}

impl From<DocumentEntity> for DocumentModel {
    fn from(entity: DocumentEntity) -> Self {
        DocumentModel {
            id: entity.id,
            title: entity.title,
            content: entity.content,
            folder_id: entity.folder_id,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct FolderEntity {
    pub id: i32,
    pub name: String,
    pub parent_id: Option<i32>,
}

impl From<FolderEntity> for FolderModel {
    fn from(entity: FolderEntity) -> Self {
        FolderModel {
            id: entity.id,
            name: entity.name,
            parent_id: entity.parent_id,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseEntity {
    pub id: i32,
    pub name: String,
}

impl From<DatabaseEntity> for DatabaseModel {
    fn from(entity: DatabaseEntity) -> Self {
        DatabaseModel {
            id: entity.id,
            name: entity.name,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseColumnEntity {
    pub id: i32,
    pub database_id: i32,
    pub label: String,
    pub column_type: String,
    pub config: String,
    pub position: i32,
}

impl From<DatabaseColumnEntity> for DatabaseColumnModel {
    fn from(entity: DatabaseColumnEntity) -> Self {
        let column_type = ColumnType::from_str(&entity.column_type);
        let config: ColumnConfig = serde_json::from_str(&entity.config).unwrap_or_default();
        DatabaseColumnModel {
            id: entity.id,
            database_id: entity.database_id,
            label: entity.label,
            column_type,
            config,
            position: entity.position,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseRowEntity {
    pub id: i32,
    pub database_id: i32,
    pub uuid: String,
}

impl From<DatabaseRowEntity> for DatabaseRowModel {
    fn from(entity: DatabaseRowEntity) -> Self {
        DatabaseRowModel {
            id: entity.id,
            database_id: entity.database_id,
            uuid: entity.uuid,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseCellEntity {
    pub id: i32,
    pub row_id: i32,
    pub column_id: i32,
    pub value: String,
}

impl From<DatabaseCellEntity> for DatabaseCellModel {
    fn from(entity: DatabaseCellEntity) -> Self {
        DatabaseCellModel {
            id: entity.id,
            row_id: entity.row_id,
            column_id: entity.column_id,
            value: entity.value,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseViewEntity {
    pub id: i32,
    pub database_id: i32,
    pub name: String,
    pub view_type: String,
    pub folder_id: Option<i32>,
}

impl From<DatabaseViewEntity> for DatabaseViewModel {
    fn from(entity: DatabaseViewEntity) -> Self {
        DatabaseViewModel {
            id: entity.id,
            database_id: entity.database_id,
            name: entity.name,
            view_type: ViewType::from_str(&entity.view_type),
            folder_id: entity.folder_id,
        }
    }
}

#[derive(Debug, FromRow)]
pub struct DatabaseViewColumnEntity {
    pub id: i32,
    pub view_id: i32,
    pub column_id: i32,
    pub position: i32,
}

impl From<DatabaseViewColumnEntity> for DatabaseViewColumnModel {
    fn from(entity: DatabaseViewColumnEntity) -> Self {
        DatabaseViewColumnModel {
            id: entity.id,
            view_id: entity.view_id,
            column_id: entity.column_id,
            position: entity.position,
        }
    }
}
