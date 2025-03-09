use std::time::Duration;

#[derive(Debug)]
pub struct SourceRow {
    pub id: i32,
    pub file_name: String,
    pub file_path: String,
    pub hash: String,
    pub file_size: u64,
    pub last_modified: Duration,
}
