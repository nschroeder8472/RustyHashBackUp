# RustyHashBackup API Documentation

## Overview

The RustyHashBackup application has been converted to support both CLI and API modes. This document describes the API endpoints, data models, and how to use them.

## Running the Application

### API Mode (Default)
```bash
cargo run
```

The API server will start on `http://localhost:8000` by default.

### CLI Mode (Legacy)
The CLI functionality is still available for backwards compatibility.

```bash
# Build and run with custom config
cargo run -- --config path/to/config.json

# See all CLI options
cargo run -- --help
```

## API Endpoints

All API endpoints are prefixed with `/api`.

### Configuration Management

#### GET /api/config
Get the current configuration.

**Response:**
```json
{
  "success": true,
  "message": "Configuration retrieved successfully",
  "config": {
    "database_file": "backup.db",
    "backup_sources": [...],
    "backup_destinations": [...],
    ...
  }
}
```

#### POST /api/config
Set or update the configuration.

**Request Body:**
```json
{
  "database_file": "backup.db",
  "max_mebibytes_for_hash": 1,
  "backup_sources": [
    {
      "parent_directory": "/path/to/source",
      "max_depth": 999999,
      "skip_dirs": [".git", "node_modules"]
    }
  ],
  "backup_destinations": ["/path/to/dest1", "/path/to/dest2"],
  "skip_source_hash_check_if_newer": true,
  "force_overwrite_backup": false,
  "overwrite_backup_if_existing_is_newer": false,
  "max_threads": 8,
  "schedule": null,
  "run_on_startup": true
}
```

**Response:**
```json
{
  "success": true,
  "message": "Configuration set successfully",
  "config": {...}
}
```

#### GET /api/validate
Validate the current configuration without starting a backup.

**Response:**
```json
{
  "success": true,
  "message": "Configuration is valid",
  "config": {...}
}
```

### Backup Operations

#### POST /api/start
Start a backup operation.

**Request Body:**
```json
{
  "log_level": "info",
  "quiet": false,
  "validate_only": false,
  "dry_run": false,
  "dry_run_full": false,
  "once": true
}
```

**Parameters:**
- `log_level`: Log verbosity (trace, debug, info, warn, error). Default: "info"
- `quiet`: Suppress progress output. Default: false
- `validate_only`: Only validate config. Default: false
- `dry_run`: Quick dry run (skips hashing). Default: false
- `dry_run_full`: Full dry run (simulates all operations). Default: false
- `once`: Run once instead of using schedule. Default: false

**Response:**
```json
{
  "success": true,
  "message": "Backup started with mode: None",
  "backup_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

#### POST /api/stop
Stop the currently running backup.

**Response:**
```json
{
  "success": true,
  "message": "Stop signal sent. Backup will stop after current operation."
}
```

### Status and Progress

#### GET /api/status
Get the current backup status and progress.

**Response:**
```json
{
  "status": "running",
  "progress": {
    "phase": 2,
    "phase_description": "Preparing backups",
    "files_processed": 150,
    "total_files": 500,
    "bytes_processed": 104857600,
    "total_bytes": 524288000,
    "percentage": 30.0,
    "current_file": "/path/to/current/file.txt"
  },
  "started_at": "2025-01-15T10:30:00Z",
  "completed_at": null,
  "error": null,
  "dry_run_mode": "None"
}
```

**Status values:**
- `idle`: No backup running
- `running`: Backup in progress
- `stopping`: Stop requested, finishing current operation
- `completed`: Backup completed successfully
- `failed`: Backup failed with error

**Progress phases:**
- Phase 1: Discovering source files
- Phase 2: Preparing backups (hashing, checking database)
- Phase 3: Copying files

#### GET /api/events
Server-Sent Events (SSE) stream for real-time progress updates.

**Usage:**
```javascript
const eventSource = new EventSource('/api/events');

eventSource.addEventListener('message', (event) => {
  const data = JSON.parse(event.data);
  console.log('Progress update:', data);
});
```

**Event Data:**
```json
{
  "status": "running",
  "progress": {...},
  "message": "Backup completed successfully"
}
```

### History

#### GET /api/history
Get backup history (last 100 runs).

**Response:**
```json
{
  "entries": [
    {
      "id": "550e8400-e29b-41d4-a716-446655440000",
      "started_at": "2025-01-15T10:30:00Z",
      "completed_at": "2025-01-15T10:35:00Z",
      "status": "completed",
      "files_processed": 500,
      "bytes_processed": 524288000,
      "error": null,
      "dry_run": false
    }
  ],
  "total": 1
}
```

### Health Check

#### GET /api/health
Simple health check endpoint.

**Response:**
```
OK
```

## Application State

The application maintains the following state:

1. **Configuration**: Current backup configuration
2. **Status**: Current backup status (idle, running, stopping, completed, failed)
3. **Progress**: Real-time progress information
4. **Stop Signal**: Atomic flag for graceful shutdown
5. **Current Run**: Information about the active backup run
6. **History**: Last 100 backup runs

State is thread-safe and can be accessed concurrently from multiple API requests.

## Error Handling

All endpoints return appropriate HTTP status codes:
- `200 OK`: Successful request
- `400 Bad Request`: Invalid request data
- `500 Internal Server Error`: Server error

Error responses include details:
```json
{
  "error": "Error message",
  "details": "Additional details if available"
}
```

## HTMX Frontend Suggestions

For building an HTMX frontend, consider the following:

### 1. Dashboard Page
```html
<!-- Auto-updating status -->
<div hx-get="/api/status" hx-trigger="every 2s" hx-swap="outerHTML">
  <!-- Status display -->
</div>

<!-- Progress bar -->
<div class="progress-bar" style="width: {{percentage}}%"></div>
```

### 2. Configuration Form
```html
<form hx-post="/api/config" hx-swap="outerHTML">
  <input type="text" name="database_file" required>
  <!-- More form fields -->
  <button type="submit">Save Configuration</button>
</form>
```

### 3. Backup Controls
```html
<!-- Start backup -->
<button hx-post="/api/start"
        hx-vals='{"quiet": false, "once": true}'
        hx-swap="none">
  Start Backup
</button>

<!-- Stop backup -->
<button hx-post="/api/stop" hx-swap="none">
  Stop Backup
</button>
```

### 4. Real-time Progress (SSE)
```html
<div hx-ext="sse" sse-connect="/api/events" sse-swap="message">
  <!-- Progress updates will appear here -->
</div>
```

### 5. Backup History Table
```html
<table hx-get="/api/history" hx-trigger="load">
  <thead>
    <tr>
      <th>Started</th>
      <th>Completed</th>
      <th>Status</th>
      <th>Files</th>
      <th>Size</th>
    </tr>
  </thead>
  <tbody>
    <!-- Rows rendered from API response -->
  </tbody>
</table>
```

### 6. Recommended Features

1. **Dashboard**
   - Current status indicator
   - Real-time progress bar
   - Start/stop controls
   - Last backup summary

2. **Configuration Editor**
   - Form-based config editing
   - Validation feedback
   - Test/validate button
   - Save/load from file

3. **Backup History**
   - Sortable/filterable table
   - Detailed view for each run
   - Export to CSV

4. **Logs Viewer**
   - Real-time log streaming
   - Log level filtering
   - Search functionality

5. **Settings**
   - Schedule configuration
   - Notification preferences
   - Theme selection

### 7. UI Components to Include

- **Status Badge**: Visual indicator of backup status (idle/running/completed/failed)
- **Progress Ring/Bar**: Animated progress visualization
- **File Counter**: Current file X of Y
- **Speed Indicator**: Files/sec or MB/sec
- **ETA Calculator**: Estimated time remaining
- **Toast Notifications**: Success/error messages

### 8. Example Dashboard Layout

```
┌─────────────────────────────────────────────┐
│ RustyHashBackup                             │
├─────────────────────────────────────────────┤
│ Status: ● Running         [Stop]            │
│                                             │
│ Progress: ████████░░░░░░░ 60%              │
│ Files: 300 / 500                           │
│ Phase: Copying files                        │
│ Current: /path/to/file.txt                 │
│                                             │
│ [Start Backup] [Configure] [History]       │
├─────────────────────────────────────────────┤
│ Recent Backups                              │
│ ┌─────────┬──────────┬────────┬──────────┐│
│ │ Started │ Duration │ Status │ Files    ││
│ ├─────────┼──────────┼────────┼──────────┤│
│ │ 10:30   │ 5m 23s   │ ✓      │ 500      ││
│ └─────────┴──────────┴────────┴──────────┘│
└─────────────────────────────────────────────┘
```

## Testing the API

### Using curl

```bash
# Get status
curl http://localhost:8000/api/status

# Set config
curl -X POST http://localhost:8000/api/config \
  -H "Content-Type: application/json" \
  -d @config.json

# Start backup
curl -X POST http://localhost:8000/api/start \
  -H "Content-Type: application/json" \
  -d '{"quiet": false, "once": true}'

# Stop backup
curl -X POST http://localhost:8000/api/stop

# Get history
curl http://localhost:8000/api/history

# Listen to events (SSE)
curl -N http://localhost:8000/api/events
```

### Using JavaScript fetch

```javascript
// Start backup
const startBackup = async () => {
  const response = await fetch('/api/start', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      quiet: false,
      once: true,
      dry_run: false
    })
  });
  const data = await response.json();
  console.log('Backup started:', data.backup_id);
};

// Check status
const getStatus = async () => {
  const response = await fetch('/api/status');
  const data = await response.json();
  return data;
};
```

## Next Steps

1. **Frontend Development**
   - Create HTML templates with HTMX
   - Add CSS styling (consider Tailwind CSS or Bootstrap)
   - Implement real-time updates with SSE

2. **Additional API Endpoints** (suggestions)
   - `GET /api/files/preview`: Preview files to be backed up
   - `GET /api/sources`: List available source directories
   - `GET /api/destinations`: List destination status
   - `POST /api/verify`: Verify backup integrity
   - `GET /api/stats`: Statistics dashboard data

3. **Security Enhancements**
   - Add authentication (API keys, JWT)
   - Add CORS configuration
   - Rate limiting
   - HTTPS support

4. **Monitoring**
   - Metrics endpoint for Prometheus
   - Health check enhancements
   - System resource monitoring

5. **Database Integration**
   - Initialize database on first run
   - Database migration support
   - Backup database export
