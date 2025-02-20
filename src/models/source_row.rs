use std::time::Duration;

#[derive(Debug)]
pub struct SourceRow {
    pub file_name: String,
    pub file_path: String,
    pub hash: String,
    pub last_modified: Duration,
}
