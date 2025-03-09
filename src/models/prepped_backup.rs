use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug)]
pub struct PreppedBackup {
    pub db_id: i32,
    pub source_file: PathBuf,
    pub file_name: String,
    pub backup_paths: Vec<PathBuf>,
    pub hash: String,
    pub file_size: u64,
    pub source_last_modified_date: Duration,
    pub updated: bool,
}
