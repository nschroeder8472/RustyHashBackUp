use crate::models::backup_row::BackupRow;
use crate::models::config::Config;
use crate::models::dry_run_mode::DryRunMode;
use crate::models::error::{BackupError, Result};
use crate::models::prepped_backup::PreppedBackup;
use crate::models::source_row::SourceRow;
use crate::repo::sqlite::{
    insert_backup_row, insert_source_row, select_backed_up_file, select_source,
    update_source_last_modified, update_source_row,
};
use crate::service::hash::hash_file;
use crate::utils::directory::{get_file_last_modified, get_file_size};
use indicatif::ProgressBar;
use log::{debug, error, info, warn};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn backup_files(
    backup_candidates: HashMap<PathBuf, Vec<PathBuf>>,
    config: &Config,
    prep_progress: Option<&ProgressBar>,
    backup_progress: Option<&ProgressBar>,
    dry_run_mode: DryRunMode,
    state: Option<&crate::api_state::AppState>,
) -> Result<()> {
    info!(
        "Starting backup to {} destinations...",
        config.backup_destinations.len()
    );

    let prepped_backup_candidates =
        prepare_backup_candidates(backup_candidates, config, prep_progress, dry_run_mode, state)?;
    info!(
        "Prepared {} files for backup",
        prepped_backup_candidates.len()
    );

    let total_files = prepped_backup_candidates.len() as u64;

    // Update API state: Starting backup phase
    if let Some(st) = state {
        st.set_progress(Some(crate::models::api::BackupProgress {
            phase: 3,
            phase_description: "Copying files".to_string(),
            files_processed: 0,
            total_files,
            bytes_processed: Some(0),
            total_bytes: Some(0), // Will be updated as we process
            percentage: 0.0,
            current_file: None,
        }));
    }

    let errors: Mutex<Vec<BackupError>> = Mutex::new(Vec::new());
    let backup_progress_arc = backup_progress.map(|pb| Arc::new(pb.clone()));
    let backup_files_processed = Arc::new(Mutex::new(0u64));
    let backup_bytes_processed = Arc::new(Mutex::new(0u64));

    prepped_backup_candidates
        .into_par_iter()
        .for_each(|prepped_backup_candidate| {
            // Check stop signal
            if let Some(st) = state {
                if st.is_stop_requested() {
                    warn!("Backup cancelled by user");
                    return;
                }
            }

            let mut files_copied = 0u64;
            let mut bytes_copied = 0u64;

            for backup_path in &prepped_backup_candidate.backup_paths {
                if config.force_overwrite_backup {
                    if dry_run_mode.should_copy_files() {
                        match backup_file(
                            &prepped_backup_candidate,
                            backup_path,
                            config,
                            dry_run_mode,
                        ) {
                            Ok(_) => {
                                files_copied += 1;
                                bytes_copied += prepped_backup_candidate.file_size;
                            }
                            Err(e) => {
                                errors.lock().unwrap().push(e);
                            }
                        }
                    } else {
                        // Dry-run mode: just log what would be copied
                        info!(
                            "Would copy: {:?} → {:?}",
                            prepped_backup_candidate.source_file, backup_path
                        );
                        files_copied += 1;
                        bytes_copied += prepped_backup_candidate.file_size;
                    }
                } else if let Ok(required) =
                    is_backup_required(&prepped_backup_candidate, backup_path, config, dry_run_mode)
                {
                    if required {
                        if dry_run_mode.should_copy_files() {
                            match backup_file(
                                &prepped_backup_candidate,
                                backup_path,
                                config,
                                dry_run_mode,
                            ) {
                                Ok(_) => {
                                    files_copied += 1;
                                    bytes_copied += prepped_backup_candidate.file_size;
                                }
                                Err(e) => {
                                    errors.lock().unwrap().push(e);
                                }
                            }
                        } else {
                            // Dry-run mode: just log what would be copied
                            info!(
                                "Would copy: {:?} → {:?}",
                                prepped_backup_candidate.source_file, backup_path
                            );
                            files_copied += 1;
                            bytes_copied += prepped_backup_candidate.file_size;
                        }
                    }
                }
            }

            if let Some(pb) = &backup_progress_arc {
                pb.inc(files_copied);
                pb.inc_length(bytes_copied);
            }

            // Update API progress for backup phase
            if let Some(st) = state {
                if files_copied > 0 {
                    let mut file_count = backup_files_processed.lock().unwrap();
                    *file_count += files_copied;
                    let current_files = *file_count;
                    drop(file_count);

                    let mut byte_count = backup_bytes_processed.lock().unwrap();
                    *byte_count += bytes_copied;
                    let current_bytes = *byte_count;
                    drop(byte_count);

                    st.set_progress(Some(crate::models::api::BackupProgress {
                        phase: 3,
                        phase_description: "Copying files".to_string(),
                        files_processed: current_files,
                        total_files,
                        bytes_processed: Some(current_bytes),
                        total_bytes: Some(current_bytes), // Progressive total
                        percentage: (current_files as f32 / total_files as f32) * 100.0,
                        current_file: Some(prepped_backup_candidate.file_name.clone()),
                    }));
                }
            }
        });

    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        let error_count = errors.len();
        // Log all errors
        for err in &errors {
            error!("Backup error: {}", err);
        }

        // Return warning if some files failed but operation partially succeeded
        warn!(
            "Backup completed with {} error(s). Some files may not have been backed up.",
            error_count
        );

        // Update API state with error information
        if let Some(st) = state {
            st.notify_message(format!(
                "Backup completed with {} error(s). Check logs for details.",
                error_count
            ));
        }

        // Don't fail completely if we had some successes, but log the issue
        // In a future enhancement, you could return a custom result type with warnings
    }
    Ok(())
}

fn prepare_backup_candidates(
    backup_candidates: HashMap<PathBuf, Vec<PathBuf>>,
    config: &Config,
    progress: Option<&ProgressBar>,
    dry_run_mode: DryRunMode,
    state: Option<&crate::api_state::AppState>,
) -> Result<Vec<PreppedBackup>> {
    let total_files: u64 = backup_candidates.values().map(|v| v.len() as u64).sum();

    // Update API state: Starting preparation phase
    if let Some(st) = state {
        st.set_progress(Some(crate::models::api::BackupProgress {
            phase: 2,
            phase_description: "Preparing backups".to_string(),
            files_processed: 0,
            total_files,
            bytes_processed: None,
            total_bytes: None,
            percentage: 0.0,
            current_file: None,
        }));
    }

    let prepped_backup_candidates: Mutex<Vec<PreppedBackup>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<BackupError>> = Mutex::new(Vec::new());
    let progress_arc = progress.map(|pb| Arc::new(pb.clone()));
    let processed_count = Arc::new(Mutex::new(0u64));

    backup_candidates
        .into_par_iter()
        .for_each(|(shared_path, candidates)| {
            for candidate in candidates {
                // Check stop signal
                if let Some(st) = state {
                    if st.is_stop_requested() {
                        warn!("Backup preparation cancelled by user");
                        return;
                    }
                }

                match prepare_single_candidate(&candidate, &shared_path, config, dry_run_mode) {
                    Ok(prepped) => {
                        prepped_backup_candidates.lock().unwrap().push(prepped);
                        if let Some(pb) = &progress_arc {
                            pb.inc(1);
                        }

                        // Update API progress
                        if let Some(st) = state {
                            let mut count = processed_count.lock().unwrap();
                            *count += 1;
                            let current_count = *count;
                            drop(count);

                            st.set_progress(Some(crate::models::api::BackupProgress {
                                phase: 2,
                                phase_description: "Preparing backups".to_string(),
                                files_processed: current_count,
                                total_files,
                                bytes_processed: None,
                                total_bytes: None,
                                percentage: (current_count as f32 / total_files as f32) * 100.0,
                                current_file: Some(candidate.to_string_lossy().to_string()),
                            }));
                        }
                    }
                    Err(e) => {
                        errors.lock().unwrap().push(e);
                        if let Some(pb) = &progress_arc {
                            pb.inc(1);
                        }

                        // Update API progress even on error
                        if let Some(st) = state {
                            let mut count = processed_count.lock().unwrap();
                            *count += 1;
                            let current_count = *count;
                            drop(count);

                            st.set_progress(Some(crate::models::api::BackupProgress {
                                phase: 2,
                                phase_description: "Preparing backups".to_string(),
                                files_processed: current_count,
                                total_files,
                                bytes_processed: None,
                                total_bytes: None,
                                percentage: (current_count as f32 / total_files as f32) * 100.0,
                                current_file: None,
                            }));
                        }
                    }
                }
            }
        });

    let errors = errors.into_inner().unwrap();
    let prepped = prepped_backup_candidates.into_inner().unwrap();

    if !errors.is_empty() {
        let error_count = errors.len();
        // Log all errors
        for err in &errors {
            error!("Preparation error: {}", err);
        }

        warn!(
            "Preparation completed with {} error(s). {} files prepared successfully.",
            error_count,
            prepped.len()
        );

        // Update API state with error information
        if let Some(st) = state {
            st.notify_message(format!(
                "Preparation had {} error(s), but {} files were prepared successfully.",
                error_count,
                prepped.len()
            ));
        }
    }

    Ok(prepped)
}

fn prepare_single_candidate(
    candidate: &PathBuf,
    shared_path: &PathBuf,
    config: &Config,
    dry_run_mode: DryRunMode,
) -> Result<PreppedBackup> {
    let filename = candidate
        .file_name()
        .ok_or_else(|| BackupError::DirectoryRead(format!("No filename for {:?}", candidate)))?
        .to_string_lossy()
        .to_string();

    let filepath = candidate
        .parent()
        .ok_or_else(|| BackupError::DirectoryRead(format!("No parent path for {:?}", candidate)))?
        .to_string_lossy()
        .to_string();

    let fs_last_modified = get_file_last_modified(candidate)?;
    let fs_file_size = get_file_size(candidate)?;

    let db_source_record_option = if dry_run_mode.should_update_database() {
        select_source(&filename, &filepath).map_err(|cause| BackupError::DatabaseQuery {
            operation: format!("select source {}{}{}", filepath, MAIN_SEPARATOR, filename),
            cause,
        })?
    } else {
        None
    };

    let (updated, hash, source_id) = if let Some(db_source_record) = db_source_record_option {
        let (updated, hash) = get_is_source_file_updated(
            &db_source_record,
            candidate,
            &fs_last_modified,
            config,
            dry_run_mode,
        )?;
        (updated, hash, db_source_record.id)
    } else {
        let hash = if dry_run_mode.should_hash() {
            hash_file(candidate, &config.max_mebibytes_for_hash)?
        } else {
            debug!("Quick mode: skipping hash for {:?}", candidate);
            String::from("dry-run-quick-no-hash")
        };

        let source_id = if dry_run_mode.should_update_database() {
            let source_row = SourceRow {
                id: 0,
                file_name: filename.clone(),
                file_path: filepath.clone(),
                hash: hash.clone(),
                file_size: fs_file_size,
                last_modified: fs_last_modified,
            };
            insert_source_row(&source_row)?
        } else {
            // Dry-run mode: use placeholder ID
            0
        };

        (true, hash, source_id)
    };

    let backup_paths = get_possible_backups(
        &filename,
        &filepath,
        shared_path,
        &config.backup_destinations,
    )?;

    Ok(PreppedBackup {
        db_id: source_id,
        source_file: candidate.clone(),
        file_name: filename,
        backup_paths,
        hash,
        file_size: fs_file_size,
        source_last_modified_date: fs_last_modified,
        updated,
    })
}

fn is_backup_required(
    prepped_backup: &PreppedBackup,
    back_up_path: &PathBuf,
    config: &Config,
    dry_run_mode: DryRunMode,
) -> Result<bool> {
    let exists = fs::exists(back_up_path).unwrap_or(false);

    if !exists {
        debug!(
            "{:?} backup does not exist at {:?}",
            prepped_backup.source_file, back_up_path
        );
        return Ok(true);
    }

    debug!(
        "{:?} backup exists at {:?}. Checking if update needed",
        prepped_backup.source_file, back_up_path
    );
    existing_file_needs_updated(prepped_backup, back_up_path, config, dry_run_mode)
}

fn existing_file_needs_updated(
    prepped_backup: &PreppedBackup,
    back_up_path: &PathBuf,
    config: &Config,
    dry_run_mode: DryRunMode,
) -> Result<bool> {
    if !fs::exists(back_up_path).unwrap_or(false) {
        return Ok(true);
    }

    if dry_run_mode.is_quick() {
        let fs_file_size = get_file_size(back_up_path)?;
        if prepped_backup.file_size != fs_file_size {
            debug!(
                "Quick mode: File size differs, would update: {:?}",
                back_up_path
            );
            return Ok(true);
        }
        debug!(
            "Quick mode: File size matches, would skip: {:?}",
            back_up_path
        );
        return Ok(false);
    }

    let back_up_filename = back_up_path
        .file_name()
        .ok_or_else(|| BackupError::DirectoryRead(format!("No filename for {:?}", back_up_path)))?
        .to_string_lossy()
        .to_string();

    let back_up_filepath = back_up_path
        .parent()
        .ok_or_else(|| BackupError::DirectoryRead(format!("No parent for {:?}", back_up_path)))?
        .to_string_lossy()
        .to_string();

    let fs_last_modified = get_file_last_modified(back_up_path)?;
    let fs_file_size = get_file_size(back_up_path)?;

    let dbase_backup_file_option = if dry_run_mode.should_update_database() {
        select_backed_up_file(&back_up_filename, &back_up_filepath).map_err(|cause| {
            BackupError::DatabaseQuery {
                operation: format!(
                    "select backup {}{}{}",
                    back_up_filepath, MAIN_SEPARATOR, back_up_filename
                ),
                cause,
            }
        })?
    } else {
        None
    };

    match dbase_backup_file_option {
        Some(backup_file) => {
            if backup_file.last_modified.as_secs() <= fs_last_modified.as_secs() {
                if prepped_backup.file_size == fs_file_size {
                    let fs_hash = hash_file(back_up_path, &config.max_mebibytes_for_hash)?;
                    if backup_file.hash == fs_hash {
                        debug!("Existing backup file is up to date: {:?}", back_up_path);
                        return Ok(false);
                    }
                }
                debug!("Existing backup file needs update: {:?}", back_up_path);
                Ok(true)
            } else if config.overwrite_backup_if_existing_is_newer {
                warn!(
                    "Existing backup file is newer than database, config forces override: {:?}",
                    back_up_path
                );
                Ok(true)
            } else {
                warn!(
                    "Existing backup file is newer than database, skipping: {:?}",
                    back_up_path
                );
                Ok(false)
            }
        }
        None => {
            debug!(
                "Unknown backup file found, checking if same as source: {:?}",
                back_up_path
            );
            if prepped_backup.file_size == fs_file_size {
                let fs_hash = hash_file(back_up_path, &config.max_mebibytes_for_hash)?;
                if *prepped_backup.hash == fs_hash {
                    info!(
                        "Unknown backup matches source, adding to database: {:?}",
                        back_up_path
                    );
                    if dry_run_mode.should_update_database() {
                        let backup_row = create_backup_row(prepped_backup, back_up_path)?;
                        insert_backup_row(backup_row)?;
                    }
                    return Ok(false);
                }
            }
            debug!("Unknown backup differs from source: {:?}", back_up_path);
            Ok(true)
        }
    }
}

fn backup_file(
    prepped_backup: &PreppedBackup,
    backup_path: &PathBuf,
    config: &Config,
    dry_run_mode: DryRunMode,
) -> Result<()> {
    // Note: In dry-run modes, this function should not be called since we log directly in backup_files()
    // But if it is called, we still respect the dry_run_mode
    if !dry_run_mode.should_copy_files() {
        debug!(
            "Dry-run mode: Would copy {:?} → {:?}",
            &prepped_backup.source_file, backup_path
        );
        return Ok(());
    }

    let parent = backup_path.parent().ok_or_else(|| {
        BackupError::DirectoryRead(format!("No parent directory for {:?}", backup_path))
    })?;

    if !fs::exists(parent).unwrap_or(false) {
        fs::create_dir_all(parent)?;
    }

    info!(
        "Copying: {:?} → {:?}",
        &prepped_backup.source_file, backup_path
    );

    fs::copy(&prepped_backup.source_file, backup_path).map_err(|cause| BackupError::FileCopy {
        from: prepped_backup.source_file.clone(),
        to: backup_path.clone(),
        cause,
    })?;

    debug!("Verifying backup integrity: {:?}", backup_path);
    let backup_hash = hash_file(backup_path, &config.max_mebibytes_for_hash)?;

    if backup_hash != prepped_backup.hash {
        warn!(
            "Backup verification FAILED for {:?}: hash mismatch! Deleting corrupted backup.",
            backup_path
        );
        if let Err(e) = fs::remove_file(backup_path) {
            error!(
                "Failed to delete corrupted backup file {:?}: {}",
                backup_path, e
            );
        }
        return Err(BackupError::DirectoryRead(format!(
            "Backup verification failed for {:?}: source hash {} != backup hash {}",
            backup_path, prepped_backup.hash, backup_hash
        )));
    }

    debug!("Backup verification passed: {:?}", backup_path);

    let backup_row = create_backup_row(prepped_backup, backup_path)?;
    insert_backup_row(backup_row)?;
    Ok(())
}

fn create_backup_row(prepped_backup: &PreppedBackup, backup_path: &PathBuf) -> Result<BackupRow> {
    let last_modified = get_file_last_modified(backup_path)?;
    let file_path = backup_path
        .parent()
        .ok_or_else(|| BackupError::DirectoryRead(format!("No parent for {:?}", backup_path)))?
        .to_str()
        .ok_or_else(|| {
            BackupError::DirectoryRead(format!("Invalid path encoding for {:?}", backup_path))
        })?
        .to_string();

    Ok(BackupRow {
        source_id: prepped_backup.db_id,
        file_name: prepped_backup.file_name.clone(),
        file_path,
        last_modified,
    })
}

fn get_is_source_file_updated(
    source_candidate: &SourceRow,
    backup_candidate: &PathBuf,
    candidate_last_modified: &Duration,
    config: &Config,
    dry_run_mode: DryRunMode,
) -> Result<(bool, String)> {
    let hash: String;
    let backup_file_size = get_file_size(backup_candidate)?;

    if source_candidate.last_modified.as_secs() < candidate_last_modified.as_secs() {
        if config.skip_source_hash_check_if_newer {
            hash = source_candidate.hash.clone();
            Ok((true, hash))
        } else {
            // Skip hashing in Quick mode
            hash = if dry_run_mode.should_hash() {
                hash_file(backup_candidate, &config.max_mebibytes_for_hash)?
            } else {
                debug!("Quick mode: skipping hash check for {:?}", backup_candidate);
                source_candidate.hash.clone()
            };

            if hash == source_candidate.hash && backup_file_size == source_candidate.file_size {
                if dry_run_mode.should_update_database() {
                    update_source_last_modified(source_candidate.id, candidate_last_modified)?;
                }
                Ok((false, hash))
            } else {
                if dry_run_mode.should_update_database() {
                    update_source_row(
                        source_candidate.id,
                        &hash,
                        &backup_file_size,
                        candidate_last_modified,
                    )?;
                }
                Ok((true, hash))
            }
        }
    } else {
        hash = source_candidate.hash.clone();
        Ok((false, hash))
    }
}

fn get_possible_backups(
    file_name: &str,
    file_path: &str,
    shared_path: &PathBuf,
    destinations: &[String],
) -> Result<Vec<PathBuf>> {
    let relative_path = if let Some(parent) = shared_path.parent() {
        let parent_str = parent.to_str().ok_or_else(|| {
            BackupError::DirectoryRead(format!("Invalid path encoding for {:?}", parent))
        })?;
        file_path.trim_start_matches(parent_str)
    } else {
        let shared_str = shared_path.to_str().ok_or_else(|| {
            BackupError::DirectoryRead(format!("Invalid path encoding for {:?}", shared_path))
        })?;
        file_path.trim_start_matches(shared_str)
    };

    // Security: Check for path traversal attempts
    if relative_path.contains("..") {
        return Err(BackupError::DirectoryRead(format!(
            "Path traversal detected in relative path: {}. File path may contain '..' sequences.",
            relative_path
        )));
    }

    // Security: Check file name for path traversal
    if file_name.contains("..") || file_name.contains(MAIN_SEPARATOR) {
        return Err(BackupError::DirectoryRead(format!(
            "Invalid file name detected: {}. File names cannot contain '..' or path separators.",
            file_name
        )));
    }

    let mut possible_backup_paths = Vec::new();
    for destination in destinations {
        let dest_path = Path::new(destination);
        let backup_path = dest_path
            .join(relative_path.trim_start_matches(MAIN_SEPARATOR))
            .join(file_name);

        // Security: Verify the constructed path is actually within the destination
        // Canonicalize both paths to resolve any symbolic links or relative components
        let canonical_dest = dest_path.canonicalize().unwrap_or_else(|_| dest_path.to_path_buf());

        // For the backup path, we can't canonicalize if it doesn't exist yet,
        // so we check if its parent (when canonicalized) starts with the destination
        if let Some(backup_parent) = backup_path.parent() {
            // If parent exists, canonicalize it; otherwise use as-is
            let canonical_parent = backup_parent
                .canonicalize()
                .unwrap_or_else(|_| backup_parent.to_path_buf());

            if !canonical_parent.starts_with(&canonical_dest) {
                return Err(BackupError::DirectoryRead(format!(
                    "Security: Backup path escapes destination directory. Destination: {:?}, Attempted path: {:?}",
                    destination, backup_path
                )));
            }
        }

        possible_backup_paths.push(backup_path);
    }
    Ok(possible_backup_paths)
}
