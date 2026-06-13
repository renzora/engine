//! Console state — editor-side resource that collects and displays log entries.
//!
//! Log types, shared buffer, and global logging functions live in
//! `renzora::core::console_log` so every crate can use them.

use bevy::prelude::*;
use std::collections::{HashSet, VecDeque};

// Re-export core logging types so existing `renzora_console::state::*` imports keep working.
pub use renzora::core::console_log::{
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
    /// Categories that have been explicitly hidden by the user.
    pub hidden_categories: HashSet<String>,
    /// All categories ever seen (for building the filter UI).
    pub seen_categories: Vec<String>,
    pub show_timestamps: bool,
    pub show_frame: bool,
    pub frame_counter: u64,
    pub input_buffer: String,
    pub command_history: Vec<String>,
    pub history_index: Option<usize>,
    pub saved_input: String,
    pub focus_input: bool,
    /// Monotonic count of entries ever appended (never decreases, even as the
    /// ring buffer drops old entries). Lets a retained renderer append only the
    /// new rows each frame instead of rebuilding the whole list.
    pub pushed: u64,
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
            hidden_categories: HashSet::new(),
            seen_categories: Vec::new(),
            show_timestamps: false,
            show_frame: false,
            frame_counter: 0,
            input_buffer: String::new(),
            command_history: Vec::new(),
            history_index: None,
            saved_input: String::new(),
            focus_input: false,
            pushed: 0,
        }
    }
}

impl ConsoleState {
    /// Add a log entry directly.
    pub fn log(
        &mut self,
        level: LogLevel,
        category: impl Into<String>,
        message: impl Into<String>,
    ) {
        let cat = category.into();
        if !cat.is_empty() && !self.seen_categories.contains(&cat) {
            self.seen_categories.push(cat.clone());
        }
        self.push_entry(LogEntry {
            level,
            message: message.into(),
            timestamp: 0.0,
            frame: self.frame_counter,
            category: cat,
            count: 1,
        });
    }

    /// Append an entry, coalescing it into the previous one when it's an exact
    /// repeat (same level/category/message) — Chrome-devtools style. A merge
    /// bumps the existing entry's `count` and does **not** advance `pushed`, so
    /// the row keeps its keyed-list key and only its badge re-renders (a
    /// per-frame repeat costs one row update, never an unbounded append).
    fn push_entry(&mut self, entry: LogEntry) {
        if let Some(last) = self.entries.back_mut() {
            if last.level == entry.level
                && last.category == entry.category
                && last.message == entry.message
            {
                last.count = last.count.saturating_add(entry.count);
                last.timestamp = entry.timestamp;
                last.frame = entry.frame;
                return;
            }
        }
        self.entries.push_back(entry);
        self.pushed += 1;
        while self.entries.len() > MAX_LOG_ENTRIES {
            self.entries.pop_front();
        }
    }

    /// Drain entries from the shared buffer.
    pub fn drain_shared_buffer(&mut self, time: f64, frame: u64) {
        self.frame_counter = frame;
        // Take everything out under the lock, then release it before coalescing
        // (push_entry needs &mut self, which conflicts with the guard's borrow).
        let drained: Vec<LogEntry> = match self.shared_buffer.0.lock() {
            Ok(mut buffer) => buffer.drain(..).collect(),
            Err(_) => return,
        };
        for mut entry in drained {
            entry.timestamp = time;
            entry.frame = frame;
            // Track seen categories
            if !entry.category.is_empty() && !self.seen_categories.contains(&entry.category) {
                self.seen_categories.push(entry.category.clone());
            }
            self.push_entry(entry);
        }
    }

    /// Clear all entries. Resets `pushed` so a retained renderer detects the
    /// reset (its cursor will be ahead of the new `pushed`) and rebuilds.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.pushed = 0;
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

            // Hide entries whose category has been explicitly hidden
            if !entry.category.is_empty() && self.hidden_categories.contains(&entry.category) {
                return false;
            }

            if !self.category_filter.is_empty()
                && !entry
                    .category
                    .to_lowercase()
                    .contains(&self.category_filter.to_lowercase())
            {
                return false;
            }

            if !self.search_filter.is_empty()
                && !entry
                    .message
                    .to_lowercase()
                    .contains(&self.search_filter.to_lowercase())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_appends_entry_with_level_and_message() {
        let mut state = ConsoleState::default();
        state.entries.clear(); // Default may have stray entries from a shared buffer.
        state.log(LogLevel::Info, "test", "hello");
        assert_eq!(state.entries.len(), 1);
        let entry = &state.entries[0];
        assert!(matches!(entry.level, LogLevel::Info));
        assert_eq!(entry.message, "hello");
        assert_eq!(entry.category, "test");
    }

    #[test]
    fn log_tracks_seen_categories_uniquely() {
        let mut state = ConsoleState::default();
        state.seen_categories.clear();
        state.log(LogLevel::Info, "alpha", "1");
        state.log(LogLevel::Info, "alpha", "2"); // duplicate category
        state.log(LogLevel::Info, "beta", "3");
        assert_eq!(
            state.seen_categories,
            vec!["alpha".to_string(), "beta".to_string()]
        );
    }

    #[test]
    fn log_skips_empty_category_in_seen_list() {
        let mut state = ConsoleState::default();
        state.seen_categories.clear();
        state.log(LogLevel::Info, "", "no category");
        assert!(state.seen_categories.is_empty());
    }

    #[test]
    fn log_caps_at_max_entries() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        // Push a few past the cap; oldest should fall out.
        for i in 0..(MAX_LOG_ENTRIES + 5) {
            state.log(LogLevel::Info, "cat", format!("msg-{}", i));
        }
        assert_eq!(state.entries.len(), MAX_LOG_ENTRIES);
        // First entry kept should be one we pushed mid-loop, not msg-0.
        assert_ne!(state.entries.front().unwrap().message, "msg-0");
    }

    #[test]
    fn clear_empties_entries() {
        let mut state = ConsoleState::default();
        state.log(LogLevel::Info, "x", "y");
        state.clear();
        assert!(state.entries.is_empty());
    }

    #[test]
    fn filter_hides_disabled_levels() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        state.log(LogLevel::Info, "c", "info-msg");
        state.log(LogLevel::Warning, "c", "warn-msg");
        state.log(LogLevel::Error, "c", "err-msg");
        state.show_warnings = false;
        let visible: Vec<&str> = state
            .filtered_entries()
            .map(|e| e.message.as_str())
            .collect();
        assert_eq!(visible, vec!["info-msg", "err-msg"]);
    }

    #[test]
    fn filter_hides_explicitly_hidden_categories() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        state.log(LogLevel::Info, "alpha", "a");
        state.log(LogLevel::Info, "beta", "b");
        state.hidden_categories.insert("alpha".into());
        let visible: Vec<&str> = state
            .filtered_entries()
            .map(|e| e.message.as_str())
            .collect();
        assert_eq!(visible, vec!["b"]);
    }

    #[test]
    fn filter_category_substring_is_case_insensitive() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        state.log(LogLevel::Info, "Renderer", "1");
        state.log(LogLevel::Info, "physics", "2");
        state.category_filter = "REND".into();
        let visible: Vec<&str> = state
            .filtered_entries()
            .map(|e| e.message.as_str())
            .collect();
        assert_eq!(visible, vec!["1"]);
    }

    #[test]
    fn filter_search_substring_is_case_insensitive() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        state.log(LogLevel::Info, "c", "Loading scene 'foo.ron'");
        state.log(LogLevel::Info, "c", "spawned 12 entities");
        state.search_filter = "LOAD".into();
        let visible: Vec<&str> = state
            .filtered_entries()
            .map(|e| e.message.as_str())
            .collect();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].contains("Loading"));
    }

    #[test]
    fn empty_filters_show_everything_visible_by_level() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        state.log(LogLevel::Info, "a", "1");
        state.log(LogLevel::Info, "b", "2");
        state.log(LogLevel::Info, "", "3");
        // No filters set, all default-on levels.
        assert_eq!(state.filtered_entries().count(), 3);
    }

    #[test]
    fn helper_log_functions_set_correct_level() {
        let mut state = ConsoleState::default();
        state.entries.clear();
        log_info(&mut state, "h", "i");
        log_success(&mut state, "h", "s");
        log_warning(&mut state, "h", "w");
        log_error(&mut state, "h", "e");
        let levels: Vec<_> = state.entries.iter().map(|e| e.level).collect();
        assert!(matches!(levels[0], LogLevel::Info));
        assert!(matches!(levels[1], LogLevel::Success));
        assert!(matches!(levels[2], LogLevel::Warning));
        assert!(matches!(levels[3], LogLevel::Error));
    }
}
