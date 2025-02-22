use std::path::PathBuf;
use walkdir::WalkDir;

pub fn get_files_in_path(dir: &String, max_depth: &usize) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir)
        .max_depth(max_depth.to_owned())
        .follow_links(true)
        .contents_first(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_dir() {
            continue;
        }
        files.push(entry.path().to_path_buf());
    }
    files
}
