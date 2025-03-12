use crate::models::backup_row::BackupRow;
use crate::models::config::Config;
use crate::models::prepped_backup::PreppedBackup;
use crate::models::source_row::SourceRow;
use crate::repo::sqlite::{
    insert_backup_row, insert_source_row, select_backed_up_file, select_source, update_source_row,
    update_source_last_modified,
};
use crate::service::hash::hash_file;
use crate::utils::directory::{get_file_last_modified, get_file_size};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::sync::Mutex;
use std::time::Duration;

pub fn backup_files(backup_candidates: HashMap<PathBuf, Vec<PathBuf>>, config: &Config) {
    let prepped_backup_candidates = prepare_backup_candidates(backup_candidates, config);
    prepped_backup_candidates.into_par_iter().for_each(|prepped_backup_candidate| {
        for backup_path in &prepped_backup_candidate.backup_paths {
            if config.force_overwrite_backup || is_backup_required(&prepped_backup_candidate, backup_path, config) {
                backup_file(&prepped_backup_candidate, backup_path)
            }
        }
    })
}

fn prepare_backup_candidates(backup_candidates: HashMap<PathBuf, Vec<PathBuf>>, config: &Config) -> Vec<PreppedBackup> {
    let mut prepped_backup_candidates: Mutex<Vec<PreppedBackup>> = Mutex::new(Vec::new());
    backup_candidates.into_par_iter().for_each(|(shared_path, candidates)| {
        for candidate in candidates {
            let filename = candidate
                .file_name()
                .unwrap()
                .to_os_string()
                .to_string_lossy()
                .to_string();
            let filepath = candidate.parent().unwrap().to_string_lossy().to_string();
            let fs_last_modified = get_file_last_modified(&candidate);
            let hash;
            let fs_file_size = get_file_size(&candidate);
            let mut source_candidate;
            let db_source_record_option = match select_source(&filename, &filepath) {
                Ok(s) => s,
                Err(e) => {
                    panic!(
                        "Failed to select from database for {}{}{}: {:?}",
                        filepath, MAIN_SEPARATOR, filename, e
                    );
                }
            };
            let updated: bool;
            if db_source_record_option.is_some() {
                let db_source_record = db_source_record_option.unwrap();
                (updated, hash) = get_is_source_file_updated(
                    &db_source_record,
                    &candidate,
                    &fs_last_modified,
                    &config,
                );
                source_candidate = SourceRow {
                    id: db_source_record.id,
                    file_name: filename.to_owned(),
                    file_path: filepath.to_owned(),
                    hash: hash.to_owned(),
                    file_size: fs_file_size.to_owned(),
                    last_modified: fs_last_modified,
                };
            } else {
                hash = hash_file(&candidate, &config.max_mebibytes_for_hash);
                source_candidate = SourceRow {
                    id: 0,
                    file_name: filename.to_owned(),
                    file_path: filepath.to_owned(),
                    hash: hash.to_owned(),
                    file_size: fs_file_size.to_owned(),
                    last_modified: fs_last_modified,
                };
                let id = match insert_source_row(&source_candidate) {
                    Ok(id) => id,
                    Err(e) => {
                        panic!(
                            "Failed to insert source row for {}{}{}: {:?}",
                            filepath, MAIN_SEPARATOR, filename, e
                        );
                    }
                };
                source_candidate.id = id;
                updated = true
            };

            let backup_paths = get_possible_backups(
                &filename,
                &filepath,
                &shared_path,
                &config.backup_destinations,
            );

            prepped_backup_candidates.lock().unwrap().push(PreppedBackup {
                db_id: source_candidate.id,
                source_file: candidate,
                file_name: filename,
                backup_paths: backup_paths,
                hash: hash.to_owned(),
                file_size: fs_file_size.to_owned(),
                source_last_modified_date: fs_last_modified,
                updated: updated,
            })
        }
    });
    prepped_backup_candidates.into_inner().unwrap()
}

fn is_backup_required(prepped_backup: &PreppedBackup, back_up_path: &PathBuf, config: &Config) -> bool {
    if prepped_backup.updated && !fs::exists(back_up_path).unwrap() {  // updated, backup does not exist
        println!("{:?} Source File Updated, and Backup does not exist at destination {:?}",
            prepped_backup.source_file, back_up_path);
        true
    } else if prepped_backup.updated && fs::exists(back_up_path).unwrap() { // updated, and backup exists
        println!("{:?} Source File Updated, and Backup exists at destination {:?}. Checking if existing file needs update",
                 prepped_backup.source_file, back_up_path);
        existing_file_needs_updated(&prepped_backup, back_up_path, config)
    } else if !prepped_backup.updated && !fs::exists(back_up_path).unwrap() { //not updated, backup does not exist
        println!("{:?} Source File Not Updated, and Backup does not exist at destination {:?}",
                 prepped_backup.source_file, back_up_path);
        true
    } else { // not updated, and backup exists in file system
        println!("{:?} Source File Not Updated, and Backup exists at destination {:?}. Checking if existing file needs update",
                 prepped_backup.source_file, back_up_path);
        existing_file_needs_updated(&prepped_backup, back_up_path, config)
    }
}

fn existing_file_needs_updated(prepped_backup: &PreppedBackup, back_up_path: &PathBuf, config: &Config) -> bool {
    if !fs::exists(back_up_path).unwrap() { return true }
    let back_up_filename = back_up_path
        .file_name()
        .unwrap()
        .to_os_string()
        .to_string_lossy()
        .to_string();
    let back_up_filepath = back_up_path.parent().unwrap().to_string_lossy().to_string();
    let fs_last_modified = get_file_last_modified(back_up_path);
    let fs_file_size = get_file_size(back_up_path);
    let dbase_backup_file_option =
        match select_backed_up_file(&back_up_filename, &back_up_filepath) {
            Ok(backup_file_option) => backup_file_option,
            Err(e) => panic!(
                "Failed to select from backup database for {}{}{}: {:?}",
                back_up_filepath, MAIN_SEPARATOR, back_up_filename, e
            ),
        };
    match dbase_backup_file_option {
        Some(backup_file) => {
            if backup_file.last_modified.as_secs() <= fs_last_modified.as_secs() {
                if prepped_backup.file_size == fs_file_size {
                    let fs_hash = hash_file(back_up_path, &config.max_mebibytes_for_hash);
                    if backup_file.hash == fs_hash {
                        println!("Existing backup file is up to date: {:?}", back_up_path);
                        return false
                    }
                }
                println!("Existing backup file needs update: {:?}", back_up_path);
                true
            } else if config.overwrite_backup_if_existing_is_newer {
                println!("Existing backup file is newer than database, and config forces override: {:?}", back_up_path);
                true
            } else {
                println!("Existing backup file is newer than database, skipping {:?}", back_up_path);
                false
            }
        }
        None => {
            println!("Unknown backup file found. Checking if file is the same as source: {:?}", back_up_path);
            if prepped_backup.file_size == fs_file_size {
                let fs_hash = hash_file(back_up_path, &config.max_mebibytes_for_hash);
                if *prepped_backup.hash == fs_hash {
                    println!("Unknown backup is the same as source, inserting backup row: {:?}", back_up_path);
                    let backup_row = create_backup_row(prepped_backup, back_up_path);
                    insert_backup_row(backup_row);
                    return false
                }
            }
            println!("Unknown backup is not the same as source: {:?}", back_up_path);
            true
        }
    }
}
fn backup_file(prepped_backup: &PreppedBackup, backup_path: &PathBuf) {
    if !fs::exists(backup_path.parent().unwrap())
        .expect("Could not determine if backup path exists")
    {
        fs::create_dir_all(backup_path.parent().unwrap())
            .expect("Could not create backup directory");
    }
    println!(
        "Backing up file: {:?} to {:?}",
        &prepped_backup.source_file, backup_path
    );
    match fs::copy(
        &prepped_backup.source_file.as_path(),
        Path::new(backup_path),
    ) {
        Ok(_) => {
            let backup_row = create_backup_row(prepped_backup, backup_path);
            insert_backup_row(backup_row);
        }
        Err(e) => {
            println!("Error copying file {:?}: {:?}", backup_path, e);
        }
    }
}

fn create_backup_row(prepped_backup: &PreppedBackup, backup_path: &PathBuf) -> BackupRow {
    let last_modified = get_file_last_modified(backup_path);
    BackupRow {
        source_id: prepped_backup.db_id,
        file_name: prepped_backup.file_name.to_owned(),
        file_path: backup_path.parent().unwrap().to_str().unwrap().to_string(),
        last_modified,
    }
}

fn get_is_source_file_updated(
    source_candidate: &SourceRow,
    backup_candidate: &PathBuf,
    candidate_last_modified: &Duration,
    config: &Config,
) -> (bool, String) {
    let hash: String;
    let backup_file_size = get_file_size(backup_candidate);
    if source_candidate.last_modified.as_secs() < candidate_last_modified.as_secs() {
        if config.skip_source_hash_check_if_newer {
            hash = source_candidate.hash.to_owned();
            (true, hash)
        } else {
            hash = hash_file(&backup_candidate, &config.max_mebibytes_for_hash);
            if hash == source_candidate.hash && backup_file_size == source_candidate.file_size {
                update_source_last_modified(source_candidate.id, &candidate_last_modified);
                (false, hash)
            } else {
                update_source_row(source_candidate.id, &hash, &backup_file_size, &candidate_last_modified);
                (true, hash)
            }
        }
    } else {
        hash = source_candidate.hash.to_owned();
        (false, hash)
    }
}

fn get_possible_backups(
    file_name: &String,
    file_path: &String,
    shared_path: &PathBuf,
    destinations: &Vec<String>,
) -> Vec<PathBuf> {
    let mut possible_backup_paths = Vec::new();
    let relative_path = if shared_path.parent().is_some() {
        file_path.trim_start_matches(shared_path.parent().unwrap().to_str().unwrap())
    } else {
        file_path.trim_start_matches(shared_path.to_str().unwrap())
    };
    for destination in destinations {
        possible_backup_paths.push(
            Path::new(destination)
                .join(relative_path.trim_start_matches(MAIN_SEPARATOR))
                .join(&file_name),
        );
    }
    possible_backup_paths
}
