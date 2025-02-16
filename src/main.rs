use blake2::{Blake2b512, Digest};
use clap::{arg, Parser};
use std::io::{BufReader, Error, Read};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::{WalkDir};
use rusqlite::{Connection, Result};
use serde::Deserialize;

fn main() {
    let args = Cli::parse();
    let config: Config = setup_config(args.config_file);
    let db_conn = setup_database(config.database_file);
    let max_bytes = config.max_mebibytes_for_hash * 1048576;
    let source_files = get_files_in_path(config.backup_sources);

    if source_files.is_empty() {
        println!("No files found");
        return;
    }

    let source_hash_data = hash_files(source_files, max_bytes);
    for file_hash in source_hash_data {
        insert_source_row(&db_conn, file_hash);
    }

    println!("Done");
}

fn setup_config(config_file: String) -> Config {
    let config_file = PathBuf::from(config_file);
    let config_str = match fs::read_to_string(config_file) {
        Ok(file) => {file}
        Err(_) => {panic!("Failed to read config file");}
    };

    match serde_json::from_str(&config_str) {
        Ok(config) => config,
        Err(_) => {panic!("Failed to parse config file");}
    }
}

fn setup_database(string: String) -> Connection {
    let db_conn = match Connection::open(string) {
        Ok(conn) => {conn}
        Err(error) => {panic!("Failed to open or create database file {}", error);}
    };

    println!("Setting up database");
    let setup_queries =
    "BEGIN;

    CREATE TABLE IF NOT EXISTS Source_Files(
        ID            integer not null
            constraint Source_Files_ID
                primary key autoincrement,
        File_Name     TEXT    not null,
        File_Path     TEXT    not null,
        Hash          TEXT,
        Last_Modified integer,
        constraint Source_Files_File_Key
            unique (File_Name, File_Path));

    CREATE INDEX IF NOT EXISTS Source_Files_File_Name_index
            on Source_Files (File_Name);

    CREATE TABLE IF NOT EXISTS Backup_Files(
        ID            integer not null
            constraint Backup_Files_ID_pk
                primary key autoincrement,
        Source_ID     integer not null
            constraint Backup_Files_Backup_Files_ID_fk
                references Backup_Files,
        File_Name     TEXT    not null,
        File_Path     TEXT    not null,
        Last_Modified integer,
        constraint Backup_Files_pk
            unique (File_Name, File_Path));

    CREATE INDEX IF NOT EXISTS Backup_Files_File_Name_File_Path_index
            on Backup_Files (File_Name, File_Path);

    CREATE INDEX IF NOT EXISTS Backup_Files_Source_ID_index
            on Backup_Files (Source_ID);

    COMMIT;";

    db_conn.execute_batch(setup_queries).expect("Failed to create database");
    println!("Database setup successfully");
    db_conn
}

fn insert_source_row(db_conn: &Connection, source_row: FileHash) {
    println!("Inserting source row for file: {}\\{}", source_row.file_path, source_row.file_name);
    match db_conn.execute(
        "INSERT INTO Source_Files (File_Name, File_Path, Hash, Last_Modified)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                File_Name=excluded.File_name,
                File_Path=excluded.File_path,
                Hash=excluded.Hash,
                Last_Modified=excluded.Last_Modified;",
        (source_row.file_name, source_row.file_path, source_row.hash, source_row.date.as_secs())
    ) {
        Ok(_) => (),
        Err(error) => {println!("Error inserting new source row: {:?}", error); }
    }
}

fn get_files_in_path(backup_sources: Vec<BackupSource>) -> Vec<PathBuf> {
    let mut files= Vec::new();
    for backup_source in backup_sources {
        for entry in WalkDir::new(backup_source.parent_directory)
            .max_depth(backup_source.max_depth)
            .follow_links(true)
            .contents_first(true)
            .into_iter()
            .filter_map(Result::ok) {
            if entry.file_type().is_dir() {
                continue;
            }
            files.push(entry.path().to_path_buf());
        };
    }
    files
}

fn hash_files(files: Vec<PathBuf>, max_bytes: usize) -> Vec<FileHash> {
    let mut result = Vec::new();
    files.iter().for_each(|file| {
        let file_path = file.parent().unwrap().to_str().unwrap();
        let file_name = file.file_name().unwrap().to_str().unwrap();
        println!("Hashing {}", file_name);
        let reader = BufReader::new(fs::File::open(file).unwrap());
        match hasher(reader, max_bytes) {
            Ok(hash) => {
                let file_hash = FileHash {
                    file_name: String::from(file_name),
                    file_path: String::from(file_path),
                    hash,
                    date: file.metadata().unwrap().modified().unwrap().duration_since(UNIX_EPOCH).expect("File date is older than Epoch 0")
                };
                result.push(file_hash);
            }
            Err(_) => {panic!("Failed to hash file");}
        }
    });

    result
}

fn hasher<R: Read>(mut reader: BufReader<R>, max_bytes: usize) -> Result<String, Error> {
    let mut hasher = Blake2b512::new();
    let mut buffer = [0; 8192];
    let mut bytes_read = 0;

    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes_read += count;
        hasher.update(&buffer[..count]);
        if bytes_read > max_bytes {
            break;
        }
    }
    let output = hasher.finalize();
    let result = String::from_utf8_lossy(&output);
    Ok(result.parse().unwrap())
}

#[derive(Parser)]
struct Cli {
    #[arg(short = 'c', long = "config", default_value = "/data/config.json")]
    config_file: String
}

#[derive(Debug, Deserialize)]
struct Config {
    database_file: String,
    max_mebibytes_for_hash: usize,
    backup_sources: Vec<BackupSource>,
    backup_destinations: Vec<String>
}

#[derive(Debug, Deserialize)]
struct BackupSource {
    parent_directory: String,
    #[serde(default = "usize_max")]
    max_depth: usize

}

fn usize_max () -> usize {
    usize::MAX
}

#[derive(Debug)]
struct FileHash {
    file_name: String,
    file_path: String,
    hash: String,
    date: Duration
}
