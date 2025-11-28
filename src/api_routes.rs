use crate::api_state::AppState;
use crate::models::api::*;
use crate::models::config::Config;
use crate::models::dry_run_mode::DryRunMode;
use rocket::serde::json::Json;
use rocket::http::Status;
use rocket::{State, response::stream::{EventStream, Event}};
use rocket::tokio::select;
use rocket::tokio::time::{interval, Duration};

/// GET /api/config - Get current configuration
#[get("/config")]
pub fn get_config(state: &State<AppState>) -> Result<Json<ConfigResponse>, Status> {
    match state.get_config() {
        Some(config) => Ok(Json(ConfigResponse {
            success: true,
            message: "Configuration retrieved successfully".to_string(),
            config: Some(config),
        })),
        None => Ok(Json(ConfigResponse {
            success: false,
            message: "No configuration set".to_string(),
            config: None,
        })),
    }
}

/// POST /api/config - Set configuration
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
        }));
    }

    state.set_config(config.0.clone());

    Ok(Json(ConfigResponse {
        success: true,
        message: "Configuration set successfully".to_string(),
        config: Some(config.0),
    }))
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
        dry_run_mode: current_run.as_ref().map(|r| format!("{:?}", r.dry_run_mode)),
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
            crate::run_backup(&config_clone, dry_run_mode, quiet, Some(&state_for_blocking))
        }).await;

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
    match state.get_config() {
        Some(config) => {
            match crate::models::config_validator::validate_config(&config) {
                Ok(_) => Ok(Json(ConfigResponse {
                    success: true,
                    message: "Configuration is valid".to_string(),
                    config: Some(config),
                })),
                Err(e) => Ok(Json(ConfigResponse {
                    success: false,
                    message: format!("Configuration validation failed: {}", e),
                    config: Some(config),
                })),
            }
        }
        None => Ok(Json(ConfigResponse {
            success: false,
            message: "No configuration set".to_string(),
            config: None,
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
pub fn get_dashboard_metrics(state: &State<AppState>) -> Json<DashboardMetrics> {
    let status = state.get_status();
    let history = state.get_history();

    // Get the most recent backup from history
    let last_backup = history.first().map(|entry| DashboardMetric {
        title: "Last Backup".to_string(),
        value: format_time_ago(&entry.started_at),
        subtitle: format!("{:?}", entry.status),
        icon: "clock".to_string(),
        color: match entry.status {
            BackupStatus::Completed => "green",
            BackupStatus::Failed => "red",
            BackupStatus::Running => "blue",
            _ => "gray",
        }.to_string(),
    });

    // Current backup status
    let current_status = DashboardMetric {
        title: "Current Status".to_string(),
        value: format!("{:?}", status),
        subtitle: if status == BackupStatus::Running { "In progress" } else { "Idle" }.to_string(),
        icon: "activity".to_string(),
        color: match status {
            BackupStatus::Running => "blue",
            BackupStatus::Failed => "red",
            BackupStatus::Completed => "green",
            _ => "gray",
        }.to_string(),
    };

    // Total backups count
    let total_backups = DashboardMetric {
        title: "Total Backups".to_string(),
        value: history.len().to_string(),
        subtitle: "All time".to_string(),
        icon: "database".to_string(),
        color: "purple".to_string(),
    };

    let mut metrics = vec![current_status, total_backups];
    if let Some(last) = last_backup {
        metrics.insert(0, last);
    }

    Json(DashboardMetrics { metrics })
}

/// GET /api/progress - Get current backup progress
#[get("/progress")]
pub fn get_progress(state: &State<AppState>) -> Json<Option<BackupProgress>> {
    Json(state.get_progress())
}

/// GET /api/logs - Get all logs
#[get("/logs")]
pub fn get_logs(state: &State<AppState>) -> Json<LogsResponse> {
    let history = state.get_history();

    // Convert history entries to log format
    let logs: Vec<LogEntry> = history
        .iter()
        .flat_map(|entry| {
            let mut entries = vec![LogEntry {
                timestamp: entry.started_at.clone(),
                level: "INFO".to_string(),
                message: format!("Backup started (ID: {})", entry.id),
            }];

            if let Some(completed) = &entry.completed_at {
                entries.push(LogEntry {
                    timestamp: completed.clone(),
                    level: match entry.status {
                        BackupStatus::Completed => "INFO",
                        BackupStatus::Failed => "ERROR",
                        _ => "WARN",
                    }
                    .to_string(),
                    message: format!(
                        "Backup {} - {} files processed",
                        match entry.status {
                            BackupStatus::Completed => "completed",
                            BackupStatus::Failed => "failed",
                            _ => "stopped",
                        },
                        entry.files_processed
                    ),
                });
            }

            if let Some(error) = &entry.error {
                entries.push(LogEntry {
                    timestamp: entry.completed_at.clone().unwrap_or_else(|| entry.started_at.clone()),
                    level: "ERROR".to_string(),
                    message: error.clone(),
                });
            }

            entries
        })
        .collect();

    let total = logs.len();
    Json(LogsResponse { logs, total })
}

/// GET /api/logs/recent - Get recent logs (last 50)
#[get("/logs/recent")]
pub fn get_recent_logs(state: &State<AppState>) -> Json<LogsResponse> {
    let all_logs = get_logs(state).into_inner();
    let recent_logs: Vec<LogEntry> = all_logs.logs.into_iter().take(50).collect();
    let total = recent_logs.len();

    Json(LogsResponse {
        logs: recent_logs,
        total,
    })
}

/// POST /api/logs/clear - Clear log history
#[post("/logs/clear")]
pub fn clear_logs(state: &State<AppState>) -> Json<serde_json::Value> {
    state.clear_history();
    Json(serde_json::json!({
        "success": true,
        "message": "Logs cleared successfully"
    }))
}

// ============================================================================
// Path Aliases for RESTful naming (matching UI documentation)
// ============================================================================

/// POST /api/backup/start - Alias for /api/start
#[post("/backup/start", format = "json", data = "<request>")]
pub fn start_backup_alias(
    request: Json<StartBackupRequest>,
    state: &State<AppState>,
) -> Result<Json<StartBackupResponse>, Status> {
    start_backup(request, state)
}

/// POST /api/backup/stop - Alias for /api/stop
#[post("/backup/stop")]
pub fn stop_backup_alias(state: &State<AppState>) -> Json<StopBackupResponse> {
    stop_backup(state)
}

/// GET /api/progress/events - Alias for /api/events
#[get("/progress/events")]
pub async fn progress_events_alias(state: &State<AppState>) -> EventStream![] {
    progress_events(state)
}
