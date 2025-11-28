# RustyHashBackup

A fast, reliable hash-based file backup utility written in Rust. Uses BLAKE2b512 hashing to detect file changes and maintains backup metadata in SQLite for efficient incremental backups.

## Features

- **Hash-based change detection** - Only backs up files that have actually changed
- **Parallel processing** - Utilizes multiple CPU cores for fast backup operations
- **Incremental backups** - Tracks file state in SQLite database
- **Multiple destinations** - Backup to multiple locations simultaneously
- **Dry-run modes** - Preview what will be backed up without making changes
- **Scheduled backups** - Automated backups using cron expressions
- **Web UI & REST API** - Manage backups through a browser interface
- **Real-time progress** - Server-Sent Events for live backup status
- **Backup verification** - Validates copied files with hash comparison
- **Cancellable operations** - Stop running backups gracefully

## Installation

### Prerequisites

- Rust 1.70+ (install from [rustup.rs](https://rustup.rs))

### Build from source

```bash
git clone <repository-url>
cd RustyHashBackup
cargo build --release
```

The compiled binary will be at `target/release/RustyHashBackUp.exe` (Windows) or `target/release/RustyHashBackUp` (Linux/macOS).

## Quick Start

### 1. Create a configuration file

Create `config.json` in your working directory:


```json
{
  "database_file": "backup.db",
  "backup_sources": [
    {
      "parent_directory": "C:\\Users\\YourName\\Documents",
      "max_depth": 5,
      "skip_dirs": ["node_modules", "target", ".git"]
    }
  ],
  "backup_destinations": [
    "D:\\Backups\\Documents",
    "E:\\SecondaryBackup\\Documents"
  ],
  "max_mebibytes_for_hash": 10,
  "max_threads": 4
}
```

### 2. Run a backup

**CLI Mode (one-time backup):**
```bash
cargo run --release
```

**With custom config:**
```bash
cargo run --release -- --config /path/to/config.json
```

**Dry-run (preview changes):**
```bash
cargo run --release -- --dry-run
```

**Web UI Mode:**
```bash
cargo run --release -- --api
# Open browser to http://localhost:8000
```

## Usage

### CLI Mode

RustyHashBackup supports various command-line options:

```bash
# Basic backup
cargo run --release

# Custom config file
cargo run --release -- -c /path/to/config.json

# Dry-run (quick mode - skip hashing)
cargo run --release -- --dry-run

# Dry-run (full mode - include hashing)
cargo run --release -- --dry-run-full

# Validate config without running
cargo run --release -- --validate-only

# Set log level
cargo run --release -- --log-level debug

# One-time run (ignore schedule)
cargo run --release -- --once

# Quiet mode (no progress bars)
cargo run --release -- --quiet
```

### API/Web UI Mode

Launch the web server:

```bash
cargo run --release -- --api
```

Access the dashboard at `http://localhost:8000`

#### API Endpoints

**Configuration:**
- `GET /api/config` - Get current configuration
- `POST /api/config` - Update configuration
- `GET /api/validate` - Validate configuration

**Backup Control:**
- `POST /api/start` - Start a backup
  ```json
  {
    "dry_run": false,
    "dry_run_full": false,
    "quiet": false
  }
  ```
- `POST /api/stop` - Cancel running backup

**Monitoring:**
- `GET /api/status` - Current status and progress
- `GET /api/history` - Backup history (last 100 runs)
- `GET /api/events` - Server-Sent Events stream
- `GET /api/health` - Health check

### Scheduled Backups

Add schedule to your `config.json`:

```json
{
  "schedule": "0 2 * * *",
  "run_on_startup": true,
  ...
}
```

Cron format: `minute hour day month weekday`

Examples:
- `"0 2 * * *"` - Daily at 2:00 AM
- `"0 */4 * * *"` - Every 4 hours
- `"0 0 * * 0"` - Weekly on Sunday at midnight
- `"0 3 1 * *"` - Monthly on the 1st at 3:00 AM

Press Ctrl+C to stop the scheduler gracefully.

## Configuration Reference

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `database_file` | string | Path to SQLite database (or `:memory:`) |
| `backup_sources` | array | List of source directories to backup |
| `backup_destinations` | array | List of destination directories |

### Backup Source Options

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `parent_directory` | string | - | Root directory to backup |
| `max_depth` | number | unlimited | Maximum subdirectory depth |
| `skip_dirs` | array | `[]` | Directory names to skip |

### Optional Fields

| Field | Type | Default | Description                            |
|-------|------|---------|----------------------------------------|
| `max_mebibytes_for_hash` | number | 1 | Max amount of file to hash (MiB)  |
| `skip_source_hash_check_if_newer` | boolean | true | Skip re-hashing newer source files     |
| `force_overwrite_backup` | boolean | false | Always overwrite destination files     |
| `overwrite_backup_if_existing_is_newer` | boolean | false | Overwrite even if destination is newer |
| `max_threads` | number | CPU cores | Number of parallel threads             |
| `schedule` | string | null | Cron expression for scheduling         |
| `run_on_startup` | boolean | true | Run immediately when scheduler starts  |

## How It Works

1. **Discovery** - Scans source directories for files
2. **Preparation** - Checks database for existing records
   - New files: Hash and insert to database
   - Existing files: Compare size/timestamp
   - Modified files: Re-hash and update database
3. **Backup** - For each destination:
   - Check if backup exists
   - Compare hashes if needed
   - Copy only changed files
   - Verify copied file integrity
   - Update database records

### Hash Algorithm

- Uses **BLAKE2b512** for cryptographic hashing
- Streams files (no memory bloat)
- Only reads up to max configured size of file for efficient hashing
- Hexadecimal encoding for storage

### Database Schema

**Source_Files:**
- Tracks all source files
- Stores hash, size, and last modified time
- Unique constraint on (File_Name, File_Path)

**Backup_Files:**
- Tracks all backup copies
- Links to source file via foreign key
- Records backup location and timestamp

## Performance

- **Parallel processing** - Utilizes Rayon for multi-core performance
- **Connection pooling** - r2d2 with SQLite WAL mode
- **Efficient hashing** - 8KB buffer streaming
- **Smart re-hashing** - Skip unchanged files based on timestamp/size

## Docker Support

See `Dockerfile` in repository for containerized deployment.

```bash
docker build -t rustyhashbackup .
docker run -v /source:/source -v /dest:/destination -v /config:/data rustyhashbackup
```

## Development

### Run Tests

```bash
cargo test
```

Currently 46 passing unit tests.

### Code Quality

```bash
# Linting
cargo clippy

# Formatting
cargo fmt

# Check without building
cargo check
```

### Project Structure

```
src/
├── main.rs              # Entry point, mode dispatcher
├── api_routes.rs        # REST API handlers
├── api_state.rs         # Shared state management
├── web_routes.rs        # Web UI routes
├── models/              # Data structures
├── service/             # Business logic
├── repo/                # Database layer
└── utils/               # Helper functions
```

## Contributing

We welcome contributions! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on:
- How to report bugs and suggest features
- Development setup and coding standards
- Pull request process
- AI usage guidelines
- Code of Conduct

**Quick checklist before submitting:**
- ✅ Tests pass (`cargo test`)
- ✅ Code formatted (`cargo fmt`)
- ✅ Clippy happy (`cargo clippy`)
- ✅ Can explain your changes

## License

MIT License - See LICENSE file for details

## Troubleshooting

### Database locked errors
- Check no other process is using the database
- WAL mode should prevent most locking issues

### Permission errors
- Ensure write permissions on destination directories
- Run with appropriate privileges for source access

### Out of memory
- Reduce `max_threads` in config
- Reduce `max_mebibytes_for_hash` for large files

### Backup verification failures
- Check disk space on destination
- Verify destination drive health
- Check for antivirus interference

## Support

For issues and feature requests, please use the GitHub issue tracker.

## Acknowledgments

Built with:
- [Rust](https://www.rust-lang.org/)
- [Rocket](https://rocket.rs/) - Web framework
- [Rayon](https://github.com/rayon-rs/rayon) - Parallelism
- [SQLite](https://www.sqlite.org/) - Database
- [BLAKE2](https://www.blake2.net/) - Hashing
