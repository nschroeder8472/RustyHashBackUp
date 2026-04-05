use crate::models::api::{BackupHistoryEntry, BackupProgress, BackupStatus, ProgressEvent};
use crate::models::config::Config;
use crate::models::dry_run_mode::DryRunMode;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Maximum number of history entries to keep in memory
const MAX_HISTORY_ENTRIES: usize = 100;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Current configuration (None if not set)
    config: Arc<Mutex<Option<Config>>>,

    /// Config file path for persistence
    config_file_path: Arc<Mutex<Option<String>>>,

    /// Current backup status
    status: Arc<Mutex<BackupStatus>>,

    /// Current progress information
    progress: Arc<Mutex<Option<BackupProgress>>>,

    /// Flag to signal backup should stop
    stop_signal: Arc<AtomicBool>,

    /// Backup run information
    current_run: Arc<Mutex<Option<BackupRunInfo>>>,

    /// Recent backup history
    history: Arc<Mutex<VecDeque<BackupHistoryEntry>>>,

    /// Subscribers for progress events (SSE)
    progress_subscribers: Arc<Mutex<Vec<tokio::sync::broadcast::Sender<ProgressEvent>>>>,

    /// Scheduler thread handle (None if not running)
    scheduler_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,

    /// Atomic flag to signal scheduler should stop
    scheduler_running: Arc<AtomicBool>,
}

/// Information about the current backup run
#[derive(Debug, Clone)]
pub struct BackupRunInfo {
    pub id: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub dry_run_mode: DryRunMode,
    pub error: Option<String>,
}

impl AppState {
    /// Create a new application state
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(None)),
            config_file_path: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(BackupStatus::Idle)),
            progress: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            current_run: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(VecDeque::new())),
            progress_subscribers: Arc::new(Mutex::new(Vec::new())),
            scheduler_handle: Arc::new(Mutex::new(None)),
            scheduler_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Get the current configuration
    pub fn get_config(&self) -> Option<Config> {
        self.config.lock().unwrap().clone()
    }

    /// Set the configuration
    pub fn set_config(&self, config: Config) {
        *self.config.lock().unwrap() = Some(config);
    }

    /// Get the config file path
    pub fn get_config_file_path(&self) -> Option<String> {
        self.config_file_path.lock().unwrap().clone()
    }

    /// Set the config file path
    pub fn set_config_file_path(&self, path: String) {
        *self.config_file_path.lock().unwrap() = Some(path);
    }

    /// Save current configuration to file
    pub fn save_config_to_file(&self) -> Result<(), String> {
        use std::fs;
        use std::path::Path;

        let config = self
            .get_config()
            .ok_or_else(|| "No configuration to save".to_string())?;

        let file_path = self
            .get_config_file_path()
            .ok_or_else(|| "No config file path set".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = Path::new(&file_path).parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create config directory: {}", e))?;
            }
        }

        // Serialize config to JSON
        let config_json = serde_json::to_string_pretty(&config)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        // Write to file
        fs::write(&file_path, config_json)
            .map_err(|e| format!("Failed to write config file: {}", e))?;

        log::info!("Configuration saved to {}", file_path);
        Ok(())
    }

    /// Load configuration from file
    pub fn load_config_from_file(&self, file_path: String) -> Result<(), String> {
        use std::fs;

        // Read file
        let config_str = fs::read_to_string(&file_path)
            .map_err(|e| format!("Failed to read config file: {}", e))?;

        // Parse JSON
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| format!("Failed to parse config file: {}", e))?;

        // Validate
        crate::models::config_validator::validate_config(&config)
            .map_err(|e| format!("Config validation failed: {}", e))?;

        // Store config and file path
        self.set_config(config);
        self.set_config_file_path(file_path.clone());

        log::info!("Configuration loaded from {}", file_path);
        Ok(())
    }

    /// Get the current status
    pub fn get_status(&self) -> BackupStatus {
        self.status.lock().unwrap().clone()
    }

    /// Set the current status
    pub fn set_status(&self, status: BackupStatus) {
        *self.status.lock().unwrap() = status;
        self.notify_progress_update();
    }

    /// Get the current progress
    pub fn get_progress(&self) -> Option<BackupProgress> {
        self.progress.lock().unwrap().clone()
    }

    /// Set the current progress
    pub fn set_progress(&self, progress: Option<BackupProgress>) {
        *self.progress.lock().unwrap() = progress;
        self.notify_progress_update();
    }

    /// Update progress incrementally
    #[allow(dead_code)]
    pub fn update_progress<F>(&self, updater: F)
    where
        F: FnOnce(&mut BackupProgress),
    {
        let mut progress_guard = self.progress.lock().unwrap();
        if let Some(progress) = progress_guard.as_mut() {
            updater(progress);
            // Calculate percentage
            if progress.total_files > 0 {
                progress.percentage =
                    (progress.files_processed as f32 / progress.total_files as f32) * 100.0;
            }
        }
        drop(progress_guard);
        self.notify_progress_update();
    }

    /// Get the stop signal flag
    #[allow(dead_code)]
    pub fn get_stop_signal(&self) -> Arc<AtomicBool> {
        self.stop_signal.clone()
    }

    /// Signal that backup should stop
    pub fn request_stop(&self) {
        self.stop_signal.store(true, Ordering::SeqCst);
        self.set_status(BackupStatus::Stopping);
    }

    /// Reset the stop signal
    pub fn reset_stop_signal(&self) {
        self.stop_signal.store(false, Ordering::SeqCst);
    }

    /// Check if stop was requested
    pub fn is_stop_requested(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }

    /// Start a new backup run
    pub fn start_backup_run(&self, dry_run_mode: DryRunMode) -> String {
        let id = Uuid::new_v4().to_string();
        let run_info = BackupRunInfo {
            id: id.clone(),
            started_at: Utc::now(),
            completed_at: None,
            dry_run_mode,
            error: None,
        };
        *self.current_run.lock().unwrap() = Some(run_info);
        self.reset_stop_signal();
        self.set_status(BackupStatus::Running);
        self.set_progress(Some(BackupProgress::default()));
        id
    }

    /// Complete the current backup run
    pub fn complete_backup_run(&self, error: Option<String>) {
        let mut current_run_guard = self.current_run.lock().unwrap();
        if let Some(run_info) = current_run_guard.as_mut() {
            run_info.completed_at = Some(Utc::now());
            run_info.error = error.clone();

            let status = if error.is_some() {
                BackupStatus::Failed
            } else {
                BackupStatus::Completed
            };

            // Add to history
            let progress = self.get_progress().unwrap_or_default();
            let history_entry = BackupHistoryEntry {
                id: run_info.id.clone(),
                started_at: run_info.started_at.to_rfc3339(),
                completed_at: Some(Utc::now().to_rfc3339()),
                status: status.clone(),
                files_processed: progress.files_processed,
                bytes_processed: progress.bytes_processed,
                error: error.clone(),
                dry_run: run_info.dry_run_mode.is_dry_run(),
            };

            let mut history_guard = self.history.lock().unwrap();
            history_guard.push_front(history_entry);
            if history_guard.len() > MAX_HISTORY_ENTRIES {
                history_guard.pop_back();
            }
            drop(history_guard);

            self.set_status(status);
        }
    }

    /// Get current backup run info
    pub fn get_current_run(&self) -> Option<BackupRunInfo> {
        self.current_run.lock().unwrap().clone()
    }

    /// Get backup history
    pub fn get_history(&self) -> Vec<BackupHistoryEntry> {
        self.history.lock().unwrap().iter().cloned().collect()
    }

    /// Clear backup history
    pub fn clear_history(&self) {
        self.history.lock().unwrap().clear();
    }

    /// Subscribe to progress events
    pub fn subscribe_progress(&self) -> tokio::sync::broadcast::Receiver<ProgressEvent> {
        let (tx, rx) = tokio::sync::broadcast::channel(100);
        self.progress_subscribers.lock().unwrap().push(tx);
        rx
    }

    /// Notify all subscribers of progress update
    fn notify_progress_update(&self) {
        let status = self.get_status();
        let progress = self.get_progress();

        let event = ProgressEvent {
            status,
            progress,
            message: None,
        };

        let mut subscribers = self.progress_subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    /// Notify subscribers with a message
    pub fn notify_message(&self, message: String) {
        let status = self.get_status();
        let progress = self.get_progress();

        let event = ProgressEvent {
            status,
            progress,
            message: Some(message),
        };

        let mut subscribers = self.progress_subscribers.lock().unwrap();
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
    }

    /// Start the scheduler with current configuration
    pub fn start_scheduler(&self) -> Result<(), String> {
        // Check if already running
        if self.scheduler_running.load(Ordering::SeqCst) {
            return Err("Scheduler is already running".to_string());
        }

        // Get current configuration
        let config = self.get_config()
            .ok_or_else(|| "No configuration set".to_string())?;

        // Ensure schedule exists
        let schedule_str = config.schedule.as_ref()
            .ok_or_else(|| "No schedule configured".to_string())?;

        // Validate cron expression before spawning thread
        use cron::Schedule;
        use std::str::FromStr;
        Schedule::from_str(schedule_str)
            .map_err(|e| format!("Invalid cron expression '{}': {}", schedule_str, e))?;

        // Set running flag BEFORE spawning thread
        self.scheduler_running.store(true, Ordering::SeqCst);

        // Clone data for thread
        let state_clone = self.clone();
        let config_clone = config.clone();
        let running_clone = self.scheduler_running.clone();

        // Spawn scheduler thread
        let handle = std::thread::spawn(move || {
            if let Err(e) = crate::run_scheduler_thread(&config_clone, &state_clone, running_clone) {
                log::error!("Scheduler thread error: {}", e);
            }
        });

        // Store handle
        *self.scheduler_handle.lock().unwrap() = Some(handle);

        log::info!("Scheduler started with schedule: {}", schedule_str);
        Ok(())
    }

    /// Stop the currently running scheduler gracefully
    pub fn stop_scheduler(&self) -> Result<(), String> {
        if !self.scheduler_running.load(Ordering::SeqCst) {
            return Ok(()); // Already stopped
        }

        log::info!("Stopping scheduler...");

        // Signal scheduler thread to stop
        self.scheduler_running.store(false, Ordering::SeqCst);

        // Take ownership of handle
        let handle_opt = self.scheduler_handle.lock().unwrap().take();

        if let Some(handle) = handle_opt {
            // Wait for thread with timeout (10 seconds)
            let timeout = std::time::Duration::from_secs(10);
            let start = std::time::Instant::now();

            while !handle.is_finished() {
                if start.elapsed() > timeout {
                    log::warn!("Scheduler stop timeout - abandoning handle");
                    return Err("Scheduler stop timeout".to_string());
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }

            // Join thread to clean up
            if let Err(e) = handle.join() {
                log::error!("Scheduler thread panicked: {:?}", e);
                return Err("Scheduler thread panicked".to_string());
            }
        }

        log::info!("Scheduler stopped successfully");
        Ok(())
    }

    /// Restart the scheduler with current configuration
    pub fn restart_scheduler(&self) -> Result<(), String> {
        log::info!("Restarting scheduler...");

        // Stop existing scheduler (ignore errors)
        let _ = self.stop_scheduler();

        // Small delay for thread cleanup
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Start new scheduler
        self.start_scheduler()
    }

    /// Check if scheduler is currently running
    pub fn is_scheduler_running(&self) -> bool {
        self.scheduler_running.load(Ordering::SeqCst)
    }

    /// Get detailed scheduler status
    pub fn get_scheduler_status(&self) -> crate::models::api::SchedulerStatus {
        use cron::Schedule;
        use std::str::FromStr;
        use chrono::Utc;

        let config = self.get_config();
        let running = self.is_scheduler_running();
        let schedule_str = config.as_ref().and_then(|c| c.schedule.clone());

        // Calculate next run time if running
        let next_run = if running {
            schedule_str.as_ref().and_then(|s| {
                Schedule::from_str(s).ok().and_then(|schedule| {
                    schedule.upcoming(Utc).next().map(|dt| dt.to_rfc3339())
                })
            })
        } else {
            None
        };

        crate::models::api::SchedulerStatus {
            running,
            schedule: schedule_str,
            next_run,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
