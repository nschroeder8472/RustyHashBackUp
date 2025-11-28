// RustyHashBackup Application JavaScript

// Toast notification queue
let toastQueue = [];
let toastContainer = null;

// Initialize on DOM ready
document.addEventListener('DOMContentLoaded', function() {
    console.log('RustyHashBackup UI loaded');

    // Create toast container
    toastContainer = document.createElement('div');
    toastContainer.id = 'toast-container';
    toastContainer.className = 'fixed top-4 right-4 z-50 space-y-2';
    document.body.appendChild(toastContainer);

    // Initialize HTMX event listeners
    setupHTMXListeners();
});

// Setup HTMX event listeners
function setupHTMXListeners() {
    // Success/Error handling
    document.body.addEventListener('htmx:afterRequest', function(evt) {
        if (evt.detail.successful) {
            // Check if response has JSON
            if (evt.detail.xhr.getResponseHeader('Content-Type')?.includes('application/json')) {
                try {
                    const response = JSON.parse(evt.detail.xhr.responseText);
                    if (response.success !== undefined) {
                        if (response.success) {
                            showToast(response.message || 'Operation completed successfully', 'success');
                        } else {
                            showToast(response.message || 'Operation failed', 'error');
                        }
                    }
                } catch (e) {
                    // Not JSON or parsing failed, ignore
                }
            }
        } else if (evt.detail.failed) {
            showToast('Request failed. Please try again.', 'error');
        }
    });

    // Network error handling
    document.body.addEventListener('htmx:sendError', function(evt) {
        showToast('Network error. Please check your connection.', 'error');
    });

    // Timeout handling
    document.body.addEventListener('htmx:timeout', function(evt) {
        showToast('Request timed out. Please try again.', 'error');
    });

    // Before request (optional loading state)
    document.body.addEventListener('htmx:beforeRequest', function(evt) {
        console.log('HTMX request started:', evt.detail.path);
    });
}

// Show toast notification
function showToast(message, type = 'info', duration = 4000) {
    const toast = document.createElement('div');
    toast.className = `toast-enter px-6 py-3 rounded-lg shadow-lg text-white flex items-center space-x-3 min-w-[300px] max-w-md`;

    // Set color based on type
    const colors = {
        success: 'bg-green-600',
        error: 'bg-red-600',
        warning: 'bg-yellow-600',
        info: 'bg-blue-600'
    };
    toast.classList.add(colors[type] || colors.info);

    // Icon
    const icon = document.createElement('div');
    icon.className = 'flex-shrink-0';
    icon.innerHTML = getToastIcon(type);

    // Message
    const messageEl = document.createElement('div');
    messageEl.className = 'flex-1 font-medium';
    messageEl.textContent = message;

    // Close button
    const closeBtn = document.createElement('button');
    closeBtn.className = 'flex-shrink-0 ml-2 text-white hover:text-gray-200';
    closeBtn.innerHTML = `
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"></path>
        </svg>
    `;
    closeBtn.onclick = () => removeToast(toast);

    toast.appendChild(icon);
    toast.appendChild(messageEl);
    toast.appendChild(closeBtn);

    toastContainer.appendChild(toast);

    // Auto-remove after duration
    setTimeout(() => {
        removeToast(toast);
    }, duration);
}

// Remove toast with animation
function removeToast(toast) {
    toast.classList.remove('toast-enter');
    toast.classList.add('toast-exit');
    setTimeout(() => {
        if (toast.parentNode) {
            toast.parentNode.removeChild(toast);
        }
    }, 300);
}

// Get icon for toast type
function getToastIcon(type) {
    const icons = {
        success: `
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
        `,
        error: `
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
        `,
        warning: `
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path>
            </svg>
        `,
        info: `
            <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path>
            </svg>
        `
    };
    return icons[type] || icons.info;
}

// Format bytes to human-readable format
function formatBytes(bytes) {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return Math.round(bytes / Math.pow(k, i) * 100) / 100 + ' ' + sizes[i];
}

// Format relative time
function formatRelativeTime(timestamp) {
    const now = new Date();
    const then = new Date(timestamp);
    const diff = now - then;

    const seconds = Math.floor(diff / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (days > 0) {
        return `${days} day${days > 1 ? 's' : ''} ago`;
    } else if (hours > 0) {
        return `${hours} hour${hours > 1 ? 's' : ''} ago`;
    } else if (minutes > 0) {
        return `${minutes} minute${minutes > 1 ? 's' : ''} ago`;
    } else {
        return `${seconds} second${seconds > 1 ? 's' : ''} ago`;
    }
}

// Calculate percentage
function calculatePercentage(current, total) {
    if (total === 0) return 0;
    return Math.round((current / total) * 100);
}

// Status color mapping
const statusColors = {
    'idle': 'gray',
    'running': 'blue',
    'completed': 'green',
    'failed': 'red',
    'stopping': 'yellow'
};

// Get status color class
function getStatusColor(status) {
    return statusColors[status.toLowerCase()] || 'gray';
}

// Confirm dialog for destructive actions
function confirmAction(message, callback) {
    if (confirm(message)) {
        callback();
    }
}

// Export functions for use in HTML
window.RustyHashBackup = {
    showToast,
    formatBytes,
    formatRelativeTime,
    calculatePercentage,
    getStatusColor,
    confirmAction
};
