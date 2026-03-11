//! Global console logging — accessible from any crate.
//!
//! Messages are pushed into a lock-free shared buffer and drained each frame
//! by the editor console panel (or any other consumer).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

/// Maximum number of log entries to keep.
pub const MAX_LOG_ENTRIES: usize = 1000;

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
}

/// Thread-safe shared log buffer.
#[derive(Clone, Default)]
pub struct SharedLogBuffer(pub Arc<Mutex<VecDeque<LogEntry>>>);

impl SharedLogBuffer {
    pub fn push(&self, entry: LogEntry) {
        if let Ok(mut buffer) = self.0.lock() {
            buffer.push_back(entry);
            while buffer.len() > MAX_LOG_ENTRIES {
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
