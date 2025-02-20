use std::time::Duration;

#[derive(Debug)]
pub struct BackupRow {
    pub source_id: i32,
    pub file_name: String,
    pub file_path: String,
    pub last_modified: Duration,
}
