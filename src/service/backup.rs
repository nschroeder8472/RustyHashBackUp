use crate::models::backup_row::BackupRow;
use crate::repo::sqlite::{insert_backup_row, insert_source_row};
use crate::service::hash::hash_file;
use rusqlite::Connection;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub fn backup_files(
    files: Vec<PathBuf>,
    destinations: Vec<String>,
    max_bytes: usize,
    db_conn: &Connection,
) {
    files.iter().for_each(|file| {
        let file_hash = hash_file(&file, max_bytes);
        match insert_source_row(&db_conn, file_hash) {
            Ok(source_id) => copy_to_destinations(file, &destinations, source_id, db_conn),
            Err(error) => {
                println!("Error inserting new source row: {:?}", error);
            }
        }
    })
}

fn copy_to_destinations(
    file: &PathBuf,
    destinations: &Vec<String>,
    source_id: i32,
    db_conn: &Connection,
) {
    for destination in destinations {
        let file_name = file.file_name().unwrap().to_str().unwrap();
        let destination_file = Path::new(destination).join(file_name);
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
                    file_name: String::from(file_name),
                    file_path: destination.clone(),
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
