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
