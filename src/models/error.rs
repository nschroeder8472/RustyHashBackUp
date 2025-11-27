use std::io;
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BackupError {
    #[error("Failed to read config file '{path}': {cause}")]
    ConfigRead { path: PathBuf, cause: io::Error },

    #[error("Failed to parse config file '{path}': {cause}")]
    ConfigParse {
        path: PathBuf,
        cause: serde_json::Error,
    },

    #[error("Failed to open or create database file '{path}': {cause}")]
    DatabaseConnection {
        path: String,
        cause: rusqlite::Error,
    },

    #[error("Database query failed for '{operation}': {cause}")]
    DatabaseQuery {
        operation: String,
        cause: rusqlite::Error,
    },

    #[error("Failed to update {table} for ID {id}: {cause}")]
    DatabaseUpdate {
        table: String,
        id: i64,
        cause: rusqlite::Error,
    },

    #[error("Failed to insert into {table} for {file}: {cause}")]
    DatabaseInsert {
        table: String,
        file: String,
        cause: rusqlite::Error,
    },

    #[error("Failed to hash file '{path}': {cause}")]
    HashError { path: PathBuf, cause: io::Error },

    #[error("Failed to read directory entry: {0}")]
    DirectoryRead(String),

    #[error("Failed to get metadata for '{path}': {cause}")]
    MetadataError { path: PathBuf, cause: io::Error },

    #[error("File modification time is invalid for '{path}': {cause}")]
    ModificationTimeError {
        path: PathBuf,
        cause: std::time::SystemTimeError,
    },

    #[error("Failed to copy file from '{from}' to '{to}': {cause}")]
    FileCopy {
        from: PathBuf,
        to: PathBuf,
        cause: io::Error,
    },

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Failed to build thread pool: {0}")]
    ThreadPool(#[from] rayon::ThreadPoolBuildError),
}

pub type Result<T> = std::result::Result<T, BackupError>;
