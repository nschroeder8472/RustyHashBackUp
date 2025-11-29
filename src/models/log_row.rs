use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct LogRow {
    pub id: i64,
    pub timestamp: i64,
    pub level: String,
    pub message: String,
    pub context: Option<String>,
    pub source: Option<String>,
}
