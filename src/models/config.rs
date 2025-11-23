use crate::models::config_validator::validate_config;
use crate::models::error::{BackupError, Result};
use log::info;
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
    #[serde(default = "default_max_threads")]
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
const fn usize_one() -> usize {1}
const fn bool_false() -> bool { false }
const fn bool_true() -> bool { true }
fn default_max_threads() -> usize {
    num_cpus::get_physical()
}

pub fn setup_config(config_file: String) -> Result<Config> {
    let config_path = PathBuf::from(config_file);
    info!("Loading config from: {}", config_path.display());

    let config_str = fs::read_to_string(&config_path).map_err(|cause| {
        BackupError::ConfigRead {
            path: config_path.clone(),
            cause,
        }
    })?;

    let config: Config = serde_json::from_str(&config_str).map_err(|cause| {
        BackupError::ConfigParse {
            path: config_path,
            cause,
        }
    })?;

    // Validate configuration
    validate_config(&config)?;

    Ok(config)
}
