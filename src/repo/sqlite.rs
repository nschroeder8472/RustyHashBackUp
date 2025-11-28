use crate::models::backed_up_file::BackedUpFile;
use crate::models::backup_row::BackupRow;
use crate::models::error::{BackupError, Result};
use crate::models::source_row::SourceRow;
use log::{debug, info};
use once_cell::sync::Lazy;
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{Error, OptionalExtension};
use std::sync::{Arc, RwLock};
use std::time::Duration;

type DbPool = Pool<SqliteConnectionManager>;

static DB_POOL: Lazy<RwLock<Option<Arc<DbPool>>>> = Lazy::new(|| RwLock::new(None));

pub fn set_db_pool(db_file: &str) -> Result<()> {
    if db_file.is_empty() {
        return Err(BackupError::DirectoryRead(
            "Database file path cannot be empty. Provide a valid path or use ':memory:' for in-memory database.".to_string()
        ));
    }

    info!("Initializing database connection pool: {}", db_file);

    let is_in_memory = db_file == ":memory:" || db_file.starts_with("file::memory:");
    let use_wal = !is_in_memory;

    let manager = SqliteConnectionManager::file(db_file).with_init(move |conn| {
        let mut pragmas = String::from(
            "PRAGMA busy_timeout = 5000;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA foreign_keys = ON;",
        );

        if use_wal {
            pragmas.push_str(" PRAGMA journal_mode = WAL;");
        }

        conn.execute_batch(&pragmas)
    });

    // Build connection pool
    // Pool size: num_physical_cpus + 7 for good mix of reads/writes
    let pool_size = num_cpus::get_physical() + 7;
    let pool = r2d2::Pool::builder()
        .max_size(pool_size as u32)
        .build(manager)
        .map_err(|e| {
            BackupError::DirectoryRead(format!("Failed to create database connection pool: {}", e))
        })?;

    info!("Database pool created with {} connections", pool_size);

    // Store pool in global
    let mut global_pool = DB_POOL.write().unwrap();
    *global_pool = Some(Arc::new(pool));

    Ok(())
}

fn get_connection() -> Result<r2d2::PooledConnection<SqliteConnectionManager>> {
    let pool_lock = DB_POOL.read().unwrap();
    let pool = pool_lock.as_ref().ok_or_else(|| {
        BackupError::DirectoryRead(
            "Database pool not initialized. Call set_db_pool() first.".to_string(),
        )
    })?;

    pool.get().map_err(|e| {
        BackupError::DirectoryRead(format!(
            "Failed to get database connection from pool: {}",
            e
        ))
    })
}

pub fn setup_database() -> Result<()> {
    info!("Initializing database schema");
    let setup_queries = "BEGIN;
    PRAGMA ENCODING = 'UTF-8';

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

    let conn = get_connection()?;
    conn.execute_batch(setup_queries)
        .map_err(|cause| BackupError::DatabaseQuery {
            operation: "create tables".to_string(),
            cause,
        })?;
    info!("Database schema initialized successfully");
    Ok(())
}

pub fn select_source(
    source_file: &str,
    source_path: &str,
) -> rusqlite::Result<Option<SourceRow>> {
    let conn = get_connection().map_err(|_| Error::InvalidParameterName("pool".to_string()))?;
    let mut query = conn.prepare(
        "SELECT *
                FROM Source_Files
                WHERE File_Name=?1
                    AND File_Path=?2",
    )?;
    query
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
        .optional()
}

pub fn select_backed_up_file(
    filename: &str,
    filepath: &str,
) -> rusqlite::Result<Option<BackedUpFile>> {
    let conn = get_connection().map_err(|_| Error::InvalidParameterName("pool".to_string()))?;
    let mut query = conn.prepare(
        "SELECT bf.File_Name, bf.File_Path, bf.Last_Modified, sf.Hash
            FROM Backup_Files bf
            LEFT JOIN Source_Files sf
            ON sf.ID = bf.Source_ID
            WHERE bf.File_Name=?1 AND bf.File_Path=?2",
    )?;
    query
        .query_row([filename, filepath], |row| {
            Ok(BackedUpFile {
                file_name: row.get(0)?,
                file_path: row.get(1)?,
                last_modified: Duration::from_secs(row.get(2)?),
                hash: row.get(3)?,
            })
        })
        .optional()
}

pub fn insert_source_row(source_row: &SourceRow) -> Result<i32> {
    let conn = get_connection()?;
    debug!(
        "Inserting source record: {}/{}",
        source_row.file_path, source_row.file_name
    );

    conn.query_row(
        "INSERT INTO Source_Files (File_Name, File_Path, Hash, File_Size, Last_Modified)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (File_Name, File_Path) DO UPDATE SET
             Hash = excluded.Hash,
             File_Size = excluded.File_Size,
             Last_Modified = excluded.Last_Modified
         RETURNING ID",
        (
            &source_row.file_name,
            &source_row.file_path,
            &source_row.hash,
            &source_row.file_size,
            source_row.last_modified.as_secs(),
        ),
        |row| row.get(0),
    )
    .map_err(|cause| BackupError::DatabaseInsert {
        table: "Source_Files".to_string(),
        file: format!("{}/{}", source_row.file_path, source_row.file_name),
        cause,
    })
}

pub fn update_source_last_modified(row_id: i32, last_modified: &Duration) -> Result<()> {
    let conn = get_connection()?;
    conn.execute(
        "UPDATE Source_Files SET Last_Modified=?1 WHERE ID=?2",
        (last_modified.as_secs(), row_id),
    )
    .map_err(|cause| BackupError::DatabaseUpdate {
        table: "Source_Files".to_string(),
        id: row_id as i64,
        cause,
    })?;
    Ok(())
}

pub fn update_source_row(
    row_id: i32,
    hash: &String,
    file_size: &u64,
    last_modified: &Duration,
) -> Result<()> {
    let conn = get_connection()?;
    conn.execute(
        "UPDATE Source_Files SET Hash=?1, File_Size=?2, Last_Modified=?3 WHERE ID=?4",
        (hash, file_size, last_modified.as_secs(), row_id),
    )
    .map_err(|cause| BackupError::DatabaseUpdate {
        table: "Source_Files".to_string(),
        id: row_id as i64,
        cause,
    })?;
    Ok(())
}

pub fn insert_backup_row(backup_row: BackupRow) -> Result<()> {
    let conn = get_connection()?;
    conn.execute(
        "INSERT INTO Backup_Files (Source_ID, File_Name, File_Path, Last_Modified)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT (File_Name, File_Path) DO UPDATE SET
                Source_ID=excluded.Source_ID,
                Last_Modified=excluded.Last_Modified;",
        (
            backup_row.source_id,
            &backup_row.file_name,
            &backup_row.file_path,
            backup_row.last_modified.as_secs(),
        ),
    )
    .map_err(|cause| BackupError::DatabaseInsert {
        table: "Backup_Files".to_string(),
        file: backup_row.file_name.clone(),
        cause,
    })?;
    debug!("Inserted backup record: {}", backup_row.file_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::Duration;

    // Helper to set up a fresh in-memory database for each test
    fn setup_test_db() {
        // Use SHARED in-memory database for testing
        // Regular ":memory:" creates separate databases per connection in a pool
        // Using "file::memory:?cache=shared" allows pool connections to share the same database
        set_db_pool("file::memory:?cache=shared").unwrap();
        setup_database().unwrap();
    }

    #[test]
    #[serial]
    fn test_setup_database_creates_schema() {
        setup_test_db();

        // Verify tables exist by attempting to query them
        let conn = get_connection().unwrap();
        let result = conn.execute("SELECT 1 FROM Source_Files WHERE 1=0", []);
        assert!(result.is_ok());

        let result = conn.execute("SELECT 1 FROM Backup_Files WHERE 1=0", []);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_insert_source_row_new_record() {
        setup_test_db();

        let source_row = SourceRow {
            id: 0,
            file_name: "test.txt".to_string(),
            file_path: "/test/path".to_string(),
            hash: "abc123".to_string(),
            file_size: 1024,
            last_modified: Duration::from_secs(1000),
        };

        let id = insert_source_row(&source_row).unwrap();
        assert!(id > 0);
    }

    #[test]
    #[serial]
    fn test_insert_source_row_upsert_on_conflict() {
        setup_test_db();

        let source_row = SourceRow {
            id: 0,
            file_name: "test.txt".to_string(),
            file_path: "/test/path".to_string(),
            hash: "abc123".to_string(),
            file_size: 1024,
            last_modified: Duration::from_secs(1000),
        };

        // Insert first time
        let id1 = insert_source_row(&source_row).unwrap();

        // Insert again with different hash - should upsert
        let updated_row = SourceRow {
            hash: "def456".to_string(),
            file_size: 2048,
            ..source_row
        };

        let id2 = insert_source_row(&updated_row).unwrap();

        // Should return same ID (upsert, not insert)
        assert_eq!(id1, id2);

        // Verify the hash was updated
        let retrieved = select_source(&"test.txt".to_string(), &"/test/path".to_string()).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().hash, "def456");
    }

    #[test]
    #[serial]
    fn test_select_source_returns_existing_record() {
        setup_test_db();

        let source_row = SourceRow {
            id: 0,
            file_name: "exists.txt".to_string(),
            file_path: "/exists".to_string(),
            hash: "hash123".to_string(),
            file_size: 512,
            last_modified: Duration::from_secs(2000),
        };

        insert_source_row(&source_row).unwrap();

        let result = select_source(&"exists.txt".to_string(), &"/exists".to_string()).unwrap();

        assert!(result.is_some());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.file_name, "exists.txt");
        assert_eq!(retrieved.file_path, "/exists");
        assert_eq!(retrieved.hash, "hash123");
        assert_eq!(retrieved.file_size, 512);
    }

    #[test]
    #[serial]
    fn test_select_source_returns_none_for_missing() {
        setup_test_db();

        let result =
            select_source(&"nonexistent.txt".to_string(), &"/nowhere".to_string()).unwrap();

        assert!(result.is_none());
    }

    #[test]
    #[serial]
    fn test_update_source_last_modified() {
        setup_test_db();

        let source_row = SourceRow {
            id: 0,
            file_name: "update_test.txt".to_string(),
            file_path: "/update".to_string(),
            hash: "original_hash".to_string(),
            file_size: 100,
            last_modified: Duration::from_secs(1000),
        };

        let id = insert_source_row(&source_row).unwrap();

        // Update last modified time
        let new_time = Duration::from_secs(2000);
        update_source_last_modified(id, &new_time).unwrap();

        // Verify update
        let retrieved = select_source(&"update_test.txt".to_string(), &"/update".to_string())
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.last_modified.as_secs(), 2000);
        // Hash should remain unchanged
        assert_eq!(retrieved.hash, "original_hash");
    }

    #[test]
    #[serial]
    fn test_update_source_row() {
        setup_test_db();

        let source_row = SourceRow {
            id: 0,
            file_name: "full_update.txt".to_string(),
            file_path: "/full_update".to_string(),
            hash: "old_hash".to_string(),
            file_size: 100,
            last_modified: Duration::from_secs(1000),
        };

        let id = insert_source_row(&source_row).unwrap();

        // Update hash, size, and time
        let new_hash = "new_hash".to_string();
        let new_size = 200u64;
        let new_time = Duration::from_secs(3000);

        update_source_row(id, &new_hash, &new_size, &new_time).unwrap();

        // Verify all fields updated
        let retrieved = select_source(&"full_update.txt".to_string(), &"/full_update".to_string())
            .unwrap()
            .unwrap();
        assert_eq!(retrieved.hash, "new_hash");
        assert_eq!(retrieved.file_size, 200);
        assert_eq!(retrieved.last_modified.as_secs(), 3000);
    }

    #[test]
    #[serial]
    fn test_insert_backup_row() {
        setup_test_db();

        // First insert a source row
        let source_row = SourceRow {
            id: 0,
            file_name: "source.txt".to_string(),
            file_path: "/source".to_string(),
            hash: "source_hash".to_string(),
            file_size: 500,
            last_modified: Duration::from_secs(1500),
        };

        let source_id = insert_source_row(&source_row).unwrap();

        // Now insert a backup row
        let backup_row = BackupRow {
            source_id,
            file_name: "source.txt".to_string(),
            file_path: "/backup/dest".to_string(),
            last_modified: Duration::from_secs(1500),
        };

        let result = insert_backup_row(backup_row);
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_select_backed_up_file_with_join() {
        setup_test_db();

        // Insert source
        let source_row = SourceRow {
            id: 0,
            file_name: "joined.txt".to_string(),
            file_path: "/source".to_string(),
            hash: "joined_hash".to_string(),
            file_size: 750,
            last_modified: Duration::from_secs(2500),
        };

        let source_id = insert_source_row(&source_row).unwrap();

        // Insert backup
        let backup_row = BackupRow {
            source_id,
            file_name: "joined.txt".to_string(),
            file_path: "/backup".to_string(),
            last_modified: Duration::from_secs(2500),
        };

        insert_backup_row(backup_row).unwrap();

        // Select backed up file (should join with source to get hash)
        let result =
            select_backed_up_file(&"joined.txt".to_string(), &"/backup".to_string()).unwrap();

        assert!(result.is_some());
        let backed_up = result.unwrap();
        assert_eq!(backed_up.file_name, "joined.txt");
        assert_eq!(backed_up.file_path, "/backup");
        assert_eq!(backed_up.hash, "joined_hash"); // Hash from joined Source_Files table
        assert_eq!(backed_up.last_modified.as_secs(), 2500);
    }

    #[test]
    #[serial]
    fn test_select_backed_up_file_returns_none_for_missing() {
        setup_test_db();

        let result =
            select_backed_up_file(&"missing.txt".to_string(), &"/nowhere".to_string()).unwrap();

        assert!(result.is_none());
    }
}
