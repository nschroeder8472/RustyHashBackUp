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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_config_with_all_fields() {
        use tempfile::TempDir;

        // Create temp directories for testing
        let temp_source = TempDir::new().unwrap();
        let temp_dest1 = TempDir::new().unwrap();
        let temp_dest2 = TempDir::new().unwrap();

        let config_content = format!(r#"{{
            "database_file": "",
            "max_mebibytes_for_hash": 5,
            "backup_sources": [
                {{
                    "parent_directory": "{}",
                    "max_depth": 10,
                    "skip_dirs": ["target", "node_modules"]
                }}
            ],
            "backup_destinations": ["{}", "{}"],
            "skip_source_hash_check_if_newer": false,
            "force_overwrite_backup": true,
            "overwrite_backup_if_existing_is_newer": false,
            "max_threads": 8
        }}"#,
            temp_source.path().to_str().unwrap().replace("\\", "\\\\"),
            temp_dest1.path().to_str().unwrap().replace("\\", "\\\\"),
            temp_dest2.path().to_str().unwrap().replace("\\", "\\\\")
        );

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = setup_config(temp_file.path().to_str().unwrap().to_string()).unwrap();

        assert_eq!(config.database_file, "");
        assert_eq!(config.max_mebibytes_for_hash, 5);
        assert_eq!(config.backup_sources.len(), 1);
        assert_eq!(config.backup_destinations.len(), 2);
        assert_eq!(config.skip_source_hash_check_if_newer, false);
        assert_eq!(config.force_overwrite_backup, true);
        assert_eq!(config.max_threads, 8);
    }

    #[test]
    fn test_load_config_with_defaults() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();

        let config_content = format!(r#"{{
            "database_file": "",
            "backup_sources": [
                {{
                    "parent_directory": "{}"
                }}
            ],
            "backup_destinations": ["{}"]
        }}"#,
            temp_dir.path().to_str().unwrap().replace("\\", "\\\\"),
            temp_dir.path().to_str().unwrap().replace("\\", "\\\\")
        );

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(config_content.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let config = setup_config(temp_file.path().to_str().unwrap().to_string()).unwrap();

        // Check defaults are applied
        assert_eq!(config.max_mebibytes_for_hash, 1); // default
        assert_eq!(config.skip_source_hash_check_if_newer, true); // default
        assert_eq!(config.force_overwrite_backup, false); // default
        assert_eq!(config.overwrite_backup_if_existing_is_newer, false); // default
        assert_eq!(config.max_threads, num_cpus::get_physical()); // default
        assert_eq!(config.backup_sources[0].max_depth, usize::MAX); // default
        assert_eq!(config.backup_sources[0].skip_dirs.len(), 0); // default empty vec
    }

    #[test]
    fn test_error_on_missing_config_file() {
        let result = setup_config("/this/does/not/exist/config.json".to_string());

        assert!(result.is_err());
        match result {
            Err(crate::models::error::BackupError::ConfigRead { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected ConfigRead error"),
        }
    }

    #[test]
    fn test_error_on_invalid_json() {
        let invalid_json = r#"{
            "database_file": ":memory:",
            "backup_sources": [
                "parent_directory": "."  // Missing opening brace
            ]
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(invalid_json.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = setup_config(temp_file.path().to_str().unwrap().to_string());

        assert!(result.is_err());
        match result {
            Err(crate::models::error::BackupError::ConfigParse { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected ConfigParse error"),
        }
    }

    #[test]
    fn test_error_on_missing_required_fields() {
        let missing_sources = r#"{
            "database_file": ":memory:",
            "backup_destinations": ["."]
        }"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(missing_sources.as_bytes()).unwrap();
        temp_file.flush().unwrap();

        let result = setup_config(temp_file.path().to_str().unwrap().to_string());

        assert!(result.is_err());
        // Should fail at deserialization since backup_sources is required
        match result {
            Err(crate::models::error::BackupError::ConfigParse { .. }) => {
                // Expected error type
            }
            _ => panic!("Expected ConfigParse error for missing required field"),
        }
    }
}
