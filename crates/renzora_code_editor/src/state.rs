use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use renzora_editor::MonoFont;

/// An open script/file tab in the code editor.
#[derive(Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub is_modified: bool,
    pub error: Option<ScriptError>,
    pub last_checked_content: String,
    /// Previous cursor byte-index. Used so auto-scroll only fires when the
    /// cursor actually moved, instead of every frame the editor has focus
    /// (which was fighting the user's manual scroll).
    pub last_cursor_index: Option<usize>,
    /// 0-based line numbers with a breakpoint set in the gutter.
    pub breakpoints: HashSet<usize>,
    /// Active folds keyed by the fold's start line (0-based, in the
    /// *currently visible* content). The value is the text that was extracted
    /// from the buffer when the fold was created — restored on unfold or save.
    pub folds: HashMap<usize, String>,
}

/// A script compilation error.
#[derive(Clone)]
pub struct ScriptError {
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

/// Font size limits for zoom.
const MIN_FONT_SIZE: f32 = 8.0;
const MAX_FONT_SIZE: f32 = 40.0;
const DEFAULT_FONT_SIZE: f32 = 16.0;

/// Shared state resource for the code editor.
#[derive(Clone, bevy::prelude::Resource)]
pub struct CodeEditorState {
    pub open_files: Vec<OpenFile>,
    pub active_tab: Option<usize>,
    pub font_size: f32,
    pub find_open: bool,
    pub find_text: String,
    pub replace_text: String,
    pub find_case_sensitive: bool,
    pub find_whole_word: bool,
    pub find_use_regex: bool,
    pub find_focus_requested: bool,

    // Go-to-line dialog
    pub goto_line_open: bool,
    pub goto_line_buffer: String,
    pub goto_line_focus_requested: bool,
    /// 1-based line number to scroll to on the next frame.
    pub pending_goto_line: Option<usize>,

    // Autocomplete popup
    pub autocomplete_open: bool,
    /// The word prefix being completed (scanned back from the cursor).
    pub autocomplete_filter: String,
    /// Byte offset where the prefix starts in the file's content.
    pub autocomplete_prefix_start: usize,
    /// Highlighted row in the popup.
    pub autocomplete_selected: usize,
    /// Anchor position (bottom-left of the screen-space popup).
    pub autocomplete_anchor: Option<bevy_egui::egui::Pos2>,
    /// Set by a popup row click; consumed the same frame to insert + close.
    pub autocomplete_click_commit: bool,

    // Visual toggles
    pub show_minimap: bool,
    pub show_whitespace: bool,

    /// Consumed on the next frame to jump the scroll area to this y offset.
    pub pending_scroll_offset: Option<f32>,

    /// Index of a tab the user clicked X on while it had unsaved changes.
    /// Drives the save-or-discard confirmation modal.
    pub close_confirm_tab: Option<usize>,

    /// Strip trailing spaces/tabs from each line on save.
    pub trim_trailing_whitespace_on_save: bool,
    /// Type `(` `[` `{` `"` `'` to insert the closing pair too.
    pub auto_close_pairs: bool,

    /// Open as a modal: pick a tab to diff against the active one.
    pub diff_open: bool,
    /// Index in `open_files` of the "right" side. `None` = compare against the
    /// active file's on-disk version.
    pub diff_other_tab: Option<usize>,
    /// Right-pane tab index when split view is active. `None` = no split.
    pub split_active_tab: Option<usize>,
    /// Extra cursor positions (byte offsets) for multi-cursor edits. Empty by
    /// default; populated by Alt+click and Ctrl+D-style commands.
    pub extra_cursors: Vec<usize>,

    /// Mirror of `EditorSettings::mono_font` so the editor toolbar can swap
    /// it without pulling the resource in directly. Synced bidirectionally
    /// by `sync_code_editor_prefs_to_settings` / the lib-level setup.
    pub mono_font: MonoFont,

    /// egui reads `input.time` to drive its cursor-blink cycle. We track the
    /// wall-clock time of the last relevant keypress and, while we're inside
    /// the keep-visible window, disable the blink so the cursor stays lit —
    /// effectively "resetting" the blink on every edit.
    pub last_cursor_visible_reset: Option<f64>,
}

impl Default for CodeEditorState {
    fn default() -> Self {
        Self {
            open_files: Vec::new(),
            active_tab: None,
            font_size: DEFAULT_FONT_SIZE,
            find_open: false,
            find_text: String::new(),
            replace_text: String::new(),
            find_case_sensitive: false,
            find_whole_word: false,
            find_use_regex: false,
            find_focus_requested: false,
            goto_line_open: false,
            goto_line_buffer: String::new(),
            goto_line_focus_requested: false,
            pending_goto_line: None,
            autocomplete_open: false,
            autocomplete_filter: String::new(),
            autocomplete_prefix_start: 0,
            autocomplete_selected: 0,
            autocomplete_anchor: None,
            autocomplete_click_commit: false,
            show_minimap: true,
            show_whitespace: false,
            pending_scroll_offset: None,
            close_confirm_tab: None,
            trim_trailing_whitespace_on_save: true,
            auto_close_pairs: true,
            diff_open: false,
            diff_other_tab: None,
            split_active_tab: None,
            extra_cursors: Vec::new(),
            mono_font: MonoFont::default(),
            last_cursor_visible_reset: None,
        }
    }
}

impl CodeEditorState {
    /// Find the next match of `find_text` in the active file's content starting from `from`.
    /// Returns the byte index of the match start.
    pub fn find_next_in(content: &str, needle: &str, from: usize, case_sensitive: bool) -> Option<usize> {
        let matches = find_all_matches(content, needle, case_sensitive, false);
        if matches.is_empty() {
            return None;
        }
        // First match at-or-after `from`, otherwise wrap to the first.
        matches
            .iter()
            .find(|(s, _)| *s >= from)
            .or_else(|| matches.first())
            .map(|(s, _)| *s)
    }

    /// Replace all occurrences in active file. Returns count replaced.
    pub fn replace_all_active(&mut self) -> usize {
        let Some(idx) = self.active_tab else { return 0 };
        let Some(file) = self.open_files.get_mut(idx) else { return 0 };
        if self.find_text.is_empty() {
            return 0;
        }
        let matches = find_all_matches(
            &file.content,
            &self.find_text,
            self.find_case_sensitive,
            self.find_whole_word,
        );
        if matches.is_empty() {
            return 0;
        }
        // Apply bottom-up so earlier offsets stay valid.
        let count = matches.len();
        for (s, e) in matches.iter().rev() {
            file.content.replace_range(*s..*e, &self.replace_text);
        }
        file.is_modified = true;
        count
    }
}

/// All match ranges of `needle` in `content`. `whole_word` requires the
/// surrounding bytes to not be identifier characters.
pub fn find_all_matches(
    content: &str,
    needle: &str,
    case_sensitive: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }
    let nlen = needle.len();
    let bytes = content.as_bytes();
    let (hay, ndl): (&str, std::borrow::Cow<str>) = if case_sensitive {
        (content, std::borrow::Cow::Borrowed(needle))
    } else {
        // Stash the lowercased haystack in a longer-lived buffer via a thread
        // local? Simpler: do a lowercased scan when not case-sensitive.
        return find_all_case_insensitive(content, needle, whole_word);
    };
    let _ = (hay, ndl);

    let mut i = 0;
    while i + nlen <= bytes.len() {
        if &bytes[i..i + nlen] == needle.as_bytes() {
            if !whole_word || word_boundary_ok(bytes, i, i + nlen) {
                out.push((i, i + nlen));
                i += nlen;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn find_all_case_insensitive(
    content: &str,
    needle: &str,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let hay_lower = content.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let bytes = content.as_bytes();
    let hay_bytes = hay_lower.as_bytes();
    let ndl = needle_lower.as_bytes();
    let mut out = Vec::new();
    let nlen = ndl.len();
    if nlen == 0 || hay_bytes.len() != bytes.len() {
        // Lowercasing changed byte length (Unicode) — fall back to a simple
        // case-sensitive scan to avoid bad slices.
        return find_all_matches(content, needle, true, whole_word);
    }
    let mut i = 0;
    while i + nlen <= hay_bytes.len() {
        if &hay_bytes[i..i + nlen] == ndl {
            if !whole_word || word_boundary_ok(bytes, i, i + nlen) {
                out.push((i, i + nlen));
                i += nlen;
                continue;
            }
        }
        i += 1;
    }
    out
}

fn word_boundary_ok(bytes: &[u8], start: usize, end: usize) -> bool {
    let before_ok = start == 0
        || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
    let after_ok = end >= bytes.len()
        || !(bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_');
    before_ok && after_ok
}

/// Strip trailing spaces / tabs from each line. Preserves the original
/// trailing-newline state.
fn trim_trailing_whitespace(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut current = String::new();
    for ch in content.chars() {
        if ch == '\n' {
            out.push_str(current.trim_end_matches(|c: char| c == ' ' || c == '\t'));
            out.push('\n');
            current.clear();
        } else {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        out.push_str(current.trim_end_matches(|c: char| c == ' ' || c == '\t'));
    }
    out
}

impl CodeEditorState {
    pub fn zoom_in(&mut self) {
        self.font_size = (self.font_size + 1.0).min(MAX_FONT_SIZE);
    }

    pub fn zoom_out(&mut self) {
        self.font_size = (self.font_size - 1.0).max(MIN_FONT_SIZE);
    }

    pub fn zoom_reset(&mut self) {
        self.font_size = DEFAULT_FONT_SIZE;
    }

    /// Open a file. If already open, just switch to its tab.
    pub fn open_file(&mut self, path: PathBuf) {
        // Check if already open
        for (idx, f) in self.open_files.iter().enumerate() {
            if f.path == path {
                self.active_tab = Some(idx);
                return;
            }
        }

        // Read from disk. Missing files are common (script_path on an entity
        // can outlive the file), so log a warning instead of erroring out.
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                log::warn!("Script file not found: {}", path.display());
                return;
            }
            Err(e) => {
                log::error!("Failed to read {}: {}", path.display(), e);
                return;
            }
        };

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let content_clone = content.clone();
        self.open_files.push(OpenFile {
            path,
            name,
            content,
            is_modified: false,
            error: None,
            last_checked_content: content_clone,
            last_cursor_index: None,
            breakpoints: HashSet::new(),
            folds: HashMap::new(),
        });
        self.active_tab = Some(self.open_files.len() - 1);
    }

    /// Close a tab by index.
    pub fn close_tab(&mut self, idx: usize) {
        if idx < self.open_files.len() {
            self.open_files.remove(idx);
            if self.open_files.is_empty() {
                self.active_tab = None;
            } else if let Some(active) = self.active_tab {
                if active >= self.open_files.len() {
                    self.active_tab = Some(self.open_files.len() - 1);
                } else if active > idx {
                    self.active_tab = Some(active - 1);
                }
            }
        }
    }

    /// Create a new script file in the given directory, open it in the editor.
    pub fn create_new_script(&mut self, scripts_dir: PathBuf) {
        let _ = std::fs::create_dir_all(&scripts_dir);

        // Find a unique name
        let mut idx = 1;
        let path = loop {
            let name = if idx == 1 {
                "new_script.lua".to_string()
            } else {
                format!("new_script_{}.lua", idx)
            };
            let candidate = scripts_dir.join(&name);
            if !candidate.exists() {
                break candidate;
            }
            idx += 1;
        };

        let template = r#"-- New Script

function on_ready(ctx, vars)
    -- Called once when the script is first attached
end

function on_update(ctx, vars)
    -- Called every frame
end
"#;

        match std::fs::write(&path, template) {
            Ok(_) => {
                log::info!("Created new script: {}", path.display());
                self.open_file(path);
                // Mark as modified so the user knows to rename/save
                if let Some(file) = self.active_tab.and_then(|i| self.open_files.get_mut(i)) {
                    file.is_modified = true;
                }
            }
            Err(e) => log::error!("Failed to create script: {}", e),
        }
    }

    /// Save the active file to disk.
    pub fn save_active(&mut self) {
        if let Some(idx) = self.active_tab {
            self.save_file(idx);
        }
    }

    /// Save the file at `idx` to disk. Any active folds are restored first
    /// so the disk copy always has the full content.
    pub fn save_file(&mut self, idx: usize) {
        let trim = self.trim_trailing_whitespace_on_save;
        let Some(file) = self.open_files.get_mut(idx) else { return };
        restore_all_folds(file);
        if trim {
            let cleaned = trim_trailing_whitespace(&file.content);
            if cleaned != file.content {
                file.content = cleaned;
            }
        }
        match std::fs::write(&file.path, &file.content) {
            Ok(_) => {
                file.is_modified = false;
                log::info!("Saved: {}", file.path.display());
            }
            Err(e) => log::error!("Failed to save {}: {}", file.path.display(), e),
        }
    }

    /// Close every tab except `keep_idx`. Modified tabs are preserved so
    /// changes don't disappear silently.
    pub fn close_others(&mut self, keep_idx: usize) {
        let mut new_files = Vec::new();
        let mut new_active = None;
        for (i, f) in std::mem::take(&mut self.open_files).into_iter().enumerate() {
            if i == keep_idx || f.is_modified {
                if i == keep_idx {
                    new_active = Some(new_files.len());
                }
                new_files.push(f);
            }
        }
        self.open_files = new_files;
        self.active_tab = if self.open_files.is_empty() {
            None
        } else {
            new_active.or(Some(0))
        };
    }

    /// Close every tab. Modified tabs are preserved.
    pub fn close_all(&mut self) {
        let new_files: Vec<OpenFile> = std::mem::take(&mut self.open_files)
            .into_iter()
            .filter(|f| f.is_modified)
            .collect();
        self.active_tab = if new_files.is_empty() { None } else { Some(0) };
        self.open_files = new_files;
    }

    /// Cycle to the next tab.
    pub fn next_tab(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        let n = self.open_files.len();
        let cur = self.active_tab.unwrap_or(0);
        self.active_tab = Some((cur + 1) % n);
    }

    /// Cycle to the previous tab.
    pub fn prev_tab(&mut self) {
        if self.open_files.is_empty() {
            return;
        }
        let n = self.open_files.len();
        let cur = self.active_tab.unwrap_or(0);
        self.active_tab = Some((cur + n - 1) % n);
    }

    /// Save every modified open file to disk. Folds are restored first so
    /// disk always sees the full content.
    pub fn save_all(&mut self) {
        let mut saved = 0usize;
        for file in self.open_files.iter_mut() {
            if !file.is_modified {
                continue;
            }
            restore_all_folds(file);
            match std::fs::write(&file.path, &file.content) {
                Ok(_) => {
                    file.is_modified = false;
                    saved += 1;
                }
                Err(e) => log::error!("Failed to save {}: {}", file.path.display(), e),
            }
        }
        if saved > 0 {
            log::info!("Saved {} modified file(s)", saved);
        }
    }
}

/// Restore every active fold back into `file.content`. Folds are replayed in
/// descending start-line order so earlier restorations don't invalidate
/// later anchors.
pub fn restore_all_folds(file: &mut OpenFile) {
    if file.folds.is_empty() {
        return;
    }
    let mut keys: Vec<usize> = file.folds.keys().copied().collect();
    keys.sort_unstable_by(|a, b| b.cmp(a));
    for start in keys {
        if let Some(text) = file.folds.remove(&start) {
            let insert_at = end_of_line_with_newline(&file.content, start);
            if insert_at <= file.content.len() {
                file.content.insert_str(insert_at, &text);
            }
        }
    }
}

/// Byte offset just past the end of `line` (including its trailing newline
/// when present). Returns `content.len()` for lines past the last one.
fn end_of_line_with_newline(content: &str, line: usize) -> usize {
    let bytes = content.as_bytes();
    let mut seen = 0usize;
    for (i, b) in bytes.iter().enumerate() {
        if *b == b'\n' {
            if seen == line {
                return i + 1;
            }
            seen += 1;
        }
    }
    content.len()
}
