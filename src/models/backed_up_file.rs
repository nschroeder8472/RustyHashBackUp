use std::time::Duration;

#[derive(Debug)]
pub struct BackedUpFile {
    #[allow(dead_code)]
    pub file_name: String,
    #[allow(dead_code)]
    pub file_path: String,
    pub last_modified: Duration,
    pub hash: String,
}
