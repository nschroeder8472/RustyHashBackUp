use crate::models::backup_row::BackupRow;
use crate::models::config::Config;
use crate::models::prepped_backup::PreppedBackup;
use crate::models::source_row::SourceRow;
use crate::repo::sqlite::{
    insert_backup_row, insert_source_row, select_backed_up_file, select_source, update_source_hash,
    update_source_last_modified,
};
use crate::service::hash::hash_file;
use crate::utils::directory::get_file_last_modified;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::time::Duration;

pub fn backup_files(backup_candidates: HashMap<PathBuf, Vec<PathBuf>>, config: &Config) {
    let mut prepped_backup_candidates: Vec<PreppedBackup> = Vec::new();
    for candidates in backup_candidates {
        let shared_path = candidates.0;
        for candidate in candidates.1 {
            let filename = candidate
                .file_name()
                .unwrap()
                .to_os_string()
                .to_string_lossy()
                .to_string();
            let filepath = candidate.parent().unwrap().to_string_lossy().to_string();
            let candidate_last_modified = get_file_last_modified(&candidate);
            let hash;
            let mut source_row;
            let source_row_option = match select_source(&filename, &filepath) {
                Ok(source_row) => source_row,
                Err(e) => {
                    panic!(
                        "Failed to select from database for {}{}{}: {:?}",
                        filepath, MAIN_SEPARATOR, filename, e
                    );
                }
            };
            let updated: bool;
            if source_row_option.is_some() {
                source_row = source_row_option.unwrap();
                (updated, hash) = get_source_file_updated(
                    &source_row,
                    &candidate,
                    &candidate_last_modified,
                    &config,
                )
            } else {
                hash = hash_file(&candidate, &config.max_mebibytes_for_hash);
                source_row = SourceRow {
                    id: 0,
                    file_name: filename.to_owned(),
                    file_path: filepath.to_owned(),
                    hash: hash.to_owned(),
                    last_modified: candidate_last_modified,
                };
                let id = match insert_source_row(&source_row) {
                    Ok(id) => id,
                    Err(e) => {
                        panic!(
                            "Failed to insert source row for {}{}{}: {:?}",
                            filepath, MAIN_SEPARATOR, filename, e
                        );
                    }
                };
                source_row.id = id;
                updated = true
            };

            let backup_paths = get_possible_backups(
                &filename,
                &filepath,
                &shared_path,
                &config.backup_destinations,
            );

            prepped_backup_candidates.push(PreppedBackup {
                db_id: source_row.id,
                source_file: candidate,
                file_name: filename,
                backup_paths: backup_paths,
                hash: hash.to_owned(),
                source_last_modified_date: candidate_last_modified,
                updated: updated,
            })
        }
    }
    backup_files_to_destinations(prepped_backup_candidates, config);
}

fn backup_files_to_destinations(prepped_backups: Vec<PreppedBackup>, config: &Config) {
    prepped_backups.into_par_iter().for_each(|prepped_backup| {
        prepped_backup.backup_paths.iter().for_each(|backup_path| {
            if config.force_overwrite_backup {
                println!("Forced Override Backup");
                backup_file(&prepped_backup, backup_path)
            } else {
                if prepped_backup.updated {
                    //source updated, update all destinations
                    println!("Source File {:?} Updated, backing up",
                        prepped_backup.source_file);
                    backup_file(&prepped_backup, backup_path)
                } else {
                    if !fs::exists(backup_path).unwrap() {
                        //Source not updated, backup file does not exist
                        println!(
                            "Source File {:?} Not Updated, but backup file does not exist, backing up",
                                prepped_backup.source_file);
                        backup_file(&prepped_backup, backup_path);
                    } else {
                        //Source not updated, backup file does exist, confirm the file is the same
                        let back_up_filename = &backup_path
                            .file_name()
                            .unwrap()
                            .to_os_string()
                            .to_string_lossy()
                            .to_string();
                        let back_up_filepath = &backup_path.parent().unwrap().to_string_lossy().to_string();
                        // Check database for any previous backups
                        let dbase_backup_file_option =
                            match select_backed_up_file(&back_up_filename, &back_up_filepath) {
                                Ok(backup_file_option) => backup_file_option,
                                Err(e) => panic!(
                                    "Failed to select from backup database for {}{}{}: {:?}",
                                    back_up_filepath, MAIN_SEPARATOR, back_up_filename, e
                                ),
                            };
                        if dbase_backup_file_option.is_some() {
                            // existing backup found, compare to file system
                            let backed_up_file = dbase_backup_file_option.unwrap();
                            println!("Existing Backup Found: {}{}{}",
                                     backed_up_file.file_path, MAIN_SEPARATOR, backed_up_file.file_name);
                            let existing_backup_last_modified = get_file_last_modified(backup_path);
                            if existing_backup_last_modified.as_secs()
                                < backed_up_file.last_modified.as_secs()
                            {
                                println!("Existing back up is older than expected: {}{}{}",
                                         backed_up_file.file_path, MAIN_SEPARATOR, backed_up_file.file_name);
                                // existing backed up file is older
                                let existing_backup_hash =
                                    hash_file(backup_path, &config.max_mebibytes_for_hash);
                                if backed_up_file.hash != existing_backup_hash {
                                    // hashes don't match anymore
                                    println!(
                                        "Source and backup file differ for file: {}{}{}",
                                        &back_up_filepath, MAIN_SEPARATOR, &back_up_filename
                                    );
                                    backup_file(&prepped_backup, backup_path);
                                }
                            } else if backed_up_file.last_modified.as_secs()
                                < existing_backup_last_modified.as_secs()
                            {
                                //backed up file is somehow newer than expected
                                if config.overwrite_backup_if_existing_is_newer {
                                    println!(
                                        "Backup file is somehow newer than expected, backing up: {}{}{}",
                                        backed_up_file.file_path, MAIN_SEPARATOR, backed_up_file.file_name);
                                    backup_file(&prepped_backup, backup_path);
                                } else {
                                    println!(
                                        "Backup file is somehow newer than expected, skipping: {}{}{}",
                                        backed_up_file.file_path, MAIN_SEPARATOR, backed_up_file.file_name);
                                }
                            } else {
                                println!("{}{}{} File last modified is the same as expected, checking hashes",
                                         backed_up_file.file_path, MAIN_SEPARATOR, backed_up_file.file_name);
                                let existing_backup_hash =
                                    hash_file(backup_path, &config.max_mebibytes_for_hash);
                                if backed_up_file.hash != existing_backup_hash {
                                    // hashes don't match anymore
                                    println!(
                                        "Source and backup file differ for file: {}{}{}",
                                        &back_up_filepath, MAIN_SEPARATOR, &back_up_filename
                                    );
                                    backup_file(&prepped_backup, backup_path);
                                } else {
                                    println!("Hashes match, skipping: {}{}{}",
                                        &back_up_filepath, MAIN_SEPARATOR, &back_up_filename
                                    );
                                }
                            }
                        } else {
                            println!("{}{}{} Backup file does not exist in db",
                                     &back_up_filepath, MAIN_SEPARATOR, &back_up_filename);
                            if fs::exists(backup_path).unwrap() {
                                let existing_backup_hash =
                                    hash_file(backup_path, &config.max_mebibytes_for_hash);
                                if prepped_backup.hash == existing_backup_hash {
                                    println!("{}{}{} Backup file exists, and hash matches source file, inserting backup row",
                                             &back_up_filepath, MAIN_SEPARATOR, &back_up_filename);
                                    let backup_row = create_backup_row(&prepped_backup, backup_path);
                                    insert_backup_row(backup_row)
                                } else {
                                    println!("{}{}{} Backup file exists, but hashes are different, backing up",
                                             &back_up_filepath, MAIN_SEPARATOR, &back_up_filename);
                                    backup_file(&prepped_backup, backup_path);
                                }
                            } else {
                                println!("Backing up: {}{}{}",
                                    &back_up_filepath, MAIN_SEPARATOR, &back_up_filename);
                                backup_file(&prepped_backup, backup_path);
                            }
                        }
                    }
                }
            }
        });
    })
}

fn backup_file(prepped_backup: &PreppedBackup, backup_path: &PathBuf) {
    if !fs::exists(backup_path.parent().unwrap())
        .expect("Could not determine if backup path exists")
    {
        fs::create_dir_all(backup_path.parent().unwrap())
            .expect("Could not create backup directory");
    }
    println!(
        "Copying {:?} to {:?}",
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

fn get_source_file_updated(
    source_row: &SourceRow,
    backup_candidate: &PathBuf,
    candidate_last_modified: &Duration,
    config: &Config,
) -> (bool, String) {
    let hash: String;
    if source_row.last_modified.as_secs() < candidate_last_modified.as_secs() {
        if config.skip_source_hash_check_if_newer {
            hash = source_row.hash.to_owned();
            (true, hash)
        } else {
            hash = hash_file(&backup_candidate, &config.max_mebibytes_for_hash);
            if hash == source_row.hash {
                update_source_last_modified(source_row.id, &candidate_last_modified);
                (false, hash)
            } else {
                update_source_hash(source_row.id, &hash, &candidate_last_modified);
                (true, hash)
            }
        }
    } else {
        hash = source_row.hash.to_owned();
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
