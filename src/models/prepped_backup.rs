use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub struct PreppedBackup {
    pub db_id: i32,
    pub file_name: String,
    pub source_file_path: String,
    pub backup_paths: Vec<PathBuf>,
    pub hash: String,
    pub source_last_modified_date: Duration,
    pub updated: bool,
}
