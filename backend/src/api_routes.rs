use crate::api_state::AppState;
use crate::models::api::*;
use crate::models::config::Config;
use crate::models::dry_run_mode::DryRunMode;
use crate::repo::sqlite;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::tokio::select;
use rocket::tokio::time::{interval, Duration};
use rocket::{
    response::stream::{Event, EventStream},
    State,
};
use rocket_dyn_templates::{context, Template};
use serde_json::json;

/// GET /api/config - Get current configuration (JSON)
#[get("/config", rank = 2)]
pub fn get_config(state: &State<AppState>) -> Result<Json<ConfigResponse>, Status> {
    let config_file_path = state.get_config_file_path();
    match state.get_config() {
        Some(config) => Ok(Json(ConfigResponse {
            success: true,
            message: "Configuration retrieved successfully".to_string(),
            config: Some(config),
            config_file_path,
        })),
        None => Ok(Json(ConfigResponse {
            success: false,
            message: "No configuration set".to_string(),
            config: None,
            config_file_path,
        })),
    }
}

/// GET /api/config/form - Get configuration form fields pre-populated (HTML)
#[get("/config/form")]
pub fn get_config_form(state: &State<AppState>) -> Template {
    let config = state.get_config();

    Template::render(
        "partials/config_form_fields",
        context! {
            config,
        },
    )
}

/// POST /api/config - Set configuration (JSON)
#[post("/config", format = "json", data = "<config>")]
pub fn set_config(
    config: Json<Config>,
    state: &State<AppState>,
) -> Result<Json<ConfigResponse>, Status> {
    // Validate configuration
    if let Err(e) = crate::models::config_validator::validate_config(&config.0) {
        return Ok(Json(ConfigResponse {
            success: false,
            message: format!("Invalid configuration: {}", e),
            config: None,
            config_file_path: state.get_config_file_path(),
        }));
    }

    // Reinitialize database if path changed
    reinitialize_database(&config.0.database_file);

    state.set_config(config.0.clone());

    // Log configuration change
    let _ = sqlite::insert_log_entry(
        "INFO",
        "Configuration updated via API",
        Some("api_routes::set_config"),
    );

    Ok(Json(ConfigResponse {
        success: true,
        message: "Configuration set successfully".to_string(),
        config: Some(config.0),
        config_file_path: state.get_config_file_path(),
    }))
}

/// POST /api/config/form - Set configuration (JSON, returns HTML)
#[post("/config/form", format = "json", data = "<config>")]
pub fn set_config_form(config: Json<Config>, state: &State<AppState>) -> Template {
    // Validate configuration
    if let Err(e) = crate::models::config_validator::validate_config(&config.0) {
        return Template::render(
            "partials/config_form_response",
            context! {
                success: false,
                message: "Configuration validation failed",
                details: e.to_string(),
            },
        );
    }

    // Reinitialize database if path changed
    reinitialize_database(&config.0.database_file);

    state.set_config(config.0.clone());

    // Log configuration change
    let _ = sqlite::insert_log_entry(
        "INFO",
        "Configuration updated via form",
        Some("api_routes::set_config_form"),
    );

    Template::render(
        "partials/config_form_response",
        context! {
            success: true,
            message: "Configuration saved successfully",
        },
    )
}

/// POST /api/config/save - Save configuration to file
#[post("/config/save", format = "json", data = "<request>")]
pub fn save_config_to_file(
    request: Json<serde_json::Value>,
    state: &State<AppState>,
) -> Result<Json<serde_json::Value>, Status> {
    use std::fs;
    use std::path::Path;

    // Get config file path from request
    let file_path = request
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(Status::BadRequest)?
        .to_string();

    // Validate path is not empty
    if file_path.trim().is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "Config file path cannot be empty"
        })));
    }

    // Get config from request
    let config: Config = match request.get("config") {
        Some(config_value) => match serde_json::from_value(config_value.clone()) {
            Ok(c) => c,
            Err(e) => {
                return Ok(Json(json!({
                    "success": false,
                    "message": format!("Invalid config format: {}", e)
                })));
            }
        },
        None => {
            return Ok(Json(json!({
                "success": false,
                "message": "No config data provided"
            })));
        }
    };

    // Validate configuration
    if let Err(e) = crate::models::config_validator::validate_config(&config) {
        return Ok(Json(json!({
            "success": false,
            "message": format!("Config validation failed: {}", e)
        })));
    }

    // Ensure parent directory exists
    if let Some(parent) = Path::new(&file_path).parent() {
        if !parent.exists() {
            if let Err(e) = fs::create_dir_all(parent) {
                return Ok(Json(json!({
                    "success": false,
                    "message": format!("Failed to create config directory: {}", e)
                })));
            }
        }
    }

    // Serialize config to JSON
    let config_json = match serde_json::to_string_pretty(&config) {
        Ok(json) => json,
        Err(e) => {
            return Ok(Json(json!({
                "success": false,
                "message": format!("Failed to serialize config: {}", e)
            })));
        }
    };

    // Write to file
    if let Err(e) = fs::write(&file_path, config_json) {
        return Ok(Json(json!({
            "success": false,
            "message": format!("Failed to write config file: {}", e)
        })));
    }

    // Set config file path in state
    state.set_config_file_path(file_path.clone());

    // Set config in state
    state.set_config(config);

    // Reinitialize database with new config
    if let Some(config) = state.get_config() {
        reinitialize_database(&config.database_file);
    }

    // Log the save
    let _ = sqlite::insert_log_entry(
        "INFO",
        &format!("Configuration saved to file: {}", file_path),
        Some("api_routes::save_config_to_file"),
    );

    Ok(Json(json!({
        "success": true,
        "message": format!("Configuration saved to {}", file_path)
    })))
}

/// POST /api/config/load - Load configuration from file
#[post("/config/load", format = "json", data = "<request>")]
pub fn load_config_from_file(
    request: Json<serde_json::Value>,
    state: &State<AppState>,
) -> Result<Json<serde_json::Value>, Status> {
    // Get config file path from request
    let file_path = request
        .get("file_path")
        .and_then(|v| v.as_str())
        .ok_or(Status::BadRequest)?
        .to_string();

    // Validate path is not empty
    if file_path.trim().is_empty() {
        return Ok(Json(json!({
            "success": false,
            "message": "Config file path cannot be empty"
        })));
    }

    // Load config from file
    match state.load_config_from_file(file_path.clone()) {
        Ok(()) => {
            // Reinitialize database with new config
            if let Some(config) = state.get_config() {
                reinitialize_database(&config.database_file);
            }

            // Log the load
            let _ = sqlite::insert_log_entry(
                "INFO",
                &format!("Configuration loaded from file: {}", file_path),
                Some("api_routes::load_config_from_file"),
            );

            Ok(Json(json!({
                "success": true,
                "message": format!("Configuration loaded from {}", file_path),
                "config": state.get_config(),
                "config_file_path": state.get_config_file_path()
            })))
        }
        Err(e) => Ok(Json(json!({
            "success": false,
            "message": e
        }))),
    }
}

/// Helper function to reinitialize database when config changes
fn reinitialize_database(db_path: &str) {
    use std::path::Path;

    let db_file = if db_path.is_empty() {
        ":memory:".to_string()
    } else {
        db_path.to_string()
    };

    log::info!("Reinitializing database: {}", db_file);

    // Check if it's a file path and the directory exists
    if db_file != ":memory:" && !db_file.starts_with("file::memory:") {
        let path = Path::new(&db_file);
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                log::warn!("Database directory does not exist, using in-memory database instead");
                if let Err(e) = sqlite::set_db_pool(":memory:") {
                    log::error!("Failed to initialize in-memory database: {}", e);
                    return;
                }
            } else {
                if let Err(e) = sqlite::set_db_pool(&db_file) {
                    log::error!("Failed to initialize database at {}: {}", db_file, e);
                    log::info!("Falling back to in-memory database");
                    let _ = sqlite::set_db_pool(":memory:");
                    return;
                }
            }
        }
    } else {
        if let Err(e) = sqlite::set_db_pool(&db_file) {
            log::error!("Failed to initialize database: {}", e);
            return;
        }
    }

    // Setup database schema
    if let Err(e) = sqlite::setup_database() {
        log::error!("Failed to setup database schema: {}", e);
    } else {
        log::info!("Database initialized successfully");
    }
}

/// GET /api/status - Get current backup status
#[get("/status")]
pub fn get_status(state: &State<AppState>) -> Json<StatusResponse> {
    let status = state.get_status();
    let progress = state.get_progress();
    let current_run = state.get_current_run();

    Json(StatusResponse {
        status,
        progress,
        started_at: current_run.as_ref().map(|r| r.started_at.to_rfc3339()),
        completed_at: current_run
            .as_ref()
            .and_then(|r| r.completed_at.map(|dt| dt.to_rfc3339())),
        error: current_run.as_ref().and_then(|r| r.error.clone()),
        dry_run_mode: current_run
            .as_ref()
            .map(|r| format!("{:?}", r.dry_run_mode)),
    })
}

/// POST /api/start - Start a backup
#[post("/start", format = "json", data = "<request>")]
pub fn start_backup(
    request: Json<StartBackupRequest>,
    state: &State<AppState>,
) -> Result<Json<StartBackupResponse>, Status> {
    // Check if already running
    let current_status = state.get_status();
    if current_status == BackupStatus::Running {
        return Ok(Json(StartBackupResponse {
            success: false,
            message: "A backup is already running".to_string(),
            backup_id: None,
        }));
    }

    // Check if configuration is set
    let config = match state.get_config() {
        Some(config) => config,
        None => {
            return Ok(Json(StartBackupResponse {
                success: false,
                message: "No configuration set. Please set configuration first.".to_string(),
                backup_id: None,
            }));
        }
    };

    // Determine dry run mode
    let dry_run_mode = if request.dry_run_full {
        DryRunMode::Full
    } else if request.dry_run {
        DryRunMode::Quick
    } else {
        DryRunMode::None
    };

    // Start the backup run
    let backup_id = state.start_backup_run(dry_run_mode);
    let backup_id_response = backup_id.clone();

    // Clone necessary data for the async task
    let state_inner = state.inner().clone();
    let config_clone = config.clone();
    let quiet = request.quiet;

    // Spawn backup task
    rocket::tokio::spawn(async move {
        log::info!("Backup task started with ID: {}", backup_id);

        let state_for_blocking = state_inner.clone();
        let result = tokio::task::spawn_blocking(move || {
            crate::run_backup(
                &config_clone,
                dry_run_mode,
                quiet,
                Some(&state_for_blocking),
            )
        })
        .await;

        match result {
            Ok(Ok(())) => {
                state_inner.complete_backup_run(None);
                state_inner.notify_message("Backup completed successfully".to_string());
            }
            Ok(Err(e)) => {
                let error_msg = format!("Backup failed: {}", e);
                state_inner.complete_backup_run(Some(error_msg.clone()));
                state_inner.notify_message(error_msg);
            }
            Err(e) => {
                let error_msg = format!("Backup task panicked: {}", e);
                state_inner.complete_backup_run(Some(error_msg.clone()));
                state_inner.notify_message(error_msg);
            }
        }
    });

    Ok(Json(StartBackupResponse {
        success: true,
        message: format!("Backup started with mode: {:?}", dry_run_mode),
        backup_id: Some(backup_id_response),
    }))
}

/// POST /api/stop - Stop the current backup
#[post("/stop")]
pub fn stop_backup(state: &State<AppState>) -> Json<StopBackupResponse> {
    let current_status = state.get_status();

    if current_status != BackupStatus::Running {
        return Json(StopBackupResponse {
            success: false,
            message: "No backup is currently running".to_string(),
        });
    }

    state.request_stop();

    Json(StopBackupResponse {
        success: true,
        message: "Stop signal sent. Backup will stop after current operation.".to_string(),
    })
}

/// GET /api/history - Get backup history
#[get("/history")]
pub fn get_history(state: &State<AppState>) -> Json<BackupHistoryResponse> {
    let entries = state.get_history();
    let total = entries.len();

    Json(BackupHistoryResponse { entries, total })
}

/// GET /api/events - Server-Sent Events for real-time progress updates
#[get("/events")]
pub fn progress_events(state: &State<AppState>) -> EventStream![] {
    let mut receiver = state.subscribe_progress();
    let state_clone = state.inner().clone();

    EventStream! {
        let mut interval = interval(Duration::from_secs(1));

        loop {
            select! {
                event = receiver.recv() => {
                    match event {
                        Ok(progress_event) => {
                            yield Event::json(&progress_event);
                        }
                        Err(_) => {
                            // Channel closed, resubscribe
                            receiver = state_clone.subscribe_progress();
                        }
                    }
                }
                _ = interval.tick() => {
                    // Send heartbeat to keep connection alive
                    yield Event::data("heartbeat");
                }
            }
        }
    }
}

/// GET /api/validate - Validate current configuration
#[get("/validate")]
pub fn validate_config_endpoint(state: &State<AppState>) -> Result<Json<ConfigResponse>, Status> {
    let config_file_path = state.get_config_file_path();
    match state.get_config() {
        Some(config) => match crate::models::config_validator::validate_config(&config) {
            Ok(_) => Ok(Json(ConfigResponse {
                success: true,
                message: "Configuration is valid".to_string(),
                config: Some(config),
                config_file_path,
            })),
            Err(e) => Ok(Json(ConfigResponse {
                success: false,
                message: format!("Configuration validation failed: {}", e),
                config: Some(config),
                config_file_path,
            })),
        },
        None => Ok(Json(ConfigResponse {
            success: false,
            message: "No configuration set".to_string(),
            config: None,
            config_file_path,
        })),
    }
}

/// GET /api/health - Health check endpoint
#[get("/health")]
pub fn health_check() -> &'static str {
    "OK"
}

/// Helper function to format timestamp as "X time ago"
fn format_time_ago(timestamp: &str) -> String {
    use chrono::{DateTime, Utc};

    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
        let now = Utc::now();
        let duration = now.signed_duration_since(dt.with_timezone(&Utc));

        if duration.num_seconds() < 60 {
            "Just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{} min ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{} hours ago", duration.num_hours())
        } else {
            format!("{} days ago", duration.num_days())
        }
    } else {
        "Unknown".to_string()
    }
}

/// GET /api/dashboard/metrics - Get dashboard metrics
#[get("/dashboard/metrics")]
pub fn get_dashboard_metrics(state: &State<AppState>) -> Template {
    let status = state.get_status();
    let history = state.get_history();
    let config = state.get_config();

    // Get the most recent backup from history
    let last_backup = history.first().map(|entry| {
        json!({
            "time_ago": format_time_ago(&entry.started_at),
            "status": format!("{:?}", entry.status),
            "color": match entry.status {
                BackupStatus::Completed => "green",
                BackupStatus::Failed => "red",
                BackupStatus::Running => "blue",
                _ => "gray",
            }
        })
    });

    // Current backup status
    let current_status = json!({
        "value": format!("{:?}", status),
        "subtitle": if status == BackupStatus::Running { "In progress" } else { "Idle" },
        "color": match status {
            BackupStatus::Running => "blue",
            BackupStatus::Failed => "red",
            BackupStatus::Completed => "green",
            _ => "gray",
        }
    });

    // Continuous backup status (placeholder - not yet implemented)
    let continuous_backup = json!({
        "status": "Disabled",
        "subtitle": "Manual mode",
        "color": "gray"
    });

    // Query database for total files backed up
    let total_source_files = sqlite::get_total_source_files().unwrap_or(0);
    let total_source_size = sqlite::get_total_source_size().unwrap_or(0);
    let total_files = json!({
        "value": total_source_files.to_string(),
        "subtitle": sqlite::format_bytes(total_source_size)
    });

    // Source directories from config
    let source_count = config.as_ref().map(|c| c.backup_sources.len()).unwrap_or(0);
    let source_directories = json!({
        "count": source_count.to_string(),
        "size": format!("{} files", total_source_files)
    });

    // Backup destinations from config
    let dest_count = config
        .as_ref()
        .map(|c| c.backup_destinations.len())
        .unwrap_or(0);
    let backup_destinations = json!({
        "count": dest_count.to_string(),
        "status": if dest_count > 0 { "Active" } else { "None configured" }
    });

    Template::render(
        "partials/dashboard_metrics",
        context! {
            last_backup,
            current_status,
            continuous_backup,
            total_files,
            source_directories,
            backup_destinations,
        },
    )
}

/// GET /api/progress - Get current backup progress
#[get("/progress")]
pub fn get_progress(state: &State<AppState>) -> Json<Option<BackupProgress>> {
    Json(state.get_progress())
}

/// GET /api/logs - Get all logs with optional filters
#[get("/logs?<level>&<since>&<search>&<limit>&<offset>")]
pub fn get_logs(
    level: Option<String>,
    since: Option<i64>,
    search: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Template {
    use chrono::DateTime;

    // Query database for logs with filters
    let logs = sqlite::query_logs(level.as_deref(), since, search.as_deref(), limit, offset)
        .unwrap_or_else(|_| vec![]);

    // Format logs for display
    let formatted_logs: Vec<serde_json::Value> = logs
        .iter()
        .map(|log| {
            // Format timestamp as human-readable
            let formatted_time = if let Some(dt) = DateTime::from_timestamp(log.timestamp, 0) {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                "Unknown".to_string()
            };

            json!({
                "level": log.level,
                "message": log.message,
                "context": log.context,
                "source": log.source,
                "timestamp": log.timestamp,
                "formatted_time": formatted_time,
            })
        })
        .collect();

    Template::render(
        "partials/log_entries",
        context! {
            logs: formatted_logs,
        },
    )
}

/// GET /api/logs/recent - Get recent logs (last 50)
#[get("/logs/recent")]
pub fn get_recent_logs() -> Template {
    use chrono::DateTime;

    // Query database for recent logs (last 50)
    let logs = sqlite::query_logs(None, None, None, Some(50), None).unwrap_or_else(|_| vec![]);

    // Format logs for display
    let formatted_logs: Vec<serde_json::Value> = logs
        .iter()
        .map(|log| {
            // Format timestamp as human-readable
            let formatted_time = if let Some(dt) = DateTime::from_timestamp(log.timestamp, 0) {
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            } else {
                "Unknown".to_string()
            };

            json!({
                "level": log.level,
                "message": log.message,
                "context": log.context,
                "source": log.source,
                "timestamp": log.timestamp,
                "formatted_time": formatted_time,
            })
        })
        .collect();

    Template::render(
        "partials/logs_preview_items",
        context! {
            logs: formatted_logs,
        },
    )
}

/// POST /api/logs/clear - Clear log history
#[post("/logs/clear")]
pub fn clear_logs() -> Json<serde_json::Value> {
    match sqlite::delete_all_logs() {
        Ok(count) => Json(json!({
            "success": true,
            "message": format!("Cleared {} log entries", count)
        })),
        Err(e) => Json(json!({
            "success": false,
            "message": format!("Failed to clear logs: {}", e)
        })),
    }
}

/// GET /api/logs/stats - Get log statistics by level
#[get("/logs/stats")]
pub fn get_log_stats() -> Template {
    // Query database for log counts by level
    let error_count = sqlite::query_logs(Some("ERROR"), None, None, None, None)
        .map(|logs| logs.len())
        .unwrap_or(0);

    let warn_count = sqlite::query_logs(Some("WARN"), None, None, None, None)
        .map(|logs| logs.len())
        .unwrap_or(0);

    let info_count = sqlite::query_logs(Some("INFO"), None, None, None, None)
        .map(|logs| logs.len())
        .unwrap_or(0);

    let debug_count = sqlite::query_logs(Some("DEBUG"), None, None, None, None)
        .map(|logs| logs.len())
        .unwrap_or(0);

    let trace_count = sqlite::query_logs(Some("TRACE"), None, None, None, None)
        .map(|logs| logs.len())
        .unwrap_or(0);

    let total_count = error_count + warn_count + info_count + debug_count + trace_count;

    Template::render(
        "partials/log_stats",
        context! {
            error_count,
            warn_count,
            info_count,
            debug_count,
            trace_count,
            total_count,
        },
    )
}

/// GET /api/storage/overview - Get storage overview
#[get("/storage/overview")]
pub fn get_storage_overview(state: &State<AppState>) -> Template {
    let config = state.get_config();

    // Get destination paths from config
    let destinations = config
        .as_ref()
        .map(|c| c.backup_destinations.clone())
        .unwrap_or_default();

    // Query database for storage statistics
    let storage_stats = sqlite::get_storage_overview(&destinations).unwrap_or_else(|_| {
        crate::models::storage::StorageStats {
            total_source_files: 0,
            total_source_size: 0,
            destination_stats: vec![],
        }
    });

    // Format destination stats for template
    let formatted_destinations: Vec<serde_json::Value> = storage_stats
        .destination_stats
        .iter()
        .map(|dest| {
            let size_formatted = sqlite::format_bytes(dest.total_size);
            // Calculate percentage (placeholder - real implementation would need max capacity)
            let percentage =
                ((dest.total_size as f64 / 1_000_000_000_000.0) * 100.0).min(100.0) as u32;

            json!({
                "path": dest.destination_root,
                "size_formatted": size_formatted,
                "file_count": dest.file_count,
                "percentage": percentage,
            })
        })
        .collect();

    Template::render(
        "partials/storage_overview",
        context! {
            destinations: formatted_destinations,
        },
    )
}
