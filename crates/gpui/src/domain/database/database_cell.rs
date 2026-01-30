#[derive(Debug, Clone)]
pub struct DatabaseCellModel {
    pub id: i32,
    pub row_id: i32,
    pub column_id: i32,
    pub value: String,
}
