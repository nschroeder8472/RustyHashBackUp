use crate::models::backup_row::BackupRow;
use crate::models::source_row::SourceRow;
use rusqlite::{Connection, Error, OptionalExtension};
use std::time::Duration;

pub fn setup_database(string: &String) -> Connection {
    let db_conn = match Connection::open(string) {
        Ok(conn) => conn,
        Err(error) => {
            panic!("Failed to open or create database file {}", error);
        }
    };

    println!("Setting up database");
    let setup_queries = "BEGIN;

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

    db_conn
        .execute_batch(setup_queries)
        .expect("Failed to create database");
    println!("Database setup successfully");
    db_conn
}

pub fn select_source(
    db_conn: &Connection,
    source_file: &String,
    source_path: &String,
) -> rusqlite::Result<Option<SourceRow>> {
    let mut query = db_conn.prepare(
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
                last_modified: Duration::from_secs(row.get(4)?),
            })
        })
        .optional();
    source_row
}

pub fn insert_source_row(db_conn: &Connection, source_row: &SourceRow) -> Result<i32, Error> {
    println!(
        "Inserting source row for file: {} {}",
        &source_row.file_path, &source_row.file_name
    );
    match db_conn.execute(
        "INSERT INTO Source_Files (File_Name, File_Path, Hash, Last_Modified)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                File_Name=excluded.File_name,
                File_Path=excluded.File_path,
                Hash=excluded.Hash,
                Last_Modified=excluded.Last_Modified;",
        (
            &source_row.file_name,
            &source_row.file_path,
            &source_row.hash,
            &source_row.last_modified.as_secs(),
        ),
    ) {
        Ok(_) => db_conn.query_row(
            "SELECT ID
                FROM Source_Files
                WHERE File_Name=?1
                    AND File_Path=?2",
            (&source_row.file_name, &source_row.file_path),
            |row| row.get(0),
        ),
        Err(e) => Err(e),
    }
}

pub fn update_source_last_modified(db_conn: &Connection, row_id: i32, last_modified: &Duration) {
    db_conn
        .execute(
            "UPDATE Source_Files SET Last_Modified=?1 WHERE ID=?2",
            (last_modified.as_secs(), row_id),
        )
        .expect("Failed to update last modified for row");
}

pub fn update_source_hash(
    db_conn: &Connection,
    row_id: i32,
    hash: &String,
    last_modified: &Duration,
) {
    db_conn
        .execute(
            "UPDATE Source_Files SET Hash=?1, Last_Modified=?2 WHERE ID=?3",
            (hash, last_modified.as_secs(), row_id),
        )
        .expect("Failed to update hash for row");
}

pub fn insert_backup_row(db_conn: &Connection, backup_row: BackupRow) {
    match db_conn.execute(
        "INSERT INTO Backup_Files (Source_ID, File_Name, File_Path, Last_Modified)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                Source_ID=excluded.Source_ID,
                File_Name=excluded.File_name,
                File_Path=excluded.File_path,
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
