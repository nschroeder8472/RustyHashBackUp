use crate::models::error::{BackupError, Result};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

pub fn get_files_in_path(dir: &String, skip_dirs: &Vec<String>, max_depth: &usize) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut dir_walk = WalkDir::new(dir)
        .max_depth(max_depth.to_owned())
        .follow_links(true)
        .into_iter();

    while let Some(entry) = dir_walk.next() {
        let entry = entry.map_err(|e| {
            BackupError::DirectoryRead(format!("Failed to read directory entry: {}", e))
        })?;

        if entry.file_type().is_dir()
            && skip_dirs.contains(&entry.file_name().to_string_lossy().to_string())
        {
            dir_walk.skip_current_dir();
            continue;
        } else if entry.file_type().is_dir() {
            continue;
        }
        files.push(entry.path().to_path_buf());
    }
    Ok(files)
}

pub fn get_file_size(file: &PathBuf) -> Result<u64> {
    let metadata = file.metadata().map_err(|cause| {
        BackupError::MetadataError {
            path: file.clone(),
            cause,
        }
    })?;
    Ok(metadata.len())
}

pub fn get_file_last_modified(file: &PathBuf) -> Result<Duration> {
    let metadata = file.metadata().map_err(|cause| {
        BackupError::MetadataError {
            path: file.clone(),
            cause,
        }
    })?;

    let modified = metadata.modified().map_err(|cause| {
        BackupError::MetadataError {
            path: file.clone(),
            cause,
        }
    })?;

    modified.duration_since(UNIX_EPOCH).map_err(|cause| {
        BackupError::ModificationTimeError {
            path: file.clone(),
            cause,
        }
    })
}
