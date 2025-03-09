use crate::models::backed_up_file::BackedUpFile;
use crate::models::backup_row::BackupRow;
use crate::models::source_row::SourceRow;
use once_cell::sync::Lazy;
use rusqlite::{Connection, Error, OptionalExtension};
use std::sync::Mutex;
use std::time::Duration;

static DB_CONN: Lazy<Mutex<Connection>> =
    Lazy::new(|| Mutex::new(Connection::open_in_memory().unwrap()));

pub fn set_db_connection(db_file: &String) {
    if db_file == "" {
        return;
    }

    let mut conn = DB_CONN.lock().unwrap();
    *conn = match Connection::open(db_file) {
        Ok(conn) => conn,
        Err(error) => {
            panic!("Failed to open or create database file {}", error);
        }
    };
}

pub fn setup_database() {
    println!("Setting up database");
    let setup_queries = "BEGIN;
    pragma ENCODING = 'UTF-8';

    CREATE TABLE IF NOT EXISTS Source_Files(
        ID            integer not null
            constraint Source_Files_ID
                primary key autoincrement,
        File_Name     TEXT    not null,
        File_Path     TEXT    not null,
        Hash          TEXT,
        File_Size     integer,
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
            constraint Backup_Files_Source_Files_ID_fk
                references Source_Files,
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

    let conn = DB_CONN.lock().unwrap();
    conn.execute_batch(setup_queries)
        .expect("Failed to create database");
    println!("Database setup successfully");
}

pub fn select_source(
    source_file: &String,
    source_path: &String,
) -> rusqlite::Result<Option<SourceRow>> {
    let conn = DB_CONN.lock().unwrap();
    let mut query = conn.prepare(
        "SELECT *
                FROM Source_Files
                WHERE File_Name=?1
                    AND File_Path=?2",
    )?;
    let source_row = query
        .query_row([source_file, source_path], |row| {
            Ok(SourceRow {
                id: row.get(0)?,
                file_name: row.get(1)?,
                file_path: row.get(2)?,
                hash: row.get(3)?,
                file_size: row.get(4)?,
                last_modified: Duration::from_secs(row.get(5)?),
            })
        })
        .optional();
    source_row
}

pub fn select_backed_up_file(
    filename: &String,
    filepath: &String,
) -> rusqlite::Result<Option<BackedUpFile>> {
    let conn = DB_CONN.lock().unwrap();
    let mut query = conn.prepare(
        "SELECT bf.File_Name, bf.File_Path, bf.Last_Modified, sf.Hash
            FROM Backup_Files bf
            LEFT JOIN Source_Files sf
            ON sf.ID = bf.Source_ID
            WHERE bf.File_Name=?1 AND bf.File_Path=?2",
    )?;
    let backed_up_file = query
        .query_row([filename, filepath], |row| {
            Ok(BackedUpFile {
                file_name: row.get(0)?,
                file_path: row.get(1)?,
                last_modified: Duration::from_secs(row.get(2)?),
                hash: row.get(3)?,
            })
        })
        .optional();
    backed_up_file
}

pub fn insert_source_row<'a>(source_row: &SourceRow) -> rusqlite::Result<i32, Error> {
    let conn = DB_CONN.lock().unwrap();
    println!(
        "Inserting source row for file: {} {}",
        &source_row.file_path, &source_row.file_name
    );
    match &conn.execute(
        "INSERT INTO Source_Files (File_Name, File_Path, Hash, File_Size, Last_Modified)
                VALUES (?1, ?2, ?3, ?4, ?5)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                Hash=excluded.Hash,
                File_Size=excluded.File_Size,   
                Last_Modified=excluded.Last_Modified;",
        (
            &source_row.file_name,
            &source_row.file_path,
            &source_row.hash,
            &source_row.file_size,
            &source_row.last_modified.as_secs(),
        ),
    ) {
        Ok(_) => conn.query_row(
            "SELECT ID
                FROM Source_Files
                WHERE File_Name=?1
                    AND File_Path=?2",
            (&source_row.file_name, &source_row.file_path),
            |row| row.get(0),
        ),
        Err(_) => Err(Error::QueryReturnedNoRows),
    }
}

pub fn update_source_last_modified(row_id: i32, last_modified: &Duration) {
    let conn = DB_CONN.lock().unwrap();
    conn.execute(
        "UPDATE Source_Files SET Last_Modified=?1 WHERE ID=?2",
        (last_modified.as_secs(), row_id),
    )
    .expect("Failed to update last modified for row");
}

pub fn update_source_row(row_id: i32, hash: &String, file_size: &u64, last_modified: &Duration) {
    let conn = DB_CONN.lock().unwrap();
    conn.execute(
        "UPDATE Source_Files SET Hash=?1, File_Size=?2 Last_Modified=?3 WHERE ID=?4",
        (hash, file_size, last_modified.as_secs(), row_id),
    )
    .expect("Failed to update source row");
}

pub fn insert_backup_row(backup_row: BackupRow) {
    let conn = DB_CONN.lock().unwrap();
    match conn.execute(
        "INSERT INTO Backup_Files (Source_ID, File_Name, File_Path, Last_Modified)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                Source_ID=excluded.Source_ID,
                Last_Modified=excluded.Last_Modified;",
        (
            backup_row.source_id,
            &backup_row.file_name,
            backup_row.file_path,
            backup_row.last_modified.as_secs(),
        ),
    ) {
        Ok(_) => {
            println!("Successfully inserted backup row {}", backup_row.file_name);
        }
        Err(e) => {
            println!(
                "Failed to insert backup row {}: {}",
                backup_row.file_name, e
            );
        }
    }
}
