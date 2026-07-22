//! Global console logging — accessible from any crate.
//!
//! Messages are pushed into a lock-free shared buffer and drained each frame
//! by the editor console panel (or any other consumer).

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Default cap on retained log entries. Kept deliberately small: the editor
/// console panel spawns one UI row per entry, so a large backlog adds per-frame
/// layout/render work and drops frames. Users can raise it in
/// Settings → Developer → Console Log Limit.
pub const DEFAULT_MAX_LOG_ENTRIES: usize = 100;

/// Live cap, seeded to [`DEFAULT_MAX_LOG_ENTRIES`] and overridable at runtime
/// from the editor setting (see [`set_max_log_entries`]). Atomic because
/// [`SharedLogBuffer::push`] is called from any thread and must read the cap
/// without taking a lock.
static MAX_LOG_ENTRIES: AtomicUsize = AtomicUsize::new(DEFAULT_MAX_LOG_ENTRIES);

/// The current retained-entry cap.
pub fn max_log_entries() -> usize {
    MAX_LOG_ENTRIES.load(Ordering::Relaxed)
}

/// Set the retained-entry cap (floored at 1). Called by the editor when the
/// Console Log Limit setting loads or changes; the smaller buffer takes effect
/// on the next push (existing entries above the new cap trim as fresh logs
/// arrive).
pub fn set_max_log_entries(limit: usize) {
    MAX_LOG_ENTRIES.store(limit.max(1), Ordering::Relaxed);
}

/// Log level for console messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl LogLevel {
    pub fn icon(&self) -> &'static str {
        match self {
            LogLevel::Info => "\u{f129}",
            LogLevel::Success => "\u{f00c}",
            LogLevel::Warning => "\u{f071}",
            LogLevel::Error => "\u{f00d}",
        }
    }

    pub fn color(&self) -> [u8; 3] {
        match self {
            LogLevel::Info => [140, 180, 220],
            LogLevel::Success => [100, 200, 120],
            LogLevel::Warning => [230, 180, 80],
            LogLevel::Error => [220, 80, 80],
        }
    }
}

/// A single log entry.
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: f64,
    pub frame: u64,
    pub category: String,
    /// How many consecutive identical entries this represents. Repeated lines
    /// (same level/category/message) collapse into a single entry with this
    /// bumped — Chrome-devtools style — so a per-frame log can't flood the
    /// buffer or stall the frame. `timestamp`/`frame` track the latest hit.
    pub count: u32,
}

/// Thread-safe shared log buffer.
#[derive(Clone, Default)]
pub struct SharedLogBuffer(pub Arc<Mutex<VecDeque<LogEntry>>>);

impl SharedLogBuffer {
    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut buffer) = self.0.lock() {
            // Coalesce a run of identical lines into one entry with a count,
            // so repeated/per-frame logs increment a badge instead of growing
            // the buffer (and the panel) unboundedly.
            if let Some(last) = buffer.back_mut() {
                if last.level == entry.level
                    && last.category == entry.category
                    && last.message == entry.message
                {
                    last.count = last.count.saturating_add(1);
                    last.timestamp = entry.timestamp;
                    last.frame = entry.frame;
                    return;
                }
            }
            buffer.push_back(entry);
            let cap = max_log_entries();
            while buffer.len() > cap {
                buffer.pop_front();
            }
        }
    }
}

/// Global log buffer singleton.
static GLOBAL_LOG_BUFFER: OnceLock<SharedLogBuffer> = OnceLock::new();

/// Initialize the global log buffer (called once at startup).
pub fn init_global_log_buffer() -> SharedLogBuffer {
    let buffer = SharedLogBuffer::default();
    let _ = GLOBAL_LOG_BUFFER.set(buffer.clone());
    buffer
}

/// Get the global log buffer.
pub fn get_global_log_buffer() -> Option<&'static SharedLogBuffer> {
    GLOBAL_LOG_BUFFER.get()
}

/// Log a message to the global console (can be called from anywhere).
pub fn console_log(level: LogLevel, category: &str, message: impl Into<String>) {
    if let Some(buffer) = get_global_log_buffer() {
        buffer.push(LogEntry {
            level,
            message: message.into(),
            timestamp: 0.0,
            frame: 0,
            category: category.to_string(),
            count: 1,
        });
    }
}

/// Convenience: log info.
pub fn console_info(category: &str, message: impl Into<String>) {
    console_log(LogLevel::Info, category, message);
}

/// Convenience: log success.
pub fn console_success(category: &str, message: impl Into<String>) {
    console_log(LogLevel::Success, category, message);
}

/// Convenience: log warning.
pub fn console_warn(category: &str, message: impl Into<String>) {
    console_log(LogLevel::Warning, category, message);
}

/// Convenience: log error.
pub fn console_error(category: &str, message: impl Into<String>) {
    console_log(LogLevel::Error, category, message);
}

// ── Macros ──────────────────────────────────────────────────────────────────

#[macro_export]
macro_rules! clog_info {
    ($cat:expr, $($arg:tt)*) => {
        $crate::console_log::console_info($cat, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! clog_success {
    ($cat:expr, $($arg:tt)*) => {
        $crate::console_log::console_success($cat, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! clog_warn {
    ($cat:expr, $($arg:tt)*) => {
        $crate::console_log::console_warn($cat, format!($($arg)*))
    };
}

#[macro_export]
macro_rules! clog_error {
    ($cat:expr, $($arg:tt)*) => {
        $crate::console_log::console_error($cat, format!($($arg)*))
    };
}
