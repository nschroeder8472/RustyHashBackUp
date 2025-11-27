# Scheduling Feature Documentation

## Overview

RustyHashBackup now supports scheduled backups using cron expressions. You can configure the application to run backups automatically at specified intervals.

## Configuration

Add these fields to your `config.json`:

```json
{
  "schedule": "0 0 2 * * *",
  "run_on_startup": true,
  ...
}
```

### Configuration Fields

- **`schedule`** (optional): Cron expression for scheduling backups
  - If omitted, runs once and exits (original behavior)
  - If provided, runs continuously on the specified schedule

- **`run_on_startup`** (optional, default: `true`): Whether to run a backup immediately when the scheduler starts
  - `true`: Run backup on startup, then follow schedule
  - `false`: Wait for first scheduled time

## Cron Expression Format

Uses 6-field cron format: `seconds minutes hours days months weekdays`

### Examples

```
"0 0 2 * * *"      - Daily at 2:00 AM
"0 */30 * * * *"   - Every 30 minutes
"0 0 */6 * * *"    - Every 6 hours
"0 0 0 * * 1"      - Every Monday at midnight
"0 0 9,17 * * 1-5" - Weekdays at 9 AM and 5 PM
```

### Field Values

| Field    | Values          | Special Characters |
|----------|-----------------|-------------------|
| seconds  | 0-59            | * / , -           |
| minutes  | 0-59            | * / , -           |
| hours    | 0-23            | * / , -           |
| days     | 1-31            | * / , - ?         |
| months   | 1-12            | * / , -           |
| weekdays | 0-6 (Sun-Sat)   | * / , - ?         |

## CLI Usage

### One-Time Execution (No Schedule)

```bash
# No schedule in config - runs once
cargo run -- -c config.json

# Schedule in config but override with --once flag
cargo run -- -c config.json --once
```

### Scheduled Execution

```bash
# Runs on schedule (requires schedule field in config)
cargo run -- -c config.json
```

### Stopping the Scheduler

Press `Ctrl+C` to gracefully shutdown the scheduler. The application will stop after the current backup completes.

## Example Configurations

### Daily Backups at 2 AM

```json
{
  "database_file": "./backup.db",
  "schedule": "0 0 2 * * *",
  "run_on_startup": false,
  "backup_sources": [
    {
      "parent_directory": "/data/important"
    }
  ],
  "backup_destinations": [
    "/backup/location"
  ],
  ...
}
```

### Every 4 Hours with Immediate Start

```json
{
  "database_file": "./backup.db",
  "schedule": "0 0 */4 * * *",
  "run_on_startup": true,
  "backup_sources": [
    {
      "parent_directory": "/data/important"
    }
  ],
  "backup_destinations": [
    "/backup/location"
  ],
  ...
}
```

### Every 15 Minutes (Development/Testing)

```json
{
  "database_file": "./backup.db",
  "schedule": "0 */15 * * * *",
  "run_on_startup": true,
  ...
}
```

## Behavior

### With Schedule Configured

1. **On startup**:
   - If `run_on_startup: true` → runs backup immediately
   - If `run_on_startup: false` → waits for scheduled time

2. **Continuous operation**:
   - Calculates next scheduled backup time
   - Displays countdown in logs
   - Runs backup at scheduled time
   - Repeats indefinitely

3. **Graceful shutdown**:
   - Handles `Ctrl+C` signal
   - Stops after current backup completes
   - Logs shutdown message

### Without Schedule (Original Behavior)

- Runs backup once
- Exits when complete

## Logging

The scheduler provides informative logging:

```
[INFO] Starting scheduled backup mode with schedule: 0 0 2 * * *
[INFO] Running initial backup on startup...
[INFO] Backup operation completed successfully
[INFO] Next backup scheduled for: 2025-11-26 02:00:00 UTC (in 14235 seconds)
[INFO] Received shutdown signal, stopping scheduler...
[INFO] Scheduler stopped
```

## Docker Usage

When running in Docker, set environment variables:

```bash
docker run -d \
  -e RUSTYHASHBACKUP_CONFIG=/data/config.json \
  -v /path/to/source:/source \
  -v /path/to/dest:/destination \
  -v /path/to/config:/data \
  rustyhashbackup
```

The container will run continuously with the configured schedule.

## Testing

Test the schedule validation:

```bash
# Validate config without running
cargo run -- -c config.json --validate-only
```

Test with a short schedule for verification:

```json
{
  "schedule": "0 */1 * * * *",  // Every minute
  "run_on_startup": true,
  ...
}
```

## Troubleshooting

### Invalid Cron Expression

If you see:
```
Error: Invalid cron expression in schedule
```

Ensure you're using the 6-field format with seconds:
- ❌ `"*/5 * * * *"` (5 fields)
- ✅ `"0 */5 * * * *"` (6 fields)

### Schedule Not Running

1. Check that `schedule` field is set in config
2. Verify you're not using `--once` flag
3. Check logs for next scheduled time

## Implementation Details

### Dependencies Added

- `cron = "0.12"` - Cron expression parsing
- `chrono = "0.4"` - Date/time handling
- `ctrlc = "3.4"` - Signal handling

### Code Structure

- **src/main.rs**: Added `run_scheduled()` function and scheduler loop
- **src/models/config.rs**: Added `schedule` and `run_on_startup` fields
- **src/models/config_validator.rs**: Added cron expression validation

### Future Enhancements (Potential)

- Email notifications on backup completion/failure
- Retry logic for failed scheduled backups
- Schedule history/statistics
- Multiple schedules (different sources on different schedules)
