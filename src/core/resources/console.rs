//! Console system for displaying logs in the editor

#![allow(dead_code)]

use bevy::prelude::*;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

/// Maximum number of log entries to keep
const MAX_LOG_ENTRIES: usize = 1000;

/// Log level for console messages
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
            LogLevel::Info => "\u{f129}",     // info icon
            LogLevel::Success => "\u{f00c}",  // check icon
            LogLevel::Warning => "\u{f071}",  // warning triangle
            LogLevel::Error => "\u{f00d}",    // x icon
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

/// A single log entry
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: f64,
    pub category: String,
}

/// Shared log buffer that can be written to from anywhere
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

/// Global log buffer for logging from anywhere
static GLOBAL_LOG_BUFFER: std::sync::OnceLock<SharedLogBuffer> = std::sync::OnceLock::new();

/// Initialize the global log buffer (called once at startup)
pub fn init_global_log_buffer() -> SharedLogBuffer {
    let buffer = SharedLogBuffer::default();
    let _ = GLOBAL_LOG_BUFFER.set(buffer.clone());
    buffer
}

/// Get the global log buffer
pub fn get_global_log_buffer() -> Option<&'static SharedLogBuffer> {
    GLOBAL_LOG_BUFFER.get()
}

/// Log a message to the global console (can be called from anywhere)
pub fn console_log(level: LogLevel, category: &str, message: impl Into<String>) {
    if let Some(buffer) = get_global_log_buffer() {
        buffer.push(LogEntry {
            level,
            message: message.into(),
            timestamp: 0.0, // Will be set when drained
            category: category.to_string(),
        });
    }
}

/// Convenience macros for logging
#[macro_export]
macro_rules! console_info {
    ($cat:expr, $($arg:tt)*) => {
        $crate::core::resources::console::console_log(
            $crate::core::LogLevel::Info,
            $cat,
            format!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! console_success {
    ($cat:expr, $($arg:tt)*) => {
        $crate::core::resources::console::console_log(
            $crate::core::LogLevel::Success,
            $cat,
            format!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! console_warn {
    ($cat:expr, $($arg:tt)*) => {
        $crate::core::resources::console::console_log(
            $crate::core::LogLevel::Warning,
            $cat,
            format!($($arg)*)
        )
    };
}

#[macro_export]
macro_rules! console_error {
    ($cat:expr, $($arg:tt)*) => {
        $crate::core::resources::console::console_log(
            $crate::core::LogLevel::Error,
            $cat,
            format!($($arg)*)
        )
    };
}

/// Resource for the console state
#[derive(Resource)]
pub struct ConsoleState {
    /// Log entries
    pub entries: VecDeque<LogEntry>,
    /// Shared buffer for receiving logs from tracing
    pub shared_buffer: SharedLogBuffer,
    /// Filter settings
    pub show_info: bool,
    pub show_success: bool,
    pub show_warnings: bool,
    pub show_errors: bool,
    /// Auto-scroll to bottom
    pub auto_scroll: bool,
    /// Search filter
    pub search_filter: String,
    /// Category filter (empty = show all)
    pub category_filter: String,
}

impl Default for ConsoleState {
    fn default() -> Self {
        // Initialize or get the global buffer
        let shared_buffer = init_global_log_buffer();

        Self {
            entries: VecDeque::new(),
            shared_buffer,
            show_info: true,
            show_success: true,
            show_warnings: true,
            show_errors: true,
            auto_scroll: true,
            search_filter: String::new(),
            category_filter: String::new(),
        }
    }
}

impl ConsoleState {
    /// Add a log entry directly
    pub fn log(&mut self, level: LogLevel, category: impl Into<String>, message: impl Into<String>) {
        let entry = LogEntry {
            level,
            message: message.into(),
            timestamp: 0.0, // Will be set by the system
            category: category.into(),
        };
        self.entries.push_back(entry);
        while self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Drain entries from the shared buffer
    pub fn drain_shared_buffer(&mut self, time: f64) {
        if let Ok(mut buffer) = self.shared_buffer.0.lock() {
            for mut entry in buffer.drain(..) {
                entry.timestamp = time;
                self.entries.push_back(entry);
            }
            while self.entries.len() > MAX_LOG_ENTRIES {
                self.entries.pop_front();
            }
        }
    }

    /// Clear all entries
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get filtered entries
    pub fn filtered_entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().filter(|entry| {
            // Level filter
            let level_ok = match entry.level {
                LogLevel::Info => self.show_info,
                LogLevel::Success => self.show_success,
                LogLevel::Warning => self.show_warnings,
                LogLevel::Error => self.show_errors,
            };

            if !level_ok {
                return false;
            }

            // Category filter
            if !self.category_filter.is_empty()
                && !entry.category.to_lowercase().contains(&self.category_filter.to_lowercase())
            {
                return false;
            }

            // Search filter
            if !self.search_filter.is_empty()
                && !entry.message.to_lowercase().contains(&self.search_filter.to_lowercase())
            {
                return false;
            }

            true
        })
    }
}

/// Helper functions for logging from anywhere in the editor
pub fn log_info(console: &mut ConsoleState, category: &str, message: impl Into<String>) {
    console.log(LogLevel::Info, category, message);
}

pub fn log_success(console: &mut ConsoleState, category: &str, message: impl Into<String>) {
    console.log(LogLevel::Success, category, message);
}

pub fn log_warning(console: &mut ConsoleState, category: &str, message: impl Into<String>) {
    console.log(LogLevel::Warning, category, message);
}

pub fn log_error(console: &mut ConsoleState, category: &str, message: impl Into<String>) {
    console.log(LogLevel::Error, category, message);
}
