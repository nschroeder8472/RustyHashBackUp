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
            status: Arc::new(Mutex::new(BackupStatus::Idle)),
            progress: Arc::new(Mutex::new(None)),
            stop_signal: Arc::new(AtomicBool::new(false)),
            current_run: Arc::new(Mutex::new(None)),
            history: Arc::new(Mutex::new(VecDeque::new())),
            progress_subscribers: Arc::new(Mutex::new(Vec::new())),
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
                progress.percentage = (progress.files_processed as f32 / progress.total_files as f32) * 100.0;
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
        subscribers.retain(|tx| {
            tx.send(event.clone()).is_ok()
        });
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
        subscribers.retain(|tx| {
            tx.send(event.clone()).is_ok()
        });
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
