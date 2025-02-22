use crate::models::backup_row::BackupRow;
use crate::models::config::Config;
use crate::models::prepped_backup::PreppedBackup;
use crate::models::source_row::SourceRow;
use crate::repo::sqlite::{
    insert_backup_row, insert_source_row, select_source, update_source_hash,
    update_source_last_modified,
};
use crate::service::hash::hash_file;
use rusqlite::Connection;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf, MAIN_SEPARATOR};
use std::time::UNIX_EPOCH;

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
            let candidate_last_modified = match candidate
                .metadata()
                .unwrap()
                .modified()
                .unwrap()
                .duration_since(UNIX_EPOCH)
            {
                Ok(d) => d,
                Err(e) => panic!("SystemTime before UNIX EPOCH!: {:?}", e),
            };
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
            let mut updated: bool = false;
            if source_row_option.is_some() {
                source_row = source_row_option.unwrap();
                updated = if source_row.last_modified.as_secs() < candidate_last_modified.as_secs()
                {
                    if config.skip_source_hash_check_if_newer {
                        hash = source_row.hash;
                        true
                    } else {
                        hash = hash_file(&candidate, max_bytes).hash;
                        if hash == source_row.hash {
                            println!("Source hash has not changed!");
                            update_source_last_modified(
                                db_conn,
                                source_row.id,
                                &candidate_last_modified,
                            );
                            false
                        } else {
                            println!("Source hash changed!");
                            update_source_hash(
                                db_conn,
                                source_row.id,
                                &hash,
                                &candidate_last_modified,
                            );
                            true
                        }
                    }
                } else {
                    hash = source_row.hash;
                    false
                }
            } else {
                hash = hash_file(&candidate, max_bytes).hash;
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
                file_name: source_row.file_name,
                source_file_path: source_row.file_path,
                backup_paths: backup_paths,
                hash: hash.to_owned(),
                source_last_modified_date: source_row.last_modified,
                updated: updated,
            })
        }
    }
    prepped_backup_candidates
        .iter()
        .for_each(|candidate| println!("{:?}", candidate.backup_paths));
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

fn copy_to_destinations(
    file: &PathBuf,
    source_row: SourceRow,
    source_id: i32,
    destinations: &Vec<String>,
    db_conn: &Connection,
) {
    for destination in destinations {
        let destination_file = Path::new(destination).join(&source_row.file_name);
        println!("Copying {} to {}", destination, destination_file.display());
        match fs::copy(file.as_path(), Path::new(&destination_file)) {
            Ok(_) => {
                let last_modified = destination_file
                    .metadata()
                    .unwrap()
                    .modified()
                    .unwrap()
                    .duration_since(UNIX_EPOCH)
                    .expect("File last_modified is older than Epoch 0");
                let backup_row = BackupRow {
                    source_id,
                    file_name: String::from(&source_row.file_name),
                    file_path: destination.to_owned(),
                    last_modified,
                };
                insert_backup_row(db_conn, backup_row);
            }
            Err(_) => {
                println!("Error copying file: {:?}", &destination_file);
            }
        }
    }
}
