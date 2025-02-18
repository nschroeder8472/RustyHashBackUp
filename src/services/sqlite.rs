use rusqlite::Connection;
use crate::models::file_hash::FileHash;

pub fn setup_database(string: String) -> Connection {
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

pub fn insert_source_row(db_conn: &Connection, source_row: FileHash) {
    println!("Inserting source row for file: {} {}", source_row.file_path, source_row.file_name);
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