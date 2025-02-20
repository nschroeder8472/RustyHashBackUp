mod models;
mod repo;
mod service;
mod utils;

use crate::models::config::setup_config;
use crate::service::backup::backup_files;
use crate::utils::directory::get_files_in_path;
use clap::{arg, Parser};
use models::config::Config;
use repo::sqlite::setup_database;

#[derive(Parser)]
struct Cli {
    #[arg(short = 'c', long = "config", default_value = "/data/config.json")]
    config_file: String,
}

fn main() {
    let args = Cli::parse();
    let config: Config = setup_config(args.config_file);
    let db_conn = setup_database(config.database_file);
    let max_bytes = config.max_mebibytes_for_hash * 1048576;
    let source_files = get_files_in_path(config.backup_sources);

    if source_files.is_empty() {
        println!("No files found");
        return;
    }

    backup_files(
        source_files,
        config.backup_destinations,
        max_bytes,
        &db_conn,
    );

    println!("Done");
}
