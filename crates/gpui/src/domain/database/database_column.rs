use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Int,
    String,
    Bool,
    Select,
}

impl ColumnType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColumnType::Int => "int",
            ColumnType::String => "string",
            ColumnType::Bool => "bool",
            ColumnType::Select => "select",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "int" => ColumnType::Int,
            "bool" => ColumnType::Bool,
            "select" => ColumnType::Select,
            _ => ColumnType::String,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ColumnType::Int => "Number",
            ColumnType::String => "Text",
            ColumnType::Bool => "Checkbox",
            ColumnType::Select => "Select",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            ColumnType::Int => "icons/hash.svg",
            ColumnType::String => "icons/type.svg",
            ColumnType::Bool => "icons/check-square.svg",
            ColumnType::Select => "icons/list.svg",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SelectMode {
    Single,
    Multiple,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnConfig {
    #[serde(default)]
    pub mode: Option<SelectMode>,
    #[serde(default)]
    pub options: Vec<String>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            mode: None,
            options: Vec::new(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DatabaseColumnModel {
    pub id: i32,
    pub database_id: i32,
    pub label: String,
    pub column_type: ColumnType,
    pub config: ColumnConfig,
    pub position: i32,
}
