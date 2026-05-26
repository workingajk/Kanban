use chrono::{DateTime, Local};

pub struct Column {
    pub id: i64,
    pub name: String,
    #[allow(dead_code)]
    pub position: i32,
}

#[derive(Clone)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub column_id: i64,
    pub column_name: String,
    #[allow(dead_code)]
    pub position: i32,
    pub created_at: DateTime<Local>,
    pub description: String,
    pub priority: String, // "Low", "Medium", "High"
    pub due_date: String,
}
