use std::path::PathBuf;
use walkdir::WalkDir;
use crate::models::config::BackupSource;

pub fn get_files_in_path(backup_sources: Vec<BackupSource>) -> Vec<PathBuf> {
    let mut files= Vec::new();
    for backup_source in backup_sources {
        for entry in WalkDir::new(backup_source.parent_directory)
            .max_depth(backup_source.max_depth)
            .follow_links(true)
            .contents_first(true)
            .into_iter()
            .filter_map(rusqlite::Result::ok) {
            if entry.file_type().is_dir() {
                continue;
            }
            files.push(entry.path().to_path_buf());
        };
    }
    files
}