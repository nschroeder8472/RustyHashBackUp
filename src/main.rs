mod models;
mod repo;
mod service;
mod utils;

use crate::models::config::{setup_config, BackupSource};
use crate::service::backup::backup_files;
use crate::utils::directory::get_files_in_path;
use clap::{arg, Parser};
use models::config::Config;
use repo::sqlite::setup_database;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'c', long = "config", default_value = "/data/config.json")]
    config_file: String,
}

fn main() {
    let args = Cli::parse();
    let config: Config = setup_config(args.config_file);
    println!("Config: {:?}", &config);
    let db_conn = setup_database(&config.database_file);
    let max_bytes = &config.max_mebibytes_for_hash * 1048576;
    let backup_candidates = get_source_files(&config.backup_sources);

    if backup_candidates.is_empty() {
        println!("No files found");
        return;
    }

    backup_files(backup_candidates, max_bytes, &db_conn, &config);

    println!("Done");
}

fn get_source_files(backup_sources: &Vec<BackupSource>) -> HashMap<PathBuf, Vec<PathBuf>> {
    let mut result_map = HashMap::<PathBuf, Vec<PathBuf>>::new();
    backup_sources
        .iter()
        .map(|s| {
            (
                PathBuf::from(&s.parent_directory),
                get_files_in_path(&s.parent_directory, &s.max_depth),
            )
        })
        .filter(|(_, v)| !v.is_empty())
        .for_each(|(path, files)| {
            result_map.insert(path, files);
        });
    result_map
}
