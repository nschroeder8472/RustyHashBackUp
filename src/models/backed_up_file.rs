use std::time::Duration;

#[derive(Debug)]
pub struct BackedUpFile {
    pub file_name: String,
    pub file_path: String,
    pub last_modified: Duration,
    pub hash: String,
}
