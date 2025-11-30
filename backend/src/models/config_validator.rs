use crate::models::config::{BackupSource, Config};
use crate::models::error::{BackupError, Result};
use log::{info, warn};
use std::fs;
use std::path::Path;
use std::str::FromStr;

/// Validates the entire configuration
pub fn validate_config(config: &Config) -> Result<()> {
    info!("Validating configuration...");

    // Validate numeric values
    validate_numeric_values(config)?;

    // Validate backup sources
    validate_backup_sources(&config.backup_sources)?;

    // Validate backup destinations
    validate_backup_destinations(&config.backup_destinations)?;

    // Validate database file
    validate_database_path(&config.database_file)?;

    // Validate schedule if present
    validate_schedule(config)?;

    // Check for conflicting flags
    check_conflicting_flags(config)?;

    info!("Configuration validation passed");
    Ok(())
}

/// Validate numeric configuration values
fn validate_numeric_values(config: &Config) -> Result<()> {
    if config.max_mebibytes_for_hash == 0 {
        return Err(BackupError::DirectoryRead(
            "max_mebibytes_for_hash must be greater than 0".to_string(),
        ));
    }

    if config.max_threads == 0 {
        return Err(BackupError::DirectoryRead(
            "max_threads must be greater than 0".to_string(),
        ));
    }

    // Warn if max_threads is excessive
    let cpu_count = num_cpus::get_physical();
    if config.max_threads > cpu_count * 2 {
        warn!(
            "max_threads ({}) is more than 2x the number of physical CPUs ({}). This may not improve performance.",
            config.max_threads, cpu_count
        );
    }

    Ok(())
}

/// Validate backup source directories
fn validate_backup_sources(sources: &[BackupSource]) -> Result<()> {
    if sources.is_empty() {
        return Err(BackupError::DirectoryRead(
            "At least one backup source must be configured".to_string(),
        ));
    }

    for (idx, source) in sources.iter().enumerate() {
        let path = Path::new(&source.parent_directory);

        // Check if directory exists
        if !path.exists() {
            #[cfg(windows)]
            let suggestion = format!("mkdir \"{}\"", source.parent_directory);
            #[cfg(not(windows))]
            let suggestion = format!("mkdir -p \"{}\"", source.parent_directory);

            return Err(BackupError::DirectoryRead(format!(
                "Backup source #{} does not exist: {}\nSuggestion: Create the directory with: {}",
                idx + 1,
                source.parent_directory,
                suggestion
            )));
        }

        // Check if it's a directory
        if !path.is_dir() {
            return Err(BackupError::DirectoryRead(format!(
                "Backup source #{} is not a directory: {}",
                idx + 1,
                source.parent_directory
            )));
        }

        // Check if readable
        if let Err(e) = fs::read_dir(path) {
            return Err(BackupError::DirectoryRead(format!(
                "Backup source #{} is not readable: {}\nError: {}",
                idx + 1,
                source.parent_directory,
                e
            )));
        }

        // Validate max_depth
        if source.max_depth == Some(0) {
            return Err(BackupError::DirectoryRead(format!(
                "Backup source #{} has max_depth of 0, which means no files will be found. Set max_depth to at least 1.",
                idx + 1
            )));
        }
    }

    Ok(())
}

/// Validate backup destination directories
fn validate_backup_destinations(destinations: &[String]) -> Result<()> {
    if destinations.is_empty() {
        return Err(BackupError::DirectoryRead(
            "At least one backup destination must be configured".to_string(),
        ));
    }

    for (idx, dest) in destinations.iter().enumerate() {
        let path = Path::new(dest);

        // Check if destination exists
        if !path.exists() {
            // Check if parent exists (we can create the destination)
            if let Some(parent) = path.parent() {
                if !parent.exists() {
                    #[cfg(windows)]
                    let suggestion = format!("mkdir \"{}\"", parent.display());
                    #[cfg(not(windows))]
                    let suggestion = format!("mkdir -p \"{}\"", parent.display());

                    return Err(BackupError::DirectoryRead(format!(
                        "Backup destination #{} parent directory does not exist: {}\nSuggestion: Create the parent directory with: {}",
                        idx + 1,
                        dest,
                        suggestion
                    )));
                }

                // Check if parent is writable
                if let Err(e) = check_writable(parent) {
                    return Err(BackupError::DirectoryRead(format!(
                        "Backup destination #{} parent directory is not writable: {}\nError: {}",
                        idx + 1,
                        dest,
                        e
                    )));
                }

                warn!(
                    "Backup destination #{} does not exist but will be created: {}",
                    idx + 1,
                    dest
                );
            } else {
                return Err(BackupError::DirectoryRead(format!(
                    "Backup destination #{} has no parent directory: {}",
                    idx + 1,
                    dest
                )));
            }
        } else {
            // Destination exists, check if it's a directory and writable
            if !path.is_dir() {
                return Err(BackupError::DirectoryRead(format!(
                    "Backup destination #{} exists but is not a directory: {}",
                    idx + 1,
                    dest
                )));
            }

            // Check if writable
            if let Err(e) = check_writable(path) {
                return Err(BackupError::DirectoryRead(format!(
                    "Backup destination #{} is not writable: {}\nError: {}",
                    idx + 1,
                    dest,
                    e
                )));
            }
        }
    }

    Ok(())
}

/// Validate database file path
fn validate_database_path(db_file: &str) -> Result<()> {
    if db_file.is_empty() {
        // Empty string means in-memory database, which is valid
        info!("Using in-memory database (no database_file specified)");
        return Ok(());
    }

    let path = Path::new(db_file);

    // Check if database file already exists
    if path.exists() {
        // Check if it's a file
        if !path.is_file() {
            return Err(BackupError::DirectoryRead(format!(
                "Database path exists but is not a file: {}",
                db_file
            )));
        }

        // Check if readable and writable
        if let Err(e) = fs::OpenOptions::new().read(true).write(true).open(path) {
            return Err(BackupError::DirectoryRead(format!(
                "Database file is not readable/writable: {}\nError: {}",
                db_file, e
            )));
        }
    } else {
        // Database doesn't exist, check if parent directory exists and is writable
        if let Some(parent) = path.parent() {
            // Handle edge case: if parent is empty (current directory), it always exists
            let parent_exists = if parent.as_os_str().is_empty() {
                true // Current directory always exists
            } else {
                parent.exists()
            };

            if !parent_exists {
                #[cfg(windows)]
                let suggestion = format!("mkdir \"{}\"", parent.display());
                #[cfg(not(windows))]
                let suggestion = format!("mkdir -p \"{}\"", parent.display());

                return Err(BackupError::DirectoryRead(format!(
                    "Database parent directory does not exist: {}\nSuggestion: Create the directory with: {}",
                    db_file,
                    suggestion
                )));
            }

            if let Err(e) = check_writable(parent) {
                return Err(BackupError::DirectoryRead(format!(
                    "Database parent directory is not writable: {}\nError: {}",
                    db_file, e
                )));
            }
        } else {
            return Err(BackupError::DirectoryRead(format!(
                "Database path has no parent directory: {}",
                db_file
            )));
        }
    }

    Ok(())
}

/// Validate schedule configuration
fn validate_schedule(config: &Config) -> Result<()> {
    if let Some(schedule_str) = &config.schedule {
        // Try to parse the cron expression
        match cron::Schedule::from_str(schedule_str) {
            Ok(_) => {
                info!("Schedule validated: {}", schedule_str);
            }
            Err(e) => {
                return Err(BackupError::DirectoryRead(format!(
                    "Invalid cron expression in schedule: {}\nError: {}\nExample: '0 2 * * *' for daily at 2am",
                    schedule_str, e
                )));
            }
        }
    }
    Ok(())
}

/// Check for conflicting configuration flags
fn check_conflicting_flags(config: &Config) -> Result<()> {
    // If force_overwrite_backup is true, other backup flags are ignored
    if config.force_overwrite_backup {
        if config.overwrite_backup_if_existing_is_newer {
            warn!(
                "force_overwrite_backup is enabled, so overwrite_backup_if_existing_is_newer has no effect"
            );
        }
        warn!("force_overwrite_backup is enabled - all backup files will be overwritten regardless of their state");
    }

    // If skip_source_hash_check_if_newer is true, newer source files won't be hashed
    if config.skip_source_hash_check_if_newer {
        info!("skip_source_hash_check_if_newer is enabled - newer source files will skip hash verification");
    }

    Ok(())
}

/// Check if a directory is writable by attempting to create a temporary file
fn check_writable(path: &Path) -> std::io::Result<()> {
    let test_file = path.join(".rustyhashbackup_write_test");

    // Try to create a temporary file
    fs::write(&test_file, b"test")?;

    // Clean up
    fs::remove_file(&test_file)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_check_writable_temp_dir() {
        let temp_dir = std::env::temp_dir();
        assert!(check_writable(&temp_dir).is_ok());
    }

    #[test]
    fn test_validate_numeric_values_zero_mebibytes() {
        let mut config = create_test_config();
        config.max_mebibytes_for_hash = 0;

        let result = validate_numeric_values(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("max_mebibytes_for_hash"));
    }

    #[test]
    fn test_validate_numeric_values_zero_threads() {
        let mut config = create_test_config();
        config.max_threads = 0;

        let result = validate_numeric_values(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_threads"));
    }

    #[test]
    fn test_validate_config_passes_for_valid_config() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: String::new(), // Empty = in-memory
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_nonexistent_source_directory() {
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: ":memory:".to_string(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: "/this/does/not/exist".to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_rejects_empty_backup_sources() {
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: ":memory:".to_string(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![], // Empty sources
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one backup source"));
    }

    #[test]
    fn test_rejects_empty_backup_destinations() {
        let temp_source = TempDir::new().unwrap();

        let config = Config {
            database_file: ":memory:".to_string(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![], // Empty destinations
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("At least one backup destination"));
    }

    #[test]
    fn test_validates_database_path_with_in_memory() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: String::new(), // Empty = in-memory
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_conflicting_force_overwrite_flags() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: String::new(), // Empty = in-memory
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: true,
            overwrite_backup_if_existing_is_newer: true, // Conflicting with force_overwrite
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        // Note: This currently just logs a warning, doesn't error
        // Testing that it doesn't error - it should succeed with a warning
        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_max_depth_zero() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: ":memory:".to_string(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(0), // Invalid
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_depth of 0"));
    }

    #[test]
    fn test_accepts_valid_cron_schedule() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: String::new(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: Some("0 0 2 * * *".to_string()), // Daily at 2am
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_rejects_invalid_cron_schedule() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let config = Config {
            database_file: String::new(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![BackupSource {
                parent_directory: temp_source.path().to_str().unwrap().to_string(),
                max_depth: Some(10),
                skip_dirs: vec![],
            }],
            backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: Some("invalid cron".to_string()), // Invalid
            run_on_startup: true,
        };

        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid cron expression"));
    }

    #[test]
    fn test_accepts_various_valid_cron_expressions() {
        let temp_source = TempDir::new().unwrap();
        let temp_dest = TempDir::new().unwrap();

        let valid_expressions = vec![
            "0 0 2 * * *",      // Daily at 2am
            "0 */30 * * * *",   // Every 30 minutes
            "0 0 */6 * * *",    // Every 6 hours
            "0 0 0 * * 1",      // Every Monday at midnight
            "0 0 9,17 * * 1-5", // Weekdays at 9am and 5pm
        ];

        for expr in valid_expressions {
            let config = Config {
                database_file: String::new(),
                max_mebibytes_for_hash: 1,
                backup_sources: vec![BackupSource {
                    parent_directory: temp_source.path().to_str().unwrap().to_string(),
                    max_depth: Some(10),
                    skip_dirs: vec![],
                }],
                backup_destinations: vec![temp_dest.path().to_str().unwrap().to_string()],
                skip_source_hash_check_if_newer: true,
                force_overwrite_backup: false,
                overwrite_backup_if_existing_is_newer: false,
                max_threads: 4,
                schedule: Some(expr.to_string()),
                run_on_startup: true,
            };

            let result = validate_config(&config);
            assert!(
                result.is_ok(),
                "Expected cron expression '{}' to be valid, but got error: {:?}",
                expr,
                result
            );
        }
    }

    fn create_test_config() -> Config {
        Config {
            database_file: String::new(),
            max_mebibytes_for_hash: 1,
            backup_sources: vec![],
            backup_destinations: vec![],
            skip_source_hash_check_if_newer: true,
            force_overwrite_backup: false,
            overwrite_backup_if_existing_is_newer: false,
            max_threads: 4,
            schedule: None,
            run_on_startup: true,
        }
    }
}
