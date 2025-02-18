use std::time::Duration;

#[derive(Debug)]
pub struct FileHash {
    pub file_name: String,
    pub file_path: String,
    pub hash: String,
    pub date: Duration,
}
