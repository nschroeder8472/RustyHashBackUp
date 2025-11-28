use serde::{Deserialize, Serialize};

/// Request parameters for starting a backup
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StartBackupRequest {
    /// Log level (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Suppress progress output
    #[serde(default)]
    pub quiet: bool,

    /// Validate configuration only
    #[serde(default)]
    pub validate_only: bool,

    /// Dry run mode (quick) - show what would be processed
    #[serde(default)]
    pub dry_run: bool,

    /// Dry run mode (full) - simulate all operations including hashing
    #[serde(default)]
    pub dry_run_full: bool,

    /// Run once instead of using schedule
    #[serde(default)]
    pub once: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

/// Response for start backup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartBackupResponse {
    pub success: bool,
    pub message: String,
    pub backup_id: Option<String>,
}

/// Response for stop backup request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopBackupResponse {
    pub success: bool,
    pub message: String,
}

/// Current backup status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BackupStatus {
    Idle,
    Running,
    Stopping,
    Failed,
    Completed,
}

/// Progress information for a backup operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupProgress {
    /// Current phase (1-3)
    pub phase: u8,

    /// Description of current phase
    pub phase_description: String,

    /// Files processed in current phase
    pub files_processed: u64,

    /// Total files to process
    pub total_files: u64,

    /// Bytes processed (for copy phase)
    pub bytes_processed: Option<u64>,

    /// Total bytes to process (for copy phase)
    pub total_bytes: Option<u64>,

    /// Percentage complete (0-100)
    pub percentage: f32,

    /// Current file being processed
    pub current_file: Option<String>,
}

impl Default for BackupProgress {
    fn default() -> Self {
        Self {
            phase: 0,
            phase_description: "Not started".to_string(),
            files_processed: 0,
            total_files: 0,
            bytes_processed: None,
            total_bytes: None,
            percentage: 0.0,
            current_file: None,
        }
    }
}

/// Status response for GET /status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResponse {
    pub status: BackupStatus,
    pub progress: Option<BackupProgress>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error: Option<String>,
    pub dry_run_mode: Option<String>,
}

/// Configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,
    pub config: Option<crate::models::config::Config>,
}

/// Backup history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupHistoryEntry {
    pub id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: BackupStatus,
    pub files_processed: u64,
    pub bytes_processed: Option<u64>,
    pub error: Option<String>,
    pub dry_run: bool,
}

/// Backup history response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupHistoryResponse {
    pub entries: Vec<BackupHistoryEntry>,
    pub total: usize,
}

/// Generic API error response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub details: Option<String>,
}

/// Server-Sent Event data for progress updates
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressEvent {
    pub status: BackupStatus,
    pub progress: Option<BackupProgress>,
    pub message: Option<String>,
}
