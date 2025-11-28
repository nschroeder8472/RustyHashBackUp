# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

RustyHashBackup is a hash-based file backup utility written in Rust. It uses BLAKE2b512 hashing to detect file changes and maintains backup metadata in SQLite. The tool supports parallel processing via Rayon and tracks source files and their backups across multiple destinations.

## Operation Modes

RustyHashBackup supports two operation modes:

1. **CLI Mode (Default)**: Traditional command-line interface for running backups
2. **API/Web UI Mode**: Web server with REST API and browser-based dashboard

### CLI Mode Commands

```bash
# Build the project
cargo build

# Build release version
cargo build --release

# Run CLI mode with default config (./config.json in current directory)
cargo run

# Run with custom config
cargo run -- --config path/to/config.json
# or
cargo run -- -c path/to/config.json

# Run with environment variable (useful for Docker)
RUSTYHASHBACKUP_CONFIG=/path/to/config.json cargo run

# Windows example with custom config
cargo run -- -c C:\path\to\config.json

# Run release version
cargo run --release -- -c path/to/config.json

# Dry-run modes (simulate without actual copying)
cargo run -- --dry-run           # Quick mode: skip hashing
cargo run -- --dry-run-full      # Full mode: include hashing

# One-time execution (ignore schedule)
cargo run -- --once

# Validate config without running
cargo run -- --validate-only

# Set log level
cargo run -- --log-level debug
```

### API/Web UI Mode Commands

```bash
# Launch web server on default port (8000)
cargo run -- --api

# Access web UI
# Navigate to http://localhost:8000 in your browser

# API endpoints available at http://localhost:8000/api/*
```

### Testing & Quality
```bash
# Run tests (note: currently no tests exist in the project)
cargo test

# Run linter
cargo clippy

# Format code
cargo fmt

# Check without building
cargo check
```

### Docker
```bash
# Build docker image
docker build -t rustyhashbackup .

# Run in container
docker run -v /path/to/source:/source -v /path/to/dest:/destination -v /path/to/config:/data rustyhashbackup
```

## Architecture

### Module Structure

```
src/
├── main.rs              # Entry point, mode dispatcher (CLI vs API)
├── api_routes.rs        # REST API endpoint handlers
├── api_state.rs         # Shared application state for API mode
├── web_routes.rs        # Web UI page routes (dashboard, config, logs, help)
├── models/              # Data structures
│   ├── config.rs        # Config with serde deserialization and defaults
│   ├── config_validator.rs # Configuration validation logic
│   ├── source_row.rs    # Source file database model
│   ├── backup_row.rs    # Backup file database model
│   ├── backed_up_file.rs # Joined source+backup query result
│   ├── prepped_backup.rs # Prepared backup candidate with paths
│   ├── dry_run_mode.rs  # Dry-run mode enum (None/Quick/Full)
│   ├── error.rs         # Custom error types using thiserror
│   └── api.rs           # API request/response models
├── service/             # Business logic
│   ├── backup.rs        # Core backup orchestration and file copy logic
│   └── hash.rs          # BLAKE2b512 file hashing
├── repo/                # Data access
│   └── sqlite.rs        # Database operations, schema, queries
└── utils/               # Helpers
    ├── directory.rs     # File system operations, metadata retrieval
    └── progress.rs      # Progress bar creation and formatting
```

### Data Flow (CLI Mode)

1. **Mode Selection** (main.rs:59-69)
   - Parse CLI args
   - If `--api` flag: launch Rocket web server
   - Otherwise: execute CLI mode with cli_main()

2. **CLI Initialization** (main.rs:113-157)
   - Set up logging based on --log-level
   - Load and deserialize JSON config
   - Handle --validate-only mode
   - Set up Rayon thread pool based on config.max_threads
   - Initialize SQLite connection pool and create tables
   - Determine one-time vs scheduled execution

3. **Source File Discovery** (main.rs:187-197)
   - For each backup_source in config, walk directory tree
   - Respect max_depth and skip_dirs settings
   - Return HashMap<PathBuf, Vec<PathBuf>> mapping parent paths to file lists

4. **Backup Preparation** (backup.rs:125-173)
   - Process files in parallel using Rayon
   - Check for stop signal from API (if provided)
   - For each file:
     - Check if exists in Source_Files table
     - If new: hash file (unless dry-run quick), insert to database
     - If existing: compare last_modified and file_size
     - Conditionally hash based on skip_source_hash_check_if_newer
     - Update database record if file changed
     - Calculate backup paths for each destination
   - Returns Vec<PreppedBackup> with all metadata

5. **Backup Execution** (backup.rs:22-121)
   - Process PreppedBackup candidates in parallel
   - Check for stop signal from API (if provided)
   - For each backup destination:
     - Check if backup is required (complex logic in is_backup_required)
     - Compare source and destination file metadata/hashes
     - Handle unknown files at destination
     - Copy file if needed (unless dry-run mode)
     - Verify backup integrity with hash comparison
     - Insert/update Backup_Files record

### API Endpoints

When running in API mode (`--api`), the following REST endpoints are available:

**Configuration:**
- `GET /api/config` - Retrieve current configuration
- `POST /api/config` - Set/update configuration (validates before accepting)
- `GET /api/validate` - Validate current configuration

**Backup Control:**
- `POST /api/start` - Start a backup job (supports dry-run flags)
  - Request body: `{ "dry_run": bool, "dry_run_full": bool, "quiet": bool }`
  - Returns backup job ID
- `POST /api/stop` - Request cancellation of running backup

**Status & Monitoring:**
- `GET /api/status` - Get current backup status and progress
- `GET /api/history` - Retrieve backup history (last 100 runs)
- `GET /api/events` - Server-Sent Events stream for real-time progress updates
- `GET /api/health` - Health check endpoint (returns "OK")

**Web UI Pages:**
- `GET /` - Redirects to dashboard
- `GET /dashboard` - Main dashboard view
- `GET /configuration` - Configuration management page
- `GET /logs` - Backup logs viewer
- `GET /help` - Help and documentation page

### Scheduled Backups

When a cron schedule is configured, CLI mode can run backups automatically:

```json
{
  "schedule": "0 2 * * *",     // Run at 2 AM daily (cron format)
  "run_on_startup": true       // Run immediately on startup
}
```

- Uses standard cron expression format (5 fields: minute, hour, day, month, weekday)
- Ctrl+C gracefully stops the scheduler
- Errors in scheduled runs are logged but don't stop the scheduler
- Use `--once` flag to ignore schedule and run immediately

### Database Schema

**Source_Files table:**
- ID (primary key, autoincrement)
- File_Name, File_Path (unique constraint together)
- Hash (BLAKE2b512, escaped ASCII format)
- File_Size (bytes)
- Last_Modified (Unix timestamp in seconds)

**Backup_Files table:**
- ID (primary key, autoincrement)
- Source_ID (foreign key to Source_Files)
- File_Name, File_Path (unique constraint together)
- Last_Modified (Unix timestamp in seconds)

### Configuration

Config is JSON file with structure defined in models/config.rs.

**Config File Location:**
- Default: `config.json` in current working directory
- Override with: `--config <path>` or `-c <path>` command-line flag
- Override with: `RUSTYHASHBACKUP_CONFIG` environment variable
- Docker: Set `ENV RUSTYHASHBACKUP_CONFIG=/data/config.json` in Dockerfile

**Required fields:**
- `database_file`: Path to SQLite database
- `backup_sources`: Array of source directories with optional max_depth and skip_dirs
- `backup_destinations`: Array of destination directory paths

**Optional fields with defaults:**
- `max_mebibytes_for_hash`: Max file size to hash in MiB (default: 1)
- `skip_source_hash_check_if_newer`: Skip hashing if file is newer (default: true)
- `force_overwrite_backup`: Always overwrite backups (default: false)
- `overwrite_backup_if_existing_is_newer`: Overwrite even if dest is newer (default: false)
- `max_threads`: Rayon thread pool size (default: num_physical_cpus, must be > 0)
- `schedule`: Cron expression for scheduled backups (default: None, runs once)
- `run_on_startup`: Run backup immediately on startup when using scheduler (default: true)

### Key Implementation Details

**Hash Function (hash.rs):**
- Uses BLAKE2b512 via blake2 crate
- Streams data directly to hasher without loading entire file into memory
- Reads up to max_mebibytes_for_hash * 1 MiB per file
- Uses 8192 byte buffer for efficient streaming
- Hash output encoded as hexadecimal using hex::encode()
- Proper error handling for file read failures

**Parallel Processing:**
- Uses Rayon for parallel file processing
- Thread pool configured globally in main.rs based on config.max_threads
- Mutex<Vec<PreppedBackup>> used to collect results from parallel preparation phase
- r2d2 connection pool for database access (no bottleneck)
- SQLite WAL mode for concurrent reads during writes

**Error Handling:**
- Comprehensive error handling using thiserror and anyhow
- Custom BackupError types with context
- Proper Result propagation throughout codebase
- Graceful error recovery where appropriate
- Detailed error messages with helpful suggestions

**Database Layer (repo/sqlite.rs):**
- r2d2 connection pool for thread-safe concurrent access
- Pool size: `num_cpus::get_physical() + 7` for optimal read/write mix
- WAL mode enabled for file-based databases
- Pragmas: busy_timeout=5000ms, synchronous=NORMAL, foreign_keys=ON
- All database operations return Result types

**Path Handling:**
- Uses PathBuf and MAIN_SEPARATOR for cross-platform compatibility
- Backup paths calculated by stripping shared parent from source and appending to destinations
- Default config path: `config.json` (current directory), overridable via `--config` flag or `RUSTYHASHBACKUP_CONFIG` env var
- Platform-specific error messages for mkdir suggestions (Windows vs Unix/Linux)

## Project Status

**Current Status: Production-Ready ✅**

All critical and high-priority issues have been resolved. The project now features:
- ✅ Streaming hash function (no memory bloat)
- ✅ Proper hex encoding for hashes
- ✅ Comprehensive error handling (thiserror + anyhow)
- ✅ 46 passing unit tests
- ✅ Database connection pooling with WAL mode
- ✅ Cross-platform support (Windows, Linux, macOS)
- ✅ Dry-run modes (Quick/Full) and backup verification
- ✅ Progress reporting and configurable logging
- ✅ REST API with web UI dashboard
- ✅ Scheduled backups via cron expressions
- ✅ Real-time progress streaming via SSE
- ✅ Backup cancellation support
- ✅ CLI and API modes with proper entry point architecture

Reference IMPROVEMENTS.md for detailed list of completed improvements and remaining "nice to have" features.

## Development Notes

- Project uses edition 2021
- Main dependencies:
  - **CLI/Core**: clap, blake2, walkdir, rusqlite, serde/serde_json, rayon, r2d2, r2d2_sqlite, thiserror, anyhow, log, env_logger, indicatif, hex, num_cpus, once_cell
  - **Scheduling**: cron, chrono, ctrlc
  - **API/Web**: rocket, rocket_dyn_templates (tera), tokio, uuid
- Designed to run in Docker with mounted volumes or natively on Windows/Linux/macOS
- Supports both CLI and API/Web UI modes (mutually exclusive at runtime)
- Structured logging via log + env_logger with configurable levels
- Database operations use ON CONFLICT DO UPDATE for upsert behavior
- File last modified times stored as Duration and converted to Unix seconds for database
- Comprehensive test coverage (46 tests) using tempfile and serial_test
- Cross-platform path handling with environment variable support
- API state management using Arc<Mutex<T>> for thread-safe shared state
- Real-time progress updates via Server-Sent Events (SSE)
- Graceful shutdown support via Ctrl+C handler
- Backup cancellation support when running through API
