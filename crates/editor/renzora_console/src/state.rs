//! Console state — editor-side resource that collects and displays log entries.
//!
//! Log types, shared buffer, and global logging functions live in
//! `renzora_core::console_log` so every crate can use them.

use bevy::prelude::*;
use std::collections::VecDeque;

// Re-export core logging types so existing `renzora_console::state::*` imports keep working.
pub use renzora_core::console_log::{
    console_log, get_global_log_buffer, init_global_log_buffer, LogEntry, LogLevel,
    SharedLogBuffer, MAX_LOG_ENTRIES,
};

/// Resource for the console state.
#[derive(Resource)]
pub struct ConsoleState {
    pub entries: VecDeque<LogEntry>,
    pub shared_buffer: SharedLogBuffer,
    pub show_info: bool,
    pub show_success: bool,
    pub show_warnings: bool,
    pub show_errors: bool,
    pub auto_scroll: bool,
    pub search_filter: String,
    pub category_filter: String,
    pub input_buffer: String,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub saved_input: String,
    pub focus_input: bool,
}

impl Default for ConsoleState {
    fn default() -> Self {
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
            input_buffer: String::new(),
            command_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            focus_input: false,
        }
    }
}

impl ConsoleState {
    /// Add a log entry directly.
    pub fn log(&mut self, level: LogLevel, category: impl Into<String>, message: impl Into<String>) {
        let entry = LogEntry {
            level,
            message: message.into(),
            timestamp: 0.0,
            category: category.into(),
        };
        self.entries.push_back(entry);
        while self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Drain entries from the shared buffer.
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

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get filtered entries.
    pub fn filtered_entries(&self) -> impl Iterator<Item = &LogEntry> {
        self.entries.iter().filter(|entry| {
            let level_ok = match entry.level {
                LogLevel::Info => self.show_info,
                LogLevel::Success => self.show_success,
                LogLevel::Warning => self.show_warnings,
                LogLevel::Error => self.show_errors,
            };

            if !level_ok {
                return false;
            }

            if !self.category_filter.is_empty()
                && !entry.category.to_lowercase().contains(&self.category_filter.to_lowercase())
            {
                return false;
            }

            if !self.search_filter.is_empty()
                && !entry.message.to_lowercase().contains(&self.search_filter.to_lowercase())
            {
                return false;
            }

            true
        })
    }
}

/// Helper functions for logging from anywhere in the editor.
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
