use crate::models::error::{BackupError, Result};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

pub fn get_files_in_path(
    dir: &String,
    skip_dirs: &Vec<String>,
    max_depth: &usize,
) -> Result<Vec<PathBuf>> {
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
    let metadata = file
        .metadata()
        .map_err(|cause| BackupError::MetadataError {
            path: file.clone(),
            cause,
        })?;
    Ok(metadata.len())
}

pub fn get_file_last_modified(file: &PathBuf) -> Result<Duration> {
    let metadata = file
        .metadata()
        .map_err(|cause| BackupError::MetadataError {
            path: file.clone(),
            cause,
        })?;

    let modified = metadata
        .modified()
        .map_err(|cause| BackupError::MetadataError {
            path: file.clone(),
            cause,
        })?;

    modified
        .duration_since(UNIX_EPOCH)
        .map_err(|cause| BackupError::ModificationTimeError {
            path: file.clone(),
            cause,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::{NamedTempFile, TempDir};

    #[test]
    fn test_get_files_in_flat_directory() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap().to_string();

        // Create a few files
        fs::File::create(temp_dir.path().join("file1.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file2.txt")).unwrap();
        fs::File::create(temp_dir.path().join("file3.log")).unwrap();

        let files = get_files_in_path(&dir_path, &vec![], &usize::MAX).unwrap();

        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_get_files_respects_max_depth() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap().to_string();

        // Create files at different depths
        fs::File::create(temp_dir.path().join("root.txt")).unwrap();

        let sub_dir1 = temp_dir.path().join("level1");
        fs::create_dir(&sub_dir1).unwrap();
        fs::File::create(sub_dir1.join("level1.txt")).unwrap();

        let sub_dir2 = sub_dir1.join("level2");
        fs::create_dir(&sub_dir2).unwrap();
        fs::File::create(sub_dir2.join("level2.txt")).unwrap();

        // max_depth = 1 should only find root.txt
        let files_depth1 = get_files_in_path(&dir_path, &vec![], &1).unwrap();
        assert_eq!(files_depth1.len(), 1);

        // max_depth = 2 should find root.txt and level1.txt
        let files_depth2 = get_files_in_path(&dir_path, &vec![], &2).unwrap();
        assert_eq!(files_depth2.len(), 2);

        // max_depth = 3 should find all three files
        let files_depth3 = get_files_in_path(&dir_path, &vec![], &3).unwrap();
        assert_eq!(files_depth3.len(), 3);
    }

    #[test]
    fn test_get_files_skips_directories() {
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path().to_str().unwrap().to_string();

        // Create files and directories
        fs::File::create(temp_dir.path().join("file.txt")).unwrap();

        let skip_dir = temp_dir.path().join("skip_me");
        fs::create_dir(&skip_dir).unwrap();
        fs::File::create(skip_dir.join("skipped.txt")).unwrap();

        let keep_dir = temp_dir.path().join("keep_me");
        fs::create_dir(&keep_dir).unwrap();
        fs::File::create(keep_dir.join("kept.txt")).unwrap();

        let files =
            get_files_in_path(&dir_path, &vec!["skip_me".to_string()], &usize::MAX).unwrap();

        // Should find file.txt and keep_me/kept.txt, but not skip_me/skipped.txt
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.file_name().unwrap() == "file.txt"));
        assert!(files.iter().any(|f| f.file_name().unwrap() == "kept.txt"));
        assert!(!files
            .iter()
            .any(|f| f.file_name().unwrap() == "skipped.txt"));
    }

    #[test]
    fn test_get_files_error_on_nonexistent_directory() {
        let result = get_files_in_path(&"/this/does/not/exist".to_string(), &vec![], &usize::MAX);

        assert!(result.is_err());
    }

    #[test]
    fn test_get_file_size() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let content = b"Hello, this is test content!";
        temp_file.write_all(content).unwrap();
        temp_file.flush().unwrap();

        let size = get_file_size(&temp_file.path().to_path_buf()).unwrap();

        assert_eq!(size, content.len() as u64);
    }

    #[test]
    fn test_get_file_last_modified() {
        let temp_file = NamedTempFile::new().unwrap();

        let last_modified = get_file_last_modified(&temp_file.path().to_path_buf()).unwrap();

        // Should return a valid duration
        assert!(last_modified.as_secs() > 0);
    }

    #[test]
    fn test_get_file_size_error_on_missing_file() {
        let nonexistent = PathBuf::from("/this/file/does/not/exist.txt");

        let result = get_file_size(&nonexistent);

        assert!(result.is_err());
        match result {
            Err(BackupError::MetadataError { path, .. }) => {
                assert_eq!(path, nonexistent);
            }
            _ => panic!("Expected MetadataError"),
        }
    }
}
