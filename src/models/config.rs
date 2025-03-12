use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub database_file: String,
    #[serde(default = "usize_one")]
    pub max_mebibytes_for_hash: usize,
    pub backup_sources: Vec<BackupSource>,
    pub backup_destinations: Vec<String>,
    #[serde(default = "bool_true")]
    pub skip_source_hash_check_if_newer: bool,
    #[serde(default = "bool_false")]
    pub force_overwrite_backup: bool,
    #[serde(default = "bool_false")]
    pub overwrite_backup_if_existing_is_newer: bool,
    #[serde(default = "usize_zero")]
    pub max_threads: usize,
}

#[derive(Debug, Deserialize)]
pub struct BackupSource {
    pub parent_directory: String,
    #[serde(default = "usize_max")]
    pub max_depth: usize,
    #[serde(default = "vec_default")]
    pub skip_dirs: Vec<String>,
}

const fn vec_default() -> Vec<String> { Vec::new() }
const fn usize_max() -> usize {
    usize::MAX
}
const fn usize_zero() -> usize {0}
const fn usize_one() -> usize {1}
const fn bool_false() -> bool { false }
const fn bool_true() -> bool { true }

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
