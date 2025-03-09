use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};
use walkdir::WalkDir;

pub fn get_files_in_path(dir: &String, skip_dirs: &Vec<String>, max_depth: &usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut dir_walk = WalkDir::new(dir)
        .max_depth(max_depth.to_owned())
        .follow_links(true)
        .into_iter();
    while let Some(entry) = dir_walk.next() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => panic!("Failed to read directory entry!"),
        };

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
    files
}

pub fn get_file_last_modified(file: &PathBuf) -> Duration {
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(e) => {
            panic!("Failed to get metadata for {}: {}", file.display(), e)
        }
    };

    match metadata.modified().unwrap().duration_since(UNIX_EPOCH) {
        Ok(d) => d,
        Err(e) => panic!("File last_modified is older than Epoch 0: {:?}", e),
    }
}
