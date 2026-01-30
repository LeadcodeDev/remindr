use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ColumnType {
    Int,
    String,
    Bool,
    Date,
}

impl ColumnType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ColumnType::Int => "int",
            ColumnType::String => "string",
            ColumnType::Bool => "bool",
            ColumnType::Date => "date",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "int" => ColumnType::Int,
            "bool" => ColumnType::Bool,
            "date" => ColumnType::Date,
            _ => ColumnType::String,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            ColumnType::Int => "Number",
            ColumnType::String => "Text",
            ColumnType::Bool => "Checkbox",
            ColumnType::Date => "Date",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            ColumnType::Int => "icons/hash.svg",
            ColumnType::String => "icons/type.svg",
            ColumnType::Bool => "icons/check-square.svg",
            ColumnType::Date => "icons/calendar.svg",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnConfig {
    #[serde(default)]
    pub default_value: Option<String>,
}

impl Default for ColumnConfig {
    fn default() -> Self {
        Self {
            default_value: None,
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
