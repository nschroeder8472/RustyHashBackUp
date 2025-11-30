use log::{Level, LevelFilter, Log, Metadata, Record};
use std::sync::mpsc::{self, Sender};
use std::thread;

/// Message sent from logger to background worker
struct LogMessage {
    level: String,
    message: String,
    source: Option<String>,
}

/// Custom logger that writes to the database asynchronously
pub struct DatabaseLogger {
    level: LevelFilter,
    tx: Sender<LogMessage>,
}

impl DatabaseLogger {
    /// Create a new DatabaseLogger and start the background worker thread
    pub fn new(level: LevelFilter) -> Self {
        let (tx, rx) = mpsc::channel::<LogMessage>();

        // Spawn background worker thread
        thread::spawn(move || {
            log_writer_worker(rx);
        });

        DatabaseLogger { level, tx }
    }

    /// Get the log level filter
    pub fn level(&self) -> LevelFilter {
        self.level
    }
}

impl Log for DatabaseLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };

        // Also log to stderr for console output
        eprintln!(
            "[{}] {}: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
            level,
            record.args()
        );

        let message = LogMessage {
            level: level.to_string(),
            message: format!("{}", record.args()),
            source: record.module_path().map(|s| s.to_string()),
        };

        // Send to background worker thread (non-blocking)
        // Ignore errors if channel is disconnected
        let _ = self.tx.send(message);
    }

    fn flush(&self) {
        // Channel-based logging doesn't need explicit flushing
    }
}

/// Background worker that writes log messages to the database
fn log_writer_worker(rx: mpsc::Receiver<LogMessage>) {
    while let Ok(log_msg) = rx.recv() {
        // Write to database
        // Ignore errors to prevent infinite recursion if logging fails
        let _ = crate::repo::sqlite::insert_log_entry(
            &log_msg.level,
            &log_msg.message,
            log_msg.source.as_deref(),
        );
    }
}

/// Initialize the database logger
pub fn init_database_logger(level: LevelFilter) -> Result<(), log::SetLoggerError> {
    let logger = DatabaseLogger::new(level);
    log::set_max_level(level);
    log::set_boxed_logger(Box::new(logger))
}
