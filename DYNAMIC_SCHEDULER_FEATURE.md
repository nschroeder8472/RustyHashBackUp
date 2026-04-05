# Feature Specification: Dynamic Scheduler Management

## Overview

Enable the scheduler to start, stop, and restart dynamically when configuration changes are made through the web UI, without requiring an application restart.

---

## Problem Statement

### Current Behavior

1. **App Launch**: Scheduler starts if a cron schedule exists in `config.json`
2. **Config Change via UI**: Updates in-memory config, but scheduler does NOT respond
3. **Workaround**: User must manually restart the entire application

### User Pain Points

- ❌ Cannot enable scheduling without restarting the app
- ❌ Cannot change schedule frequency on-the-fly
- ❌ Cannot disable scheduling without restarting
- ❌ Poor user experience for testing different schedules

---

## Proposed Solution

### New Behavior

1. **App Launch**: Same as current - scheduler starts if schedule is configured
2. **Config Load/Save via UI**:
   - Detects schedule changes
   - Automatically stops existing scheduler (if running)
   - Starts new scheduler with updated schedule (if schedule is present)
   - All without application restart

### User Benefits

- ✅ Enable/disable scheduling from web UI instantly
- ✅ Test different cron schedules without restarts
- ✅ Better developer experience
- ✅ Better production experience (no downtime)

---

## Technical Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                     AppState                            │
│  ┌───────────────────────────────────────────────────┐ │
│  │ Config (Arc<Mutex<Option<Config>>>)               │ │
│  │ SchedulerHandle (Arc<Mutex<Option<JoinHandle>>>)  │ │
│  │ SchedulerControl (Arc<AtomicBool>)                │ │
│  └───────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
   ┌────▼────┐      ┌──────▼──────┐    ┌─────▼──────┐
   │ Web UI  │      │  Scheduler  │    │   Rocket   │
   │ Changes │──────│   Thread    │    │   Server   │
   │ Config  │      │             │    │            │
   └─────────┘      └─────────────┘    └────────────┘
```

### Key Components

#### 1. AppState Extension

```rust
pub struct AppState {
    config: Arc<Mutex<Option<Config>>>,
    config_file_path: Arc<Mutex<String>>,
    status: Arc<Mutex<BackupStatus>>,
    history: Arc<Mutex<Vec<BackupHistoryEntry>>>,
    progress: Arc<Mutex<Option<BackupProgress>>>,
    stop_requested: Arc<AtomicBool>,

    // NEW: Scheduler management
    scheduler_handle: Arc<Mutex<Option<JoinHandle<()>>>>,
    scheduler_running: Arc<AtomicBool>,
}
```

**New Fields:**
- `scheduler_handle`: Thread handle for the background scheduler
- `scheduler_running`: Atomic flag to signal scheduler to stop

#### 2. Scheduler Control Methods

```rust
impl AppState {
    /// Start the scheduler with the current config
    pub fn start_scheduler(&self) -> Result<(), String> {
        // 1. Check if scheduler is already running
        // 2. Get current config
        // 3. Validate schedule exists
        // 4. Spawn scheduler thread
        // 5. Store thread handle
    }

    /// Stop the currently running scheduler
    pub fn stop_scheduler(&self) -> Result<(), String> {
        // 1. Set scheduler_running to false
        // 2. Wait for thread to finish (with timeout)
        // 3. Clear thread handle
    }

    /// Restart the scheduler with new config
    pub fn restart_scheduler(&self) -> Result<(), String> {
        // 1. Stop existing scheduler
        // 2. Start new scheduler with updated config
    }
}
```

#### 3. Modified Config Endpoints

**`POST /api/config/save`** - Save config to file:
```rust
pub fn save_config_to_file(...) -> Json<ConfigResponse> {
    // 1. Validate config
    // 2. Save to file
    // 3. Update in-memory config
    // 4. Check for schedule changes
    // 5. Restart scheduler if needed
    // 6. Return success + scheduler status
}
```

**`POST /api/config/load`** - Load config from file:
```rust
pub fn load_config_from_file(...) -> Json<ConfigResponse> {
    // 1. Read config from file
    // 2. Validate config
    // 3. Update in-memory config
    // 4. Check for schedule changes
    // 5. Restart scheduler if needed
    // 6. Return success + scheduler status
}
```

---

## Implementation Details

### Schedule Change Detection

```rust
fn has_schedule_changed(old_config: &Option<Config>, new_config: &Config) -> bool {
    match old_config {
        None => new_config.schedule.is_some(), // Schedule added
        Some(old) => {
            // Schedule removed, added, or modified
            old.schedule != new_config.schedule
        }
    }
}
```

### Scheduler Lifecycle

```rust
fn run_scheduler_background(
    config: Config,
    state: Arc<AppState>,
    running: Arc<AtomicBool>
) {
    // Parse cron schedule
    let schedule = Schedule::from_str(schedule_str)?;

    // Run on startup if configured
    if config.run_on_startup {
        run_backup(&config, state);
    }

    // Main loop - check running flag
    while running.load(Ordering::SeqCst) {
        // Calculate next run time
        let next = schedule.upcoming(Utc).next()?;

        // Sleep with periodic checks of running flag
        while Utc::now() < next && running.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(1));
        }

        // Run backup if still running
        if running.load(Ordering::SeqCst) {
            run_backup(&config, state);
        }
    }
}
```

### Thread Safety Considerations

1. **Config Access**: Already uses `Arc<Mutex<...>>` - thread-safe ✓
2. **Scheduler Handle**: Protected by `Mutex` for safe access ✓
3. **Stop Signal**: Uses `AtomicBool` for lock-free signaling ✓
4. **Graceful Shutdown**: Thread checks flag every second

---

## Edge Cases & Error Handling

### Edge Case 1: Rapid Config Changes

**Scenario**: User saves config multiple times quickly

**Solution**:
- Debounce scheduler restarts (ignore changes within 2 seconds)
- Or: Queue restart requests, only process latest

### Edge Case 2: Invalid Cron Expression

**Scenario**: User saves config with invalid cron syntax

**Solution**:
- Validate cron expression before saving
- Return error to user
- Keep existing scheduler running (don't stop it)

### Edge Case 3: Scheduler Thread Hangs

**Scenario**: Backup takes longer than expected, new config saved

**Solution**:
- Implement timeout when stopping scheduler (e.g., 10 seconds)
- Log warning if thread doesn't stop
- Force-kill thread handle (acceptable for background thread)

### Edge Case 4: Schedule Removed

**Scenario**: User removes schedule from config

**Solution**:
- Stop scheduler gracefully
- Log "Scheduler stopped - no schedule configured"
- Update UI to show "Scheduling disabled"

### Edge Case 5: App Shutdown During Backup

**Scenario**: User stops app while scheduled backup is running

**Solution**:
- Existing `stop_requested` flag already handles this
- Scheduler thread exits when flag is set
- Rocket shutdown waits for background threads

---

## API Response Updates

### ConfigResponse Enhancement

```rust
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,

    // NEW: Scheduler status
    pub scheduler_status: Option<SchedulerStatus>,
}

pub struct SchedulerStatus {
    pub running: bool,
    pub schedule: Option<String>,
    pub next_run: Option<String>, // ISO timestamp
}
```

### Example Response

```json
{
    "success": true,
    "message": "Configuration saved successfully",
    "scheduler_status": {
        "running": true,
        "schedule": "0 * * * * *",
        "next_run": "2026-02-16T06:01:00Z"
    }
}
```

---

## UI/UX Improvements

### Dashboard Enhancements

**New Status Indicator**:
```
┌─────────────────────────────────┐
│ Scheduler Status                │
├─────────────────────────────────┤
│ ● Active                        │
│ Next run: in 2 minutes          │
│ Schedule: Every hour            │
└─────────────────────────────────┘
```

### Configuration Page

**Schedule Section Feedback**:
```
Cron Schedule: [0 * * * * *     ]

Scheduler Status: ● Active
Next backup: 2026-02-16 06:01:00 UTC (in 45 seconds)

[Save Configuration]
```

**After Saving**:
```
✓ Configuration saved
✓ Scheduler restarted with new schedule
  Next backup: 2026-02-16 06:05:00 UTC
```

---

## Testing Strategy

### Unit Tests

```rust
#[test]
fn test_scheduler_starts_with_schedule() {
    let state = AppState::new();
    let config = Config { schedule: Some("0 * * * * *"), ... };
    state.set_config(config);

    state.start_scheduler().unwrap();
    assert!(state.is_scheduler_running());
}

#[test]
fn test_scheduler_stops_when_requested() {
    let state = AppState::new();
    // Start scheduler...

    state.stop_scheduler().unwrap();
    assert!(!state.is_scheduler_running());
}

#[test]
fn test_scheduler_restarts_on_config_change() {
    let state = AppState::new();
    // Start with schedule "0 * * * * *"

    // Change to "0 */5 * * * *"
    let new_config = Config { schedule: Some("0 */5 * * * *"), ... };
    state.set_config(new_config);
    state.restart_scheduler().unwrap();

    // Verify new schedule is active
}
```

### Integration Tests

1. **Schedule Enable Test**:
   - Start app without schedule
   - Load config with schedule via API
   - Verify scheduler starts
   - Wait for scheduled time
   - Verify backup runs

2. **Schedule Disable Test**:
   - Start app with schedule
   - Load config without schedule
   - Verify scheduler stops
   - Wait past scheduled time
   - Verify no backup runs

3. **Schedule Change Test**:
   - Start with "every minute" schedule
   - Change to "every 5 minutes"
   - Verify old schedule stops
   - Verify new schedule starts

### Manual Testing Checklist

- [ ] Start app with schedule → scheduler runs
- [ ] Start app without schedule → no scheduler
- [ ] Load config with schedule → scheduler starts
- [ ] Load config without schedule → scheduler stops
- [ ] Change schedule → scheduler restarts
- [ ] Save invalid cron → error, scheduler unchanged
- [ ] Stop app during scheduled backup → graceful shutdown
- [ ] Multiple rapid config changes → no crashes

---

## Performance Considerations

### Memory Impact

- **Thread Handle**: ~16 bytes (minimal)
- **AtomicBool**: 1 byte (minimal)
- **Scheduler Thread**: ~2MB stack (acceptable)

**Total Impact**: Negligible (~2MB)

### CPU Impact

- **Idle Scheduler**: Near-zero (sleeps between checks)
- **Config Change**: Brief spike during thread stop/start
- **No Impact**: On backup performance (separate thread)

### Database Impact

- **No Change**: Scheduler uses existing database connection pool
- **No Locking**: Background thread doesn't block API requests

---

## Migration Path

### Phase 1: Backend Implementation
1. Add scheduler management fields to `AppState`
2. Implement `start_scheduler()`, `stop_scheduler()`, `restart_scheduler()`
3. Modify config save/load endpoints
4. Add scheduler status to API responses

### Phase 2: Testing
1. Unit tests for scheduler lifecycle
2. Integration tests for config changes
3. Manual testing with various scenarios

### Phase 3: UI Updates
1. Add scheduler status indicator to dashboard
2. Show next run time in configuration page
3. Display toast notifications for scheduler state changes

### Phase 4: Documentation
1. Update README with dynamic scheduler info
2. Update API documentation
3. Add troubleshooting guide

---

## Success Metrics

### Functional Requirements
- ✅ Scheduler starts when config with schedule is loaded
- ✅ Scheduler stops when schedule is removed
- ✅ Scheduler restarts when schedule changes
- ✅ No application restart required
- ✅ Graceful error handling for invalid cron expressions

### Non-Functional Requirements
- ✅ Thread stops within 10 seconds of request
- ✅ No memory leaks from repeated restarts
- ✅ No race conditions or deadlocks
- ✅ No impact on API response times

---

## Future Enhancements

### Potential Additions
1. **Pause/Resume**: Temporarily disable scheduler without removing schedule
2. **Multiple Schedules**: Support different schedules for different source directories
3. **Scheduler Metrics**: Track successful/failed scheduled runs
4. **Schedule Preview**: Show next 5 run times before saving
5. **Schedule Templates**: Pre-defined common schedules (daily, weekly, etc.)

---

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Thread doesn't stop gracefully | Scheduler keeps running with old schedule | Implement 10s timeout, force-kill thread |
| Rapid config changes cause instability | App crashes or behaves unexpectedly | Debounce restart requests |
| Invalid cron crashes scheduler | No scheduled backups run | Validate before starting, log errors |
| Memory leak from repeated restarts | App memory grows over time | Ensure thread handles are properly cleaned up |
| Race condition during restart | Two schedulers running simultaneously | Use mutex lock during restart operation |

---

## Acceptance Criteria

This feature is complete when:

1. ✅ User can enable scheduling from web UI without restart
2. ✅ User can disable scheduling from web UI without restart
3. ✅ User can change schedule frequency without restart
4. ✅ Invalid cron expressions are rejected with clear error
5. ✅ Scheduler status is visible in dashboard
6. ✅ Next run time is shown in UI
7. ✅ All tests pass (unit + integration)
8. ✅ No memory leaks detected
9. ✅ Documentation is updated
10. ✅ Feature works on Windows, Linux, and macOS

---

## Timeline Estimate

- **Backend Implementation**: 4-6 hours
- **Testing**: 2-3 hours
- **UI Updates**: 2-3 hours
- **Documentation**: 1-2 hours

**Total**: 9-14 hours

---

## Questions for Review

1. Should scheduler restarts be debounced? If so, what delay?
2. Should we show a confirmation modal before restarting scheduler?
3. Should scheduler state persist across app restarts?
4. Should we add a manual "Restart Scheduler" button in the UI?
5. Should scheduler metrics (run count, success rate) be tracked?

---

## Document Version

- **Version**: 1.0
- **Date**: 2026-02-16
- **Author**: Development Team
- **Status**: Proposed
