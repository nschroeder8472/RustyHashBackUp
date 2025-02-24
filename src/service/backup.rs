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
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::time::Duration;

pub fn backup_files(
    backup_candidates: HashMap<PathBuf, Vec<PathBuf>>,
    max_bytes: usize,
    db_conn: &Connection,
    config: &Config,
) {
    let mut prepped_backup_candidates: Vec<PreppedBackup> = Vec::new();
    for candidates in backup_candidates {
        let shared_path = candidates.0;
        for candidate in candidates.1 {
            let filename = candidate.file_name().unwrap().to_str().unwrap().to_string();
            let filepath = candidate.parent().unwrap().to_str().unwrap().to_string();
            let candidate_last_modified = get_file_last_modified(&candidate);
            let hash;
            let mut source_row;
            let source_row_option = match select_source(&db_conn, &filename, &filepath) {
                Ok(source_row) => source_row,
                Err(_) => {
                    panic!(
                        "Failed to select from database for {} {}",
                        filepath, filename
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
                    db_conn,
                )
            } else {
                hash = hash_file(&candidate, max_bytes);
                source_row = SourceRow {
                    id: 0,
                    file_name: filename.to_owned(),
                    file_path: filepath.to_owned(),
                    hash: hash.to_owned(),
                    last_modified: candidate_last_modified,
                };
                let id = match insert_source_row(db_conn, &source_row) {
                    Ok(id) => id,
                    Err(e) => {
                        panic!(
                            "Failed to insert source row for {} {}: {:?}",
                            filepath, filename, e
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
    backup_files_to_destinations(prepped_backup_candidates, db_conn, config);
}

fn backup_files_to_destinations(
    prepped_backups: Vec<PreppedBackup>,
    db_conn: &Connection,
    config: &Config,
) {
    prepped_backups.iter().for_each(|prepped_backup| {
        prepped_backup.backup_paths.iter().for_each(|backup_path| {
            if config.force_overwrite_backup {
                println!("Forced Override Backup");
                backup_file(prepped_backup, backup_path, db_conn)
            } else {
                if prepped_backup.updated {
                    //source updated, update all destinations
                    println!("Source File Updated, backing up");
                    backup_file(prepped_backup, backup_path, db_conn)
                } else {
                    if !fs::exists(backup_path).unwrap() {
                        //Source not updated, backup file does not exist
                        println!(
                            "Source File Not Updated, but backup file does not exist, backing up"
                        );
                        backup_file(prepped_backup, backup_path, db_conn);
                    } else {
                        //Source not updated, backup file does exist, confirm the file is the same
                        let filename = &backup_path
                            .file_name()
                            .unwrap()
                            .to_str()
                            .unwrap()
                            .to_string();
                        let filepath = &backup_path.parent().unwrap().to_str().unwrap().to_string();
                        // Check database for any previous backups
                        let backed_up_file_option =
                            match select_backed_up_file(db_conn, &filename, &filepath) {
                                Ok(backup_file_option) => {
                                    backup_file_option
                                },
                                Err(_) => panic!(
                                    "Failed to select from backup database for {} {}",
                                    filepath, filename
                                ),
                            };
                        if backed_up_file_option.is_some() {
                            // existing backup found, compare to file system
                            let backed_up_file = backed_up_file_option.unwrap();
                            let existing_backup_last_modified = get_file_last_modified(backup_path);
                            if existing_backup_last_modified.as_secs()
                                < backed_up_file.last_modified.as_secs()
                            {
                                // existing backed up file is older
                                let existing_backup_hash =
                                    hash_file(backup_path, config.max_mebibytes_for_hash);
                                if backed_up_file.hash != existing_backup_hash {
                                    // hashes don't match anymore
                                    println!(
                                        "Source and backup file differ for file {} {}",
                                        &filepath, &filename
                                    );
                                    backup_file(prepped_backup, backup_path, db_conn);
                                }
                            } else if backed_up_file.last_modified.as_secs() < existing_backup_last_modified.as_secs(){
                                //backed up file is somehow newer than expected
                                if config.overwrite_backup_if_existing_is_newer {
                                    println!(
                                        "Backup file is somehow newer than expected, backing up"
                                    );
                                    backup_file(prepped_backup, backup_path, db_conn);
                                } else {
                                    println!(
                                        "Backup file is somehow newer than expected, skipping"
                                    );
                                }
                            } else {
                                let existing_backup_hash =
                                    hash_file(backup_path, config.max_mebibytes_for_hash);
                                if backed_up_file.hash != existing_backup_hash {
                                    // hashes don't match anymore
                                    println!(
                                        "Source and backup file differ for file {} {}",
                                        &filepath, &filename
                                    );
                                    backup_file(prepped_backup, backup_path, db_conn);
                                }
                            }
                        } else {
                            println!("Backup file does not exist in db, backing up");
                            backup_file(prepped_backup, backup_path, db_conn);
                        }
                    }
                }
            }
        });
    })
}

fn backup_file(prepped_backup: &PreppedBackup, backup_path: &PathBuf, db_conn: &Connection) {
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
            let last_modified = get_file_last_modified(backup_path);
            let backup_row = BackupRow {
                source_id: prepped_backup.db_id,
                file_name: prepped_backup.file_name.to_owned(),
                file_path: backup_path.parent().unwrap().to_str().unwrap().to_string(),
                last_modified,
            };
            insert_backup_row(db_conn, backup_row);
        }
        Err(e) => {
            println!("Error copying file {:?}: {:?}", backup_path, e);
        }
    }
}

fn get_source_file_updated(
    source_row: &SourceRow,
    backup_candidate: &PathBuf,
    candidate_last_modified: &Duration,
    config: &Config,
    db_conn: &Connection,
) -> (bool, String) {
    let hash: String;
    if source_row.last_modified.as_secs() < candidate_last_modified.as_secs() {
        if config.skip_source_hash_check_if_newer {
            hash = source_row.hash.to_owned();
            (true, hash)
        } else {
            hash = hash_file(&backup_candidate, config.max_mebibytes_for_hash);
            if hash == source_row.hash {
                update_source_last_modified(db_conn, source_row.id, &candidate_last_modified);
                (false, hash)
            } else {
                update_source_hash(db_conn, source_row.id, &hash, &candidate_last_modified);
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
