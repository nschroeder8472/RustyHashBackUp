# Frontend Issues & Fixes

## CRITICAL - Will Cause Runtime Errors

### 1. Missing template: `partials/config_form_fields`

**File:** `backend/src/api_routes.rs:43`
**Problem:** `get_config_form()` renders `"partials/config_form_fields"` but this template does not exist. Calling `GET /api/config/form` returns a 500 error.

**Fix Option A:** Create the missing template at `web/templates/partials/config_form_fields.html.tera`:

```html
{# Pre-populated config form fields partial #}
{% if config %}
<input type="hidden" id="loaded-config" value='{{ config | json_encode() | safe }}'>
<script>
    const config = JSON.parse(document.getElementById('loaded-config').value);
    populateForm(config);
</script>
{% else %}
<p class="text-gray-400 text-sm">No configuration loaded.</p>
{% endif %}
```

**Fix Option B:** Remove the dead endpoint entirely from `api_routes.rs` and `main.rs` since nothing in the frontend calls it. Remove:
- `get_config_form()` function in `api_routes.rs:38-48`
- `api_routes::get_config_form` from the routes list in `main.rs:108`

---

### 2. Clear Logs button has no response handling

**File:** `web/templates/logs.html.tera:16-19`
**Problem:** The button uses `hx-post="/api/logs/clear"` but the API returns JSON, not HTML. HTMX tries to swap raw JSON into the page. No `hx-target` or `hx-swap` is set.

**Fix:** Replace the HTMX attributes with a JS onclick handler that processes the JSON response and refreshes the log view:

```html
<!-- Before -->
<button class="px-4 py-2 bg-red-700 hover:bg-red-800 text-white rounded-lg transition-colors"
        hx-post="/api/logs/clear"
        hx-confirm="Are you sure you want to clear all logs?">

<!-- After -->
<button class="px-4 py-2 bg-red-700 hover:bg-red-800 text-white rounded-lg transition-colors"
        onclick="clearLogs()">
```

Add the `clearLogs()` function to the `<script>` block:

```javascript
function clearLogs() {
    if (!confirm('Are you sure you want to clear all logs?')) return;

    fetch('/api/logs/clear', { method: 'POST' })
        .then(response => response.json())
        .then(data => {
            if (data.success) {
                showToast(data.message, 'success');
                refreshLogs();
                // Refresh stats too
                htmx.trigger(document.querySelector('[hx-get="/api/logs/stats"]'), 'load');
            } else {
                showToast(data.message, 'error');
            }
        })
        .catch(error => showToast('Failed to clear logs', 'error'));
}
```

---

### 3. Pagination is entirely hardcoded and non-functional

**File:** `web/templates/logs.html.tera:218-236`
**Problem:** Static HTML with no click handlers. "1-50 of 2,385" is a fake number. Buttons do nothing.

**Fix:** Replace the hardcoded pagination with a dynamic version driven by actual log count. Move the pagination outside the HTMX-swapped div so it persists, and wire up the buttons:

```html
<!-- Pagination -->
<div class="p-4 border-t border-dark-border flex items-center justify-between" id="log-pagination">
    <p class="text-sm text-gray-400">
        Showing <span class="text-white" id="page-range">-</span>
        of <span class="text-white" id="total-entries">-</span> entries
    </p>
    <div class="flex space-x-2">
        <button class="px-3 py-1 bg-dark-bg border border-dark-border rounded text-sm text-gray-400 hover:text-white hover:border-purple-500 transition-colors"
                id="prev-page-btn" onclick="changePage(-1)" disabled>
            Previous
        </button>
        <span class="px-3 py-1 text-sm text-white" id="current-page-display">Page 1</span>
        <button class="px-3 py-1 bg-dark-bg border border-dark-border rounded text-sm text-gray-400 hover:text-white hover:border-purple-500 transition-colors"
                id="next-page-btn" onclick="changePage(1)">
            Next
        </button>
    </div>
</div>
```

Add pagination state and functions to the script block:

```javascript
let currentPage = 0;
const pageSize = 50;

function changePage(delta) {
    currentPage = Math.max(0, currentPage + delta);
    filterLogs();
}

// In filterLogs(), add limit/offset params:
params.append('limit', pageSize);
params.append('offset', currentPage * pageSize);

// After loading results, update pagination display:
// (This requires the API to also return a total count,
//  or you can count the returned items to detect end-of-results)
```

**Note:** The API endpoint `GET /api/logs` currently doesn't return a total count. You'd need to either add that to the response or handle it client-side by checking if fewer than `pageSize` results were returned.

---

### 4. Hardcoded mock log entries flash before HTMX loads

**File:** `web/templates/logs.html.tera:111-215`
**Problem:** Eight fake log entries with fabricated data are shown before HTMX replaces them. If the API request fails, users see fake data.

**Fix:** Replace the hardcoded mock entries with a loading placeholder:

```html
<div class="divide-y divide-dark-border max-h-[600px] overflow-y-auto"
     id="log-entries"
     hx-get="/api/logs"
     hx-trigger="load"
     hx-swap="innerHTML">

    <!-- Loading placeholder -->
    <div class="p-8 text-center text-gray-500">
        <svg class="w-8 h-8 mx-auto mb-3 animate-spin text-gray-600" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
        </svg>
        <p class="text-sm">Loading logs...</p>
    </div>
</div>
```

Also remove the `every 10s` from `hx-trigger` (see issue #14).

---

## HIGH - Functional Problems

### 5. Dashboard SSE/progress section is broken

**File:** `web/templates/dashboard.html.tera:72-102`
**Problem:** Multiple conflicting HTMX directives on the same element. The section is permanently hidden. SSE event names don't match.

**Fix:** Separate the SSE connection from the progress display. Use JavaScript to handle SSE events and toggle visibility:

```html
<!-- Backup Progress (shown when backup is running) -->
<div class="hidden" id="backup-progress-section">
    <div class="card">
        <div class="flex items-center justify-between mb-4">
            <h2 class="section-heading mb-0">Backup Progress</h2>
            <span class="text-sm text-gray-400" id="progress-phase">Phase 1/3</span>
        </div>
        <div class="space-y-3">
            <div class="flex justify-between text-sm">
                <span class="text-gray-400" id="progress-description">Discovering files...</span>
                <span class="text-white" id="progress-percentage">0%</span>
            </div>
            <div class="progress-bar-container">
                <div class="progress-bar-primary" style="width: 0%" id="progress-bar"></div>
            </div>
            <div class="flex justify-between text-xs text-gray-500">
                <span id="progress-files">0 / 0 files</span>
                <span id="progress-bytes">0 MB / 0 MB</span>
            </div>
            <div class="text-xs text-gray-600 truncate" id="progress-current-file"></div>
        </div>
    </div>
</div>
```

Add JavaScript SSE handling:

```javascript
// SSE connection for real-time progress
let eventSource = null;

function connectSSE() {
    if (eventSource) eventSource.close();

    eventSource = new EventSource('/api/events');

    eventSource.onmessage = function(event) {
        if (event.data === 'heartbeat') return;

        try {
            const data = JSON.parse(event.data);
            updateProgressUI(data);
        } catch (e) {
            console.error('Failed to parse SSE event:', e);
        }
    };

    eventSource.onerror = function() {
        // Reconnect after delay
        setTimeout(connectSSE, 5000);
    };
}

function updateProgressUI(data) {
    const section = document.getElementById('backup-progress-section');

    if (data.status === 'running') {
        section.classList.remove('hidden');

        if (data.progress) {
            document.getElementById('progress-phase').textContent = `Phase ${data.progress.phase}/3`;
            document.getElementById('progress-description').textContent = data.progress.phase_description;
            document.getElementById('progress-percentage').textContent = `${Math.round(data.progress.percentage)}%`;
            document.getElementById('progress-bar').style.width = `${data.progress.percentage}%`;
            document.getElementById('progress-files').textContent =
                `${data.progress.files_processed} / ${data.progress.total_files} files`;

            if (data.progress.current_file) {
                document.getElementById('progress-current-file').textContent = data.progress.current_file;
            }
        }
    } else {
        section.classList.add('hidden');
    }

    if (data.message) {
        const type = data.status === 'failed' ? 'error' : 'success';
        showToast(data.message, type);
    }
}

// Connect on page load
connectSSE();
```

---

### 6. Dashboard hardcoded placeholder values flash before HTMX loads

**File:** `web/templates/dashboard.html.tera:20-68`
**Problem:** Shows "2 hours ago", "Active", "2,547", "5", "3" as fake initial data.

**Fix:** Replace the hardcoded metric cards with loading skeleton placeholders:

```html
<div class="grid grid-cols-3 gap-4"
     hx-get="/api/dashboard/metrics"
     hx-trigger="load, every 30s"
     hx-swap="innerHTML">

    <!-- Loading skeleton cards -->
    {% for i in range(end=6) %}
    <div class="bg-dark-surface border border-dark-border rounded-lg p-6">
        <div class="animate-pulse">
            <div class="h-4 bg-gray-700 rounded w-24 mb-3"></div>
            <div class="h-8 bg-gray-700 rounded w-16 mb-2"></div>
            <div class="h-3 bg-gray-700 rounded w-32"></div>
        </div>
    </div>
    {% endfor %}
</div>
```

**Note:** Tera's `range()` may need checking. Alternative: just repeat 6 skeleton divs manually.

---

### 7. Quick actions sidebar: hardcoded status indicator

**File:** `web/templates/partials/quick_actions.html.tera:44-52`
**Problem:** "System Ready" and "Last backup: 2 hours ago" are static strings that never update.

**Fix:** Add HTMX polling or connect to the status API:

```html
<!-- Status Indicator -->
<div class="mt-8"
     hx-get="/api/status"
     hx-trigger="load, every 10s"
     hx-swap="innerHTML"
     id="sidebar-status">
    <div class="status-success">
        <div class="flex items-center space-x-2">
            <div class="w-2 h-2 bg-gray-500 rounded-full"></div>
            <span class="text-sm text-gray-400">Loading...</span>
        </div>
    </div>
</div>
```

This requires a new API endpoint that returns an HTML partial (the current `/api/status` returns JSON). Either:
- Create a new `GET /api/status/widget` route that returns a Tera partial, or
- Use JavaScript `fetch()` to call `/api/status` and update the DOM:

```javascript
// In base.html.tera <script> block or a new script
function updateSidebarStatus() {
    fetch('/api/status')
        .then(r => r.json())
        .then(data => {
            const container = document.getElementById('sidebar-status');
            const statusText = data.status.charAt(0).toUpperCase() + data.status.slice(1);
            const isRunning = data.status === 'running';
            const color = isRunning ? 'blue' : (data.status === 'failed' ? 'red' : 'green');
            const bgClass = isRunning ? 'status-info' : (data.status === 'failed' ? 'status-error' : 'status-success');

            container.innerHTML = `
                <div class="${bgClass}">
                    <div class="flex items-center space-x-2">
                        <div class="w-2 h-2 bg-${color}-500 rounded-full ${isRunning ? 'animate-pulse' : ''}"></div>
                        <span class="text-sm text-${color}-400">${statusText}</span>
                    </div>
                    ${data.started_at ? `<p class="text-xs text-gray-400 mt-2">Last run: ${new Date(data.started_at).toLocaleString()}</p>` : ''}
                </div>
            `;
        })
        .catch(() => {});
}

setInterval(updateSidebarStatus, 10000);
updateSidebarStatus();
```

---

### 8. Dashboard Recent Activity shows hardcoded mock data

**File:** `web/templates/partials/logs_preview.html.tera:18-82`
**Problem:** Five fake log entries shown before HTMX replaces them.

**Fix:** Replace mock content with a loading placeholder:

```html
<div class="space-y-3"
     hx-get="/api/logs/recent"
     hx-trigger="load, every 10s"
     hx-swap="innerHTML">

    <!-- Loading placeholder -->
    <div class="py-4 text-center text-gray-500">
        <p class="text-sm">Loading recent activity...</p>
    </div>
</div>
```

---

### 9. `run_on_startup` checkbox not loaded from config

**File:** `web/templates/configuration.html.tera:339-346` (inside `populateForm()`)
**Problem:** The `populateForm()` function never sets the `run_on_startup` checkbox.

**Fix:** Add the missing line after the other checkbox assignments:

```javascript
function populateForm(config) {
    // ... existing assignments ...
    document.querySelector('input[name="skip_source_hash_check_if_newer"]').checked = config.skip_source_hash_check_if_newer || false;
    document.querySelector('input[name="force_overwrite_backup"]').checked = config.force_overwrite_backup || false;
    document.querySelector('input[name="overwrite_backup_if_existing_is_newer"]').checked = config.overwrite_backup_if_existing_is_newer || false;
    document.querySelector('input[name="run_on_startup"]').checked = config.run_on_startup || false;  // ADD THIS LINE
    document.querySelector('input[name="schedule"]').value = config.schedule || '';
    // ... rest of function ...
}
```

Also add `run_on_startup` to the config object built in `saveConfig()` around line 487:

```javascript
const config = {
    database_file: formData.get('database_file').trim().replace(/\\/g, '/') || '',
    max_threads: parseInt(formData.get('max_threads')) || 2,
    max_mebibytes_for_hash: parseInt(formData.get('max_mebibytes_for_hash')) || 1,
    skip_source_hash_check_if_newer: formData.get('skip_source_hash_check_if_newer') === 'on',
    force_overwrite_backup: formData.get('force_overwrite_backup') === 'on',
    overwrite_backup_if_existing_is_newer: formData.get('overwrite_backup_if_existing_is_newer') === 'on',
    run_on_startup: formData.get('run_on_startup') === 'on',  // ADD THIS LINE
    schedule: formData.get('schedule') || null,
    backup_sources: sources,
    backup_destinations: destinations
};
```

---

### 10. Schedule field loses empty-vs-null distinction

**File:** `web/templates/configuration.html.tera:494`
**Problem:** `schedule: formData.get('schedule') || null` converts empty string to null. This is actually correct behavior for this field (empty = no schedule = null), but worth noting.

**No fix needed** - current behavior is correct. An empty schedule field should be sent as `null` to indicate "no schedule configured."

---

### 11. Source/destination index counters break after remove+add

**File:** `web/templates/configuration.html.tera:532-563, 420-461`
**Problem:** After removing a source at index 1 and adding a new one at index 3, `saveConfig()` iterates `source_path_0`, `source_path_1` (missing!), stops. Sources at index 2 and 3 are silently lost.

**Fix:** Change `saveConfig()` to collect sources/destinations by querying the DOM for all entries instead of relying on sequential index numbers:

```javascript
function saveConfig() {
    const filePath = document.getElementById('config_file_path').value.trim().replace(/\\/g, '/');
    if (!filePath) {
        showToast('Please specify a config file path', 'error');
        return;
    }

    // Collect sources by iterating over DOM elements instead of index counters
    const sources = [];
    document.querySelectorAll('#backup-sources .source-entry').forEach(entry => {
        const pathInput = entry.querySelector('input[name^="source_path_"]');
        const depthInput = entry.querySelector('input[name^="source_max_depth_"]');
        const skipInput = entry.querySelector('input[name^="source_skip_dirs_"]');

        const path = pathInput ? pathInput.value.trim().replace(/\\/g, '/') : '';
        if (path) {
            const source = { parent_directory: path };

            if (depthInput && depthInput.value.trim() !== '') {
                source.max_depth = parseInt(depthInput.value);
            }

            if (skipInput && skipInput.value.trim() !== '') {
                source.skip_dirs = skipInput.value.split(',').map(s => s.trim().replace(/\\/g, '/')).filter(s => s);
            } else {
                source.skip_dirs = [];
            }

            sources.push(source);
        }
    });

    if (sources.length === 0) {
        showToast('At least one backup source must be configured', 'error');
        return;
    }

    // Collect destinations by iterating over DOM elements
    const destinations = [];
    document.querySelectorAll('#backup-destinations .destination-entry input[name^="destination_"]').forEach(input => {
        const dest = input.value.trim().replace(/\\/g, '/');
        if (dest) destinations.push(dest);
    });

    if (destinations.length === 0) {
        showToast('At least one backup destination must be configured', 'error');
        return;
    }

    const form = document.getElementById('config-form');
    const formData = new FormData(form);

    const config = {
        database_file: formData.get('database_file').trim().replace(/\\/g, '/') || '',
        max_threads: parseInt(formData.get('max_threads')) || 2,
        max_mebibytes_for_hash: parseInt(formData.get('max_mebibytes_for_hash')) || 1,
        skip_source_hash_check_if_newer: formData.get('skip_source_hash_check_if_newer') === 'on',
        force_overwrite_backup: formData.get('force_overwrite_backup') === 'on',
        overwrite_backup_if_existing_is_newer: formData.get('overwrite_backup_if_existing_is_newer') === 'on',
        run_on_startup: formData.get('run_on_startup') === 'on',
        schedule: formData.get('schedule') || null,
        backup_sources: sources,
        backup_destinations: destinations
    };

    // ... rest of save logic unchanged ...
}
```

---

## MEDIUM - UX/Quality Issues

### 12. Checkbox formData.get() returns null when unchecked

**File:** `web/templates/configuration.html.tera:491-493`
**Problem:** Using `formData.get('name') === 'on'` works but is fragile.

**Fix (optional improvement):** Use checked property directly:

```javascript
// Before
skip_source_hash_check_if_newer: formData.get('skip_source_hash_check_if_newer') === 'on',

// After (more explicit)
skip_source_hash_check_if_newer: document.querySelector('input[name="skip_source_hash_check_if_newer"]').checked,
force_overwrite_backup: document.querySelector('input[name="force_overwrite_backup"]').checked,
overwrite_backup_if_existing_is_newer: document.querySelector('input[name="overwrite_backup_if_existing_is_newer"]').checked,
run_on_startup: document.querySelector('input[name="run_on_startup"]').checked,
```

---

### 13. filterLogs() triggers on every keystroke

**File:** `web/templates/logs.html.tera:60`
**Problem:** `oninput="filterLogs()"` fires an API request per keystroke, hammering the server.

**Fix:** Add debouncing:

```html
<!-- Before -->
<input type="text" class="input-field-sm w-full" placeholder="Search logs..."
       id="log-search" oninput="filterLogs()">

<!-- After -->
<input type="text" class="input-field-sm w-full" placeholder="Search logs..."
       id="log-search" oninput="debouncedFilterLogs()">
```

Add debounce function to the script block:

```javascript
let filterTimeout = null;

function debouncedFilterLogs() {
    clearTimeout(filterTimeout);
    filterTimeout = setTimeout(filterLogs, 300);
}
```

---

### 14. Auto-refresh overwrites filter results

**File:** `web/templates/logs.html.tera:106-108`
**Problem:** `hx-trigger="load, every 10s"` polls `/api/logs` with no params, overwriting any active filter every 10 seconds.

**Fix:** Remove the `every 10s` trigger. Only load once on page load, and let the user manually refresh or have filters trigger reloads:

```html
<!-- Before -->
<div class="divide-y divide-dark-border max-h-[600px] overflow-y-auto"
     id="log-entries"
     hx-get="/api/logs"
     hx-trigger="load, every 10s"
     hx-swap="innerHTML">

<!-- After -->
<div class="divide-y divide-dark-border max-h-[600px] overflow-y-auto"
     id="log-entries"
     hx-get="/api/logs"
     hx-trigger="load"
     hx-swap="innerHTML">
```

The Refresh button already calls `refreshLogs()` which triggers a manual reload. The `filterLogs()` function also triggers reloads. The 10s auto-refresh is actively harmful.

---

### 15. "last updated" text never actually changes

**File:** `web/templates/dashboard.html.tera:124-127`
**Problem:** Always shows "just now" regardless of actual last update time.

**Fix:** Track the actual update time:

```javascript
let lastUpdateTime = Date.now();

// Listen for HTMX content swap to detect actual updates
document.body.addEventListener('htmx:afterSwap', function(event) {
    if (event.detail.target.closest('[hx-get="/api/dashboard/metrics"]')) {
        lastUpdateTime = Date.now();
        document.getElementById('last-update').textContent = 'just now';
    }
});

function updateTimestamp() {
    const elapsed = Math.floor((Date.now() - lastUpdateTime) / 1000);
    const el = document.getElementById('last-update');

    if (elapsed < 10) {
        el.textContent = 'just now';
    } else if (elapsed < 60) {
        el.textContent = `${elapsed}s ago`;
    } else {
        el.textContent = `${Math.floor(elapsed / 60)}m ago`;
    }
}

setInterval(updateTimestamp, 5000);
```

---

### 16. Global CSS transition on all elements

**File:** `web/static/css/tailwind.input.css:178-182`
**Problem:** `* { transition-property: ... }` applies to every element. Causes unintended animations on HTMX swaps and potential performance issues.

**Fix:** Remove the global `*` rule and let Tailwind's built-in `transition-colors` utility handle it per-element:

```css
/* Before */
* {
    transition-property: background-color, border-color, color, fill, stroke;
    transition-duration: 150ms;
    transition-timing-function: cubic-bezier(0.4, 0, 0.2, 1);
}

/* After: DELETE the above rule entirely.
   Instead, add `transition-colors` to elements that need it.
   Tailwind already includes transition utilities. */
```

Elements that need transitions (buttons, links, cards) already use Tailwind's `transition-colors` class or the component classes in `@layer components` which have it applied via `@apply`.

---

### 17. Metric card dynamic Tailwind classes may not compile

**File:** `web/templates/partials/metric_card.html.tera:3,13,22`
**Problem:** Classes like `hover:border-{{ color }}-500/50`, `bg-{{ color }}-500/10` are generated at runtime. Tailwind's purge step scans source files for class names but won't find dynamically constructed ones, so these classes may be missing from the compiled CSS.

**Fix:** Add a safelist to `tailwind.config.js` for all the colors used:

```javascript
/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./templates/**/*.{html,tera}",
    "./static/js/**/*.js"
  ],
  darkMode: 'class',
  safelist: [
    // Colors used in metric_card.html.tera
    {
      pattern: /^(bg|border|text|hover:border)-(blue|green|purple|indigo|yellow|teal|red|gray)-(400|500|600)(\/10|\/50)?$/,
    },
  ],
  theme: {
    extend: {
      colors: {
        'dark-bg': '#1e1e1e',
        'dark-surface': '#2d2d2d',
        'dark-border': '#3d3d3d',
      }
    }
  },
  plugins: [],
}
```

After updating the config, rebuild the CSS:
```bash
cd web && npm run build:css
```

---

### 18. No CSRF protection on POST endpoints

**File:** All POST routes in `api_routes.rs`
**Problem:** Any page on any domain can send POST requests to `/api/start`, `/api/stop`, `/api/logs/clear`, etc.

**Fix:** Since this runs locally, the risk is lower, but for defense in depth, add a custom header check. HTMX and fetch both allow custom headers, but simple cross-origin form POSTs cannot set them:

Backend - add a Rocket fairing or request guard:

```rust
use rocket::request::{self, FromRequest, Request};

pub struct CsrfGuard;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CsrfGuard {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        // Check for custom header that browsers won't send on cross-origin requests
        match request.headers().get_one("X-Requested-With") {
            Some("RustyHashBackup") => request::Outcome::Success(CsrfGuard),
            _ => request::Outcome::Forward(rocket::http::Status::Forbidden),
        }
    }
}
```

Frontend - add the header to all requests:

```html
<!-- In base.html.tera, after HTMX script -->
<script>
    // Configure HTMX to send custom header
    document.body.addEventListener('htmx:configRequest', function(event) {
        event.detail.headers['X-Requested-With'] = 'RustyHashBackup';
    });
</script>
```

And in fetch calls in `configuration.html.tera`:

```javascript
fetch('/api/config/save', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'X-Requested-With': 'RustyHashBackup',  // ADD THIS
    },
    body: JSON.stringify({ file_path: filePath, config: config })
})
```

---

### 19. XSS risk in configuration page innerHTML

**File:** `web/templates/configuration.html.tera:362-393`
**Problem:** `populateForm()` uses `innerHTML` with `${source.parent_directory || ''}` directly from API data. If a config file contains `<script>alert(1)</script>` as a path, it executes.

**Fix:** Use `textContent` for setting values, or escape the HTML. Since these are going into `<input value="">`, the safest approach is to create elements with the DOM API instead of innerHTML:

```javascript
function createSourceEntry(index, source = {}) {
    const entry = document.createElement('div');
    entry.className = 'p-4 bg-dark-bg border border-dark-border rounded-lg source-entry';

    const wrapper = document.createElement('div');
    wrapper.className = 'flex items-start justify-between';

    const fields = document.createElement('div');
    fields.className = 'flex-1 space-y-3';

    // Path input
    const pathInput = document.createElement('input');
    pathInput.type = 'text';
    pathInput.name = `source_path_${index}`;
    pathInput.className = 'w-full px-3 py-2 bg-dark-surface border border-dark-border rounded text-white text-sm focus:outline-none focus:ring-2 focus:ring-purple-500';
    pathInput.placeholder = '/path/to/source';
    pathInput.value = source.parent_directory || '';  // .value is safe, not innerHTML
    fields.appendChild(pathInput);

    // Grid for depth + skip dirs
    const grid = document.createElement('div');
    grid.className = 'grid grid-cols-2 gap-3';

    const depthInput = document.createElement('input');
    depthInput.type = 'number';
    depthInput.name = `source_max_depth_${index}`;
    depthInput.className = 'px-3 py-2 bg-dark-surface border border-dark-border rounded text-white text-sm focus:outline-none focus:ring-2 focus:ring-purple-500';
    depthInput.placeholder = 'Max depth (optional)';
    depthInput.value = source.max_depth != null ? source.max_depth : '';
    grid.appendChild(depthInput);

    const skipInput = document.createElement('input');
    skipInput.type = 'text';
    skipInput.name = `source_skip_dirs_${index}`;
    skipInput.className = 'px-3 py-2 bg-dark-surface border border-dark-border rounded text-white text-sm focus:outline-none focus:ring-2 focus:ring-purple-500';
    skipInput.placeholder = 'Skip dirs (comma separated)';
    skipInput.value = source.skip_dirs ? source.skip_dirs.join(',') : '';
    grid.appendChild(skipInput);

    fields.appendChild(grid);
    wrapper.appendChild(fields);

    // Delete button
    const deleteBtn = document.createElement('button');
    deleteBtn.type = 'button';
    deleteBtn.className = 'ml-3 text-red-400 hover:text-red-300';
    deleteBtn.onclick = function() { removeSource(this); };
    deleteBtn.innerHTML = '<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" /></svg>';
    wrapper.appendChild(deleteBtn);

    entry.appendChild(wrapper);
    return entry;
}

// Use in populateForm():
function populateForm(config) {
    // ... scalar fields ...

    const sourcesContainer = document.getElementById('backup-sources');
    sourcesContainer.innerHTML = '';  // Clear is safe

    if (config.backup_sources && config.backup_sources.length > 0) {
        config.backup_sources.forEach((source, index) => {
            sourcesContainer.appendChild(createSourceEntry(index, source));
            sourceCount = index + 1;
        });
    }

    // Similarly for destinations - use createElement + .value instead of innerHTML
}
```

---

### 20. Schedule Backup button does nothing

**File:** `web/templates/partials/quick_actions.html.tera:34-40`
**Problem:** The button has no click handler or HTMX attributes.

**Fix Option A:** Link to the configuration page schedule section:

```html
<a href="/configuration#schedule"
   class="w-full btn-neutral">
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7V3m8 4V3m-9 8h10M5 21h14a2 2 0 002-2V7a2 2 0 00-2-2H5a2 2 0 00-2 2v12a2 2 0 002 2z"/>
    </svg>
    <span>Schedule Backup</span>
</a>
```

**Fix Option B:** Remove it entirely if the feature isn't implemented:

```html
<!-- Remove the Schedule Backup button completely -->
```

---

## LOW - Minor Issues

### 21. Log stats endpoint runs 5 separate queries

**File:** `backend/src/api_routes.rs:727-747`
**Problem:** 5 individual `query_logs()` calls, each scanning the full table.

**Fix:** Add a dedicated `count_logs_by_level()` function to `repo/sqlite.rs`:

```rust
/// Count logs grouped by level
pub fn count_logs_by_level() -> Result<HashMap<String, usize>> {
    let conn = get_connection()?;
    let mut stmt = conn
        .prepare("SELECT Level, COUNT(*) FROM Logs GROUP BY Level")
        .map_err(|cause| BackupError::DatabaseQuery {
            operation: "count logs by level".to_string(),
            cause,
        })?;

    let mut counts = HashMap::new();
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })
        .map_err(|cause| BackupError::DatabaseQuery {
            operation: "count logs by level".to_string(),
            cause,
        })?;

    for row in rows {
        let (level, count) = row.map_err(|cause| BackupError::DatabaseQuery {
            operation: "collect log counts".to_string(),
            cause,
        })?;
        counts.insert(level, count);
    }

    Ok(counts)
}
```

Update `get_log_stats()` in `api_routes.rs`:

```rust
#[get("/logs/stats")]
pub fn get_log_stats() -> Template {
    let counts = sqlite::count_logs_by_level().unwrap_or_default();

    let error_count = counts.get("ERROR").copied().unwrap_or(0);
    let warn_count = counts.get("WARN").copied().unwrap_or(0);
    let info_count = counts.get("INFO").copied().unwrap_or(0);
    let debug_count = counts.get("DEBUG").copied().unwrap_or(0);
    let trace_count = counts.get("TRACE").copied().unwrap_or(0);
    let total_count = error_count + warn_count + info_count + debug_count + trace_count;

    Template::render(
        "partials/log_stats",
        context! {
            error_count, warn_count, info_count,
            debug_count, trace_count, total_count,
        },
    )
}
```

---

### 22. `config_form_response.html.tera` is unused

**File:** `web/templates/partials/config_form_response.html.tera`
**Problem:** Exists and is rendered by `POST /api/config/form`, but nothing in the frontend calls that endpoint. The config page uses `fetch()` + JSON via `/api/config/save` instead.

**Fix:** Either remove the dead template and endpoint, or use them. Removing is simpler:

- Delete `web/templates/partials/config_form_response.html.tera`
- Remove `set_config_form()` from `api_routes.rs:87-120`
- Remove `api_routes::set_config_form` from the routes list in `main.rs:110`

---

### 23. Log level filter defaults don't match initial load

**File:** `web/templates/logs.html.tera:37` vs `logs.html.tera:106-108`
**Problem:** The dropdown defaults to "Info" selected, but `hx-get="/api/logs"` loads all levels.

**Fix Option A:** Make the initial HTMX load match the default filter by calling `filterLogs()` on load instead of using `hx-trigger="load"`:

```html
<!-- Remove hx-trigger="load" from the log entries div -->
<div class="divide-y divide-dark-border max-h-[600px] overflow-y-auto"
     id="log-entries">
    <div class="p-8 text-center text-gray-500">
        <p class="text-sm">Loading logs...</p>
    </div>
</div>
```

```javascript
// Call filterLogs on page load to apply default filters
window.addEventListener('DOMContentLoaded', function() {
    filterLogs();
});
```

**Fix Option B:** Change the dropdown default to "All Levels":

```html
<option value="all" selected>All Levels</option>
<!-- Remove 'selected' from Info -->
<option value="info">Info</option>
```
