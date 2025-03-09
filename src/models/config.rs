use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_file: String,
    pub max_mebibytes_for_hash: usize,
    pub backup_sources: Vec<BackupSource>,
    pub backup_destinations: Vec<String>,
    pub skip_source_hash_check_if_newer: bool,
    pub force_overwrite_backup: bool,
    pub overwrite_backup_if_existing_is_newer: bool,
    pub max_simultaneous_copy: usize,
}

#[derive(Debug, Deserialize)]
pub struct BackupSource {
    pub parent_directory: String,
    #[serde(default = "usize_max")]
    pub max_depth: usize,
    pub skip_dirs: Vec<String>,
}

fn usize_max() -> usize {
    usize::MAX
}

pub fn setup_config(config_file: String) -> Config {
    let config_file = PathBuf::from(config_file);
    let config_str = match fs::read_to_string(config_file) {
        Ok(file) => file,
        Err(e) => {
            panic!("Failed to read config file: {:?}", e);
        }
    };

    match serde_json::from_str(&config_str) {
        Ok(config) => config,
        Err(e) => {
            panic!("Failed to parse config file: {:?}", e);
        }
    }
}
