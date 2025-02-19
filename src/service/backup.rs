use std::fs;
use std::path::{Path, PathBuf};
use rusqlite::Connection;
use crate::repo::sqlite::insert_source_row;
use crate::service::hash::hash_file;

pub fn backup_files (files: Vec<PathBuf>, destinations: Vec<String>, max_bytes: usize, db_conn: &Connection) {
    files.iter().for_each(|file| {
        let file_hash = hash_file(&file, max_bytes);
        insert_source_row(&db_conn, file_hash);
    })
}

fn copy_to_destinations(file: PathBuf, destinations: Vec<String>, db_conn: &Connection) {
    for destination in destinations {
        match fs::copy(file.as_path(), Path::new(&destination)) {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}