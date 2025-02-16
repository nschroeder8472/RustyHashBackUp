use blake2::{Blake2b512, Digest};
use clap::{arg, Parser};
use std::io::{BufReader, Error, Read};
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};
use rayon::prelude::*;
use walkdir::WalkDir;
use rusqlite::{Connection, Result};

fn main() {
    let args = Cli::parse();
    let db_conn = Connection::open(args.database_file).unwrap();
    setup_database(&db_conn);

    let max_mebibytes = args.number_of_mebibytes * 1048576;

    let source_files = get_files_in_path(args.source_path);

    if source_files.is_empty() {
        println!("No files found");
        return;
    }

    let source_hash_data = hash_files(source_files, max_mebibytes);
    for file_hash in source_hash_data {
        insert_source_row(&db_conn, file_hash);
    }

    println!("Done");
}

fn setup_database(db_conn: &Connection) {
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

// fn get_existing_backup(conn: &Connection, dir: PathBuf) -> Vec<FileHash> {
//
// }

fn get_files_in_path(top_directory: String) -> Vec<PathBuf> {
    let mut files= Vec::new();
    for entry in WalkDir::new(top_directory)
        .follow_links(true)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok) {
        if entry.file_type().is_dir() {
            continue;
        }
        files.push(entry.path().to_path_buf());
    };
    files
}

fn hash_files(files: Vec<PathBuf>, max_mebibytes: usize) -> Vec<FileHash> {
    let result = Arc::new(Mutex::new(Vec::new()));

    files.par_iter().for_each(|file| {
        let file_path = file.parent().unwrap().to_str().unwrap();
        let file_name = file.file_name().unwrap().to_str().unwrap();
        println!("Hashing {}", file_name);
        let reader = BufReader::new(fs::File::open(file).unwrap());
        match hasher(reader, max_mebibytes) {
            Ok(hash) => {
                let file_hash = FileHash {
                    file_name: String::from(file_name),
                    file_path: String::from(file_path),
                    hash,
                    date: file.metadata().unwrap().modified().unwrap().duration_since(UNIX_EPOCH).expect("File date is older than Epoch 0")
                };
                result.lock().unwrap().push(file_hash);
            }
            Err(_) => {panic!("Failed to hash file");}
        }
    });

    Arc::try_unwrap(result).unwrap().into_inner().unwrap()
}

fn hasher<R: Read>(mut reader: BufReader<R>, max_mebibytes: usize) -> Result<String, Error> {
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
        if bytes_read > max_mebibytes {
            break;
        }
    }
    let output = hasher.finalize();
    let result = String::from_utf8_lossy(&output);
    Ok(result.parse().unwrap())
}

#[derive(Parser)]
struct Cli {
    #[arg(short = 's', long = "source")]
    source_path: String,
    #[arg(short = 'd', long = "destination")]
    destination_path: String,
    #[arg(short = 'r', long = "relative_path", default_value = "")]
    relative_path: String,
    #[arg(short = 'm', long = "max_size", default_value = "1")]
    number_of_mebibytes: usize,
    #[arg(short = 'b', long = "database_file", default_value = "/data/backup.db")]
    database_file: String
}

#[derive(Debug)]
struct FileHash {
    file_name: String,
    file_path: String,
    hash: String,
    date: Duration
}
