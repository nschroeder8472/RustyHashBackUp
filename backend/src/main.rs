mod api_routes;
mod api_state;
mod models;
mod repo;
mod service;
mod utils;
mod web_routes;

use crate::models::config::{setup_config, BackupSource};
use crate::models::dry_run_mode::DryRunMode;
use crate::repo::sqlite::set_db_pool;
use crate::service::backup::backup_files;
use crate::utils::directory::get_files_in_path;
use crate::utils::progress::{create_progress_bar, create_progress_bar_with_bytes, create_spinner};
use anyhow::{Context, Result};
use clap::Parser;
use indicatif::MultiProgress;
use log::{debug, info, warn};
use models::config::Config;
use repo::sqlite::setup_database;
use std::collections::HashMap;
use std::path::PathBuf;

#[macro_use]
extern crate rocket;

use api_state::AppState;
use rocket::fs::{relative, FileServer};
use rocket_dyn_templates::Template;

fn build_rocket(args: Cli) -> rocket::Rocket<rocket::Build> {
    // Initialize application state
    let app_state = AppState::new();

    // Strip any surrounding quotes from config file path
    let config_file_path = args
        .config_file
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string();

    // Attempt to load config from CLI args
    let config_loaded = match setup_config(config_file_path.clone()) {
        Ok(config) => {
            info!("Loaded configuration from: {}", config_file_path);
            app_state.set_config(config.clone());
            app_state.set_config_file_path(config_file_path.clone());
            Some(config)
        }
        Err(e) => {
            warn!(
                "Failed to load config from: {}. Error: {}",
                config_file_path, e
            );
            warn!("Starting with in-memory database. You can load config via the web UI.");
            None
        }
    };

    // Initialize database - use config database if available, otherwise use memory
    if let Some(config) = config_loaded {
        info!(
            "Initializing database from config: {}",
            config.database_file
        );
        if let Err(e) = set_db_pool(&config.database_file) {
            eprintln!("Failed to initialize database from config: {}", e);
            eprintln!("Falling back to in-memory database");
            if let Err(e) = set_db_pool(":memory:") {
                eprintln!("Failed to initialize in-memory database: {}", e);
            }
        } else {
            if let Err(e) = setup_database() {
                eprintln!("Failed to setup database schema: {}", e);
            } else {
                info!(
                    "Database initialized successfully: {}",
                    config.database_file
                );
            }
        }
    } else {
        info!("Initializing database with in-memory storage");
        if let Err(e) = set_db_pool(":memory:") {
            eprintln!("Failed to initialize in-memory database: {}", e);
        } else {
            if let Err(e) = setup_database() {
                eprintln!("Failed to setup database schema: {}", e);
            } else {
                info!("In-memory database initialized successfully");
            }
        }
    }

    rocket::build()
        .manage(app_state)
        .attach(Template::fairing())
        .mount("/static", FileServer::from(relative!("../web/static")))
        .mount(
            "/",
            routes![
                web_routes::index,
                web_routes::dashboard,
                web_routes::configuration,
                web_routes::logs,
                web_routes::help,
            ],
        )
        .mount(
            "/api",
            routes![
                api_routes::get_config,
                api_routes::get_config_form,
                api_routes::set_config,
                api_routes::set_config_form,
                api_routes::save_config_to_file,
                api_routes::load_config_from_file,
                api_routes::get_status,
                api_routes::start_backup,
                api_routes::stop_backup,
                api_routes::get_history,
                api_routes::progress_events,
                api_routes::validate_config_endpoint,
                api_routes::health_check,
                api_routes::get_dashboard_metrics,
                api_routes::get_progress,
                api_routes::get_logs,
                api_routes::get_recent_logs,
                api_routes::get_log_stats,
                api_routes::clear_logs,
                api_routes::get_storage_overview,
            ],
        )
}

#[rocket::main]
async fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_secs()
        .init();

    let args = Cli::parse();

    if args.api_mode {
        build_rocket(args).launch().await?;
        Ok(())
    } else {
        cli_main(args)
    }
}

#[derive(Parser)]
#[command(name = "RustyHashBackup")]
#[command(about = "Hash-based file backup utility", long_about = None)]
struct Cli {
    #[arg(
        short = 'c',
        long = "config",
        default_value = "config.json",
        env = "RUSTYHASHBACKUP_CONFIG"
    )]
    config_file: String,

    #[arg(
        short = 'l',
        long = "log-level",
        default_value = "info",
        env = "LOG_LEVEL"
    )]
    log_level: String,

    #[arg(short = 'q', long = "quiet")]
    quiet: bool,

    #[arg(short = 'v', long = "validate-only")]
    validate_only: bool,

    #[arg(short = 'd', long = "dry-run", conflicts_with = "dry_run_full")]
    dry_run: bool,

    #[arg(short = 'f', long = "dry-run-full", conflicts_with = "dry_run")]
    dry_run_full: bool,

    #[arg(short = 'o', long = "once")]
    once: bool,

    #[arg(long = "api")]
    api_mode: bool,
}

fn cli_main(args: Cli) -> Result<()> {
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info,
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_secs()
        .init();

    info!("RustyHashBackup starting...");
    let config: Config = setup_config(args.config_file).context("Failed to load configuration")?;
    debug!("Loaded config: {:?}", &config);

    if args.validate_only {
        info!("Configuration is valid. Exiting (--validate-only mode).");
        return Ok(());
    }

    let dry_run_mode = if args.dry_run_full {
        info!("Running in DRY RUN FULL mode - will simulate all operations including hashing");
        DryRunMode::Full
    } else if args.dry_run {
        info!("Running in DRY RUN QUICK mode - will show what would be processed (skips hashing)");
        DryRunMode::Quick
    } else {
        DryRunMode::None
    };

    rayon::ThreadPoolBuilder::new()
        .num_threads(config.max_threads)
        .build_global()
        .context("Failed to build thread pool")?;

    set_db_pool(&config.database_file).context("Failed to initialize database connection pool")?;

    setup_database().context("Failed to set up database schema")?;

    let run_once = args.once || config.schedule.is_none();

    if run_once {
        run_backup(&config, dry_run_mode, args.quiet, None)?;
    } else {
        run_scheduled(&config, dry_run_mode, args.quiet)?;
    }

    Ok(())
}

fn run_backup(
    config: &Config,
    dry_run_mode: DryRunMode,
    quiet: bool,
    state: Option<&AppState>,
) -> Result<()> {
    let multi_progress = if !quiet {
        Some(MultiProgress::new())
    } else {
        None
    };

    if let Some(st) = state {
        st.set_progress(Some(models::api::BackupProgress {
            phase: 1,
            phase_description: "Discovering source files".to_string(),
            files_processed: 0,
            total_files: 0,
            bytes_processed: None,
            total_bytes: None,
            percentage: 0.0,
            current_file: None,
        }));
    }

    let discovery_progress = multi_progress.as_ref().map(|mp| {
        mp.add(create_spinner(&format!(
            "{}[1/3] Discovering source files...",
            dry_run_mode.progress_prefix()
        )))
    });

    let backup_candidates = get_source_files(&config.backup_sources, discovery_progress.as_ref())?;

    if let Some(progress) = discovery_progress {
        let total: usize = backup_candidates.values().map(|v| v.len()).sum();
        progress.finish_with_message(format!(
            "{}[1/3] Found {} files across {} directories",
            dry_run_mode.progress_prefix(),
            total,
            backup_candidates.len()
        ));
    }

    if backup_candidates.is_empty() {
        warn!("No source files found to backup");
        return Ok(());
    }

    // Phase 2 & 3: Preparation and Backup
    let total_files: u64 = backup_candidates.values().map(|v| v.len() as u64).sum();

    let prep_progress = multi_progress.as_ref().map(|mp| {
        mp.add(create_progress_bar(
            total_files,
            &format!("{}[2/3] Preparing backups", dry_run_mode.progress_prefix()),
        ))
    });

    let backup_progress = multi_progress.as_ref().map(|mp| {
        let action = if dry_run_mode.should_copy_files() {
            "Copying files"
        } else {
            "Simulating file copy"
        };
        mp.add(create_progress_bar_with_bytes(
            total_files,
            &format!("{}[3/3] {}", dry_run_mode.progress_prefix(), action),
        ))
    });

    backup_files(
        backup_candidates,
        config,
        prep_progress.as_ref(),
        backup_progress.as_ref(),
        dry_run_mode,
        state,
    )
    .context("Backup operation failed")?;

    if let Some(progress) = prep_progress {
        progress.finish();
    }
    if let Some(progress) = backup_progress {
        let message = if dry_run_mode.is_dry_run() {
            format!(
                "{}[3/3] Dry run completed - {} files simulated",
                dry_run_mode.progress_prefix(),
                total_files
            )
        } else {
            format!("[3/3] Backup completed - {} files processed", total_files)
        };
        progress.finish_with_message(message);
    }

    if dry_run_mode.is_dry_run() {
        info!("DRY RUN completed - no files were actually copied or database updated");
    } else {
        info!("Backup operation completed successfully");
    }
    Ok(())
}

fn run_scheduled(config: &Config, dry_run_mode: DryRunMode, quiet: bool) -> Result<()> {
    use chrono::Utc;
    use cron::Schedule;
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    let schedule_str = config.schedule.as_ref().unwrap();
    let schedule = Schedule::from_str(schedule_str).context("Invalid cron expression")?;

    info!(
        "Starting scheduled backup mode with schedule: {}",
        schedule_str
    );

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        info!("Received shutdown signal, stopping scheduler...");
        r.store(false, Ordering::SeqCst);
    })
    .context("Failed to set Ctrl+C handler")?;

    if config.run_on_startup {
        info!("Running initial backup on startup...");
        if let Err(e) = run_backup(config, dry_run_mode, quiet, None) {
            warn!("Initial backup failed: {}", e);
        }
    }

    while running.load(Ordering::SeqCst) {
        let now = Utc::now();

        if let Some(next) = schedule.upcoming(Utc).take(1).next() {
            let duration_until_next = (next - now)
                .to_std()
                .unwrap_or(std::time::Duration::from_secs(0));

            info!(
                "Next backup scheduled for: {} (in {} seconds)",
                next.format("%Y-%m-%d %H:%M:%S %Z"),
                duration_until_next.as_secs()
            );

            let sleep_duration = std::cmp::min(
                duration_until_next,
                std::time::Duration::from_secs(1), // Check shutdown signal every second
            );

            std::thread::sleep(sleep_duration);

            if Utc::now() >= next && running.load(Ordering::SeqCst) {
                info!("Running scheduled backup...");
                if let Err(e) = run_backup(config, dry_run_mode, quiet, None) {
                    warn!("Scheduled backup failed: {}", e);
                }
            }
        } else {
            warn!("No upcoming scheduled times found");
            break;
        }
    }

    info!("Scheduler stopped");
    Ok(())
}

fn get_source_files(
    backup_sources: &Vec<BackupSource>,
    progress: Option<&indicatif::ProgressBar>,
) -> Result<HashMap<PathBuf, Vec<PathBuf>>> {
    info!(
        "Discovering files in {} source directories...",
        backup_sources.len()
    );

    let mut result_map = HashMap::<PathBuf, Vec<PathBuf>>::new();
    let mut total_files = 0;

    for source in backup_sources {
        if let Some(pb) = progress {
            pb.set_message(format!("Scanning: {}", source.parent_directory));
        }

        let files = get_files_in_path(
            &source.parent_directory,
            &source.skip_dirs,
            &source.max_depth,
        )
        .with_context(|| format!("Failed to read directory: {}", source.parent_directory))?;

        if !files.is_empty() {
            let file_count = files.len();
            total_files += file_count;
            result_map.insert(PathBuf::from(&source.parent_directory), files);

            if let Some(pb) = progress {
                pb.set_message(format!(
                    "Found {} files in {}",
                    file_count, source.parent_directory
                ));
            }
        }
    }

    info!(
        "Found {} files across {} directories",
        total_files,
        result_map.len()
    );
    Ok(result_map)
}
