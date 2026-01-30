#[derive(Debug, Clone)]
pub enum ViewType {
    Table,
}

impl ViewType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ViewType::Table => "table",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            _ => ViewType::Table,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseViewModel {
    pub id: i32,
    pub database_id: i32,
    pub name: String,
    pub view_type: ViewType,
    pub folder_id: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct DatabaseViewColumnModel {
    pub id: i32,
    pub view_id: i32,
    pub column_id: i32,
    pub position: i32,
}
