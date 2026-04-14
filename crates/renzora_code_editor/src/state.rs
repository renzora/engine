use std::path::PathBuf;

/// An open script/file tab in the code editor.
#[derive(Clone)]
pub struct OpenFile {
    pub path: PathBuf,
    pub name: String,
    pub content: String,
    pub is_modified: bool,
    pub error: Option<ScriptError>,
    pub last_checked_content: String,
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
    pub find_focus_requested: bool,
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
            find_focus_requested: false,
        }
    }
}

impl CodeEditorState {
    /// Find the next match of `find_text` in the active file's content starting from `from`.
    /// Returns the byte index of the match start.
    pub fn find_next_in(content: &str, needle: &str, from: usize, case_sensitive: bool) -> Option<usize> {
        if needle.is_empty() || from > content.len() {
            return None;
        }
        if case_sensitive {
            content[from..].find(needle).map(|i| from + i)
                .or_else(|| content[..from.min(content.len())].find(needle))
        } else {
            let hay_lower = content.to_lowercase();
            let needle_lower = needle.to_lowercase();
            hay_lower[from..].find(&needle_lower).map(|i| from + i)
                .or_else(|| hay_lower[..from.min(hay_lower.len())].find(&needle_lower))
        }
    }

    /// Replace all occurrences in active file. Returns count replaced.
    pub fn replace_all_active(&mut self) -> usize {
        let Some(idx) = self.active_tab else { return 0 };
        let Some(file) = self.open_files.get_mut(idx) else { return 0 };
        if self.find_text.is_empty() {
            return 0;
        }
        let (new_content, count) = if self.find_case_sensitive {
            let c = file.content.matches(&self.find_text).count();
            (file.content.replace(&self.find_text, &self.replace_text), c)
        } else {
            replace_all_case_insensitive(&file.content, &self.find_text, &self.replace_text)
        };
        if count > 0 {
            file.content = new_content;
            file.is_modified = true;
        }
        count
    }
}

fn replace_all_case_insensitive(haystack: &str, needle: &str, replacement: &str) -> (String, usize) {
    if needle.is_empty() {
        return (haystack.to_string(), 0);
    }
    let hay_lower = haystack.to_lowercase();
    let needle_lower = needle.to_lowercase();
    let mut out = String::with_capacity(haystack.len());
    let mut i = 0;
    let mut count = 0;
    while i <= hay_lower.len() {
        if let Some(pos) = hay_lower[i..].find(&needle_lower) {
            let abs = i + pos;
            out.push_str(&haystack[i..abs]);
            out.push_str(replacement);
            i = abs + needle.len();
            count += 1;
        } else {
            out.push_str(&haystack[i..]);
            break;
        }
    }
    (out, count)
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

        // Read from disk
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("Failed to read file: {}", e);
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
        let Some(idx) = self.active_tab else { return };
        let Some(file) = self.open_files.get_mut(idx) else { return };
        match std::fs::write(&file.path, &file.content) {
            Ok(_) => {
                file.is_modified = false;
                log::info!("Saved: {}", file.path.display());
            }
            Err(e) => log::error!("Failed to save: {}", e),
        }
    }
}
