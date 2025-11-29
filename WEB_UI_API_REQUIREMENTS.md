# Web UI API Requirements

This document outlines the missing API endpoints and enhancements needed to fully integrate the HTMX-based web UI with the RustyHashBackup backend.

## Overview

The web UI has been implemented with static/mock data. To make it fully functional, the following API endpoints need to be created or enhanced.

## Status: Existing API Endpoints ✅

These endpoints already exist and are functional:

- `GET /api/config` - Get current configuration
- `POST /api/config` - Update configuration (needs to accept form data from UI)
- `GET /api/status` - Get backup status
- `POST /api/backup/start` - Start a backup operation
- `POST /api/backup/stop` - Stop current backup
- `GET /api/events` - SSE stream for backup progress
- `GET /api/history` - Get backup history
- `POST /api/validate-config` - Validate configuration
- `GET /api/health` - Health check endpoint

## Missing API Endpoints

### 1. Dashboard Metrics Endpoint

**Endpoint:** `GET /api/dashboard/metrics`

**Purpose:** Provide dashboard metrics for the main overview page

**Response Format:**
```json
{
  "last_backup": {
    "time_ago": "2 hours ago",
    "timestamp": "2025-11-28T12:23:15Z",
    "status": "completed",
    "duration_seconds": 272
  },
  "continuous_backup": {
    "status": "active" | "inactive" | "scheduled",
    "running_since": "2025-11-28T08:00:00Z"
  },
  "current_operation": {
    "status": "idle" | "running" | "paused",
    "description": "Idle" | "Running backup" | "Paused"
  },
  "statistics": {
    "total_files_backed_up": 2547,
    "total_sources": 5,
    "total_destinations": 3,
    "monitored_size_bytes": 13316915200,
    "successful_backups": 1247,
    "failed_backups": 3,
    "files_changed_today": 127,
    "average_backup_time_seconds": 272,
    "database_size_bytes": 47383552
  }
}
```

**Used by:** `templates/dashboard.html.tera` line 14-17

---

### 2. Storage Overview Endpoint

**Endpoint:** `GET /api/storage/overview`

**Purpose:** Provide disk usage information for backup destinations

**Response Format:**
```json
{
  "destinations": [
    {
      "path": "/backup/dest1",
      "used_bytes": 167772160000,
      "total_bytes": 536870912000,
      "free_bytes": 369098752000,
      "percentage_used": 31,
      "status": "healthy" | "warning" | "critical"
    },
    {
      "path": "/backup/dest2",
      "used_bytes": 305670041600,
      "total_bytes": 536870912000,
      "free_bytes": 231200870400,
      "percentage_used": 57,
      "status": "healthy" | "warning" | "critical"
    }
  ]
}
```

**Used by:** `templates/dashboard.html.tera` line 105-137 (Storage Overview section)

---

### 3. Recent Logs Endpoint

**Endpoint:** `GET /api/logs/recent`

**Purpose:** Get the most recent log entries for the dashboard logs preview

**Query Parameters:**
- `limit` (optional, default: 5) - Number of log entries to return
- `level` (optional) - Filter by log level (error, warn, info, debug, trace)

**Response Format:**
```json
{
  "logs": [
    {
      "id": 1,
      "level": "info" | "warn" | "error" | "debug" | "trace",
      "timestamp": "2025-11-28T14:20:45Z",
      "message": "Backup completed successfully",
      "details": "2,547 files backed up to 3 destinations in 4m 32s",
      "source": "backup.rs:245"
    }
  ]
}
```

**Used by:** `templates/partials/logs_preview.html.tera` line 13-16

---

### 4. Full Logs Endpoint

**Endpoint:** `GET /api/logs`

**Purpose:** Get paginated log entries with filtering for the logs page

**Query Parameters:**
- `page` (optional, default: 1) - Page number
- `per_page` (optional, default: 50) - Entries per page
- `level` (optional) - Filter by log level
- `time_range` (optional: 1h, 6h, 24h, 7d, all) - Time range filter
- `search` (optional) - Search term

**Response Format:**
```json
{
  "logs": [
    {
      "id": 1,
      "level": "error",
      "timestamp": "2025-11-28T14:23:15Z",
      "message": "Failed to copy file: /source/large_file.dat",
      "details": "Error: Permission denied (os error 13)",
      "source": "backup.rs:145"
    }
  ],
  "pagination": {
    "current_page": 1,
    "total_pages": 48,
    "total_entries": 2385,
    "per_page": 50
  },
  "statistics": {
    "error_count": 3,
    "warn_count": 12,
    "info_count": 847,
    "debug_count": 1523,
    "total_count": 2385
  }
}
```

**Used by:** `templates/logs.html.tera` line 137-140

---

### 5. Clear Logs Endpoint

**Endpoint:** `POST /api/logs/clear`

**Purpose:** Clear all log entries (or logs before a certain time)

**Request Body (optional):**
```json
{
  "before": "2025-11-28T00:00:00Z",  // Optional: clear logs before this timestamp
  "level": "debug"  // Optional: only clear logs of this level
}
```

**Response Format:**
```json
{
  "success": true,
  "message": "Cleared 1,523 log entries",
  "cleared_count": 1523
}
```

**Used by:** `templates/logs.html.tera` line 15-17

---

## Enhanced API Endpoints

### 6. Enhanced Config Endpoint

**Current:** `POST /api/config` accepts JSON

**Enhancement Needed:** Should also accept form-encoded data from the configuration page form

**Form Fields Expected:**
- `database_file` - string
- `max_threads` - number
- `max_mebibytes_for_hash` - number
- `skip_source_hash_check_if_newer` - boolean (checkbox)
- `force_overwrite_backup` - boolean (checkbox)
- `overwrite_backup_if_existing_is_newer` - boolean (checkbox)
- `run_on_startup` - boolean (checkbox)
- `schedule` - string (cron expression, optional)
- `source_path_0`, `source_path_1`, ... - strings (dynamic count)
- `source_max_depth_0`, `source_max_depth_1`, ... - numbers (optional)
- `source_skip_dirs_0`, `source_skip_dirs_1`, ... - strings (comma-separated)
- `destination_0`, `destination_1`, ... - strings (dynamic count)

**Used by:** `templates/configuration.html.tera` line 15-18

---

### 7. Enhanced Status Endpoint

**Current:** `GET /api/status` returns basic status

**Enhancement Needed:** Include more detailed metrics matching dashboard requirements

**Suggested Response Enhancement:**
```json
{
  "status": "idle" | "running" | "paused",
  "is_running": false,
  "current_progress": { /* ... existing progress fields ... */ },
  "last_backup": {
    "timestamp": "2025-11-28T12:23:15Z",
    "status": "success" | "failed" | "partial",
    "files_processed": 2547,
    "duration_seconds": 272
  },
  "next_scheduled": "2025-11-28T18:00:00Z",
  "statistics": {
    "total_files": 2547,
    "total_sources": 5,
    "total_destinations": 3,
    "database_size_bytes": 47383552
  }
}
```

---

## Implementation Priority

### High Priority (Core Functionality)
1. **Dashboard Metrics Endpoint** - Required for dashboard to show real data
2. **Enhanced Config Endpoint** - Required for configuration page to work
3. **Recent Logs Endpoint** - Required for dashboard logs preview

### Medium Priority (Enhanced UX)
4. **Full Logs Endpoint** - Required for logs page functionality
5. **Storage Overview Endpoint** - Nice to have for dashboard storage section
6. **Clear Logs Endpoint** - Operational convenience

### Low Priority (Future Enhancement)
7. **Enhanced Status Endpoint** - Current status endpoint is functional, enhancement adds more detail

---

## Database Schema Considerations

To support the logs functionality, you may need to add a logs table to the SQLite database:

```sql
CREATE TABLE IF NOT EXISTS Logs (
    ID INTEGER PRIMARY KEY AUTOINCREMENT,
    Level TEXT NOT NULL,  -- 'error', 'warn', 'info', 'debug', 'trace'
    Timestamp INTEGER NOT NULL,  -- Unix timestamp
    Message TEXT NOT NULL,
    Details TEXT,  -- Optional additional information
    Source TEXT,  -- File and line number (e.g., "backup.rs:145")
    Created_At DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_logs_level ON Logs(Level);
CREATE INDEX idx_logs_timestamp ON Logs(Timestamp);
```

Alternatively, you could implement log reading directly from the application's log files without database storage.

---

## HTMX Integration Notes

The UI uses HTMX for dynamic updates. Key attributes used:

- `hx-get` - GET requests for loading data
- `hx-post` - POST requests for mutations
- `hx-trigger` - When to trigger request (e.g., `load`, `every 30s`)
- `hx-swap` - How to swap content (`innerHTML`, `outerHTML`, `none`)
- `hx-ext="sse"` - Server-Sent Events extension for real-time updates
- `sse-connect` - SSE endpoint URL
- `sse-swap` - SSE event to listen for

The existing `/api/events` SSE endpoint is already being used correctly.

---

## Testing Recommendations

1. Test with curl or Postman before UI integration
2. Ensure CORS is configured if testing from different origins
3. Validate JSON response formats match the schemas above
4. Test pagination with large datasets
5. Test SSE connections for real-time updates
6. Verify form data parsing for configuration updates

---

## Notes

- All timestamps should be in ISO 8601 format (e.g., `2025-11-28T14:20:45Z`)
- Byte sizes should be returned as integers (bytes) - formatting happens client-side
- All endpoints should return proper HTTP status codes (200, 400, 404, 500, etc.)
- Error responses should follow a consistent format:
  ```json
  {
    "error": true,
    "message": "Error description",
    "code": "ERROR_CODE"
  }
  ```

---

## Future Enhancements

These are not required for the current UI but would be nice additions:

1. `GET /api/sources` - List all configured backup sources with file counts
2. `GET /api/destinations` - List all configured destinations with stats
3. `GET /api/files/search` - Search for specific files in backups
4. `GET /api/backup/schedule` - Get next scheduled backup times
5. `POST /api/backup/dry-run` - Run a backup in dry-run mode
6. `GET /api/database/stats` - Database statistics and health
7. `POST /api/database/optimize` - Trigger database VACUUM
8. `GET /api/notifications` - Get system notifications/alerts
9. `WebSocket /api/ws` - Real-time WebSocket connection for live updates
