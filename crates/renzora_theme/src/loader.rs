//! Theme loading and management

use bevy::prelude::*;
use std::path::{Path, PathBuf};

use super::Theme;

/// Resource for managing editor themes
#[derive(Resource)]
pub struct ThemeManager {
    /// The currently active theme
    pub active_theme: Theme,

    /// Name of the active theme (for persistence)
    pub active_theme_name: String,

    /// Available theme names (includes "Dark" and "Light" built-ins plus custom themes)
    pub available_themes: Vec<String>,

    /// Path to the project's themes directory (if any)
    themes_dir: Option<PathBuf>,

    /// Folder of the active theme, for folder-style themes
    /// (`themes/<Name>/theme.toml`). `None` for built-ins and flat `.toml`
    /// themes. Lets consumers resolve theme-relative assets — shaders, fonts —
    /// that ship *inside* the theme folder.
    active_theme_dir: Option<PathBuf>,

    /// Whether the active theme has unsaved changes
    pub has_unsaved_changes: bool,
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self {
            active_theme: Theme::dark(),
            active_theme_name: "Dark".to_string(),
            available_themes: vec!["Dark".to_string(), "Light".to_string()],
            themes_dir: None,
            active_theme_dir: None,
            has_unsaved_changes: false,
        }
    }
}

impl ThemeManager {
    /// Create a new ThemeManager with the default dark theme
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the project themes directory and scan for custom themes
    pub fn set_project_path(&mut self, project_path: &Path) {
        let themes_dir = project_path.join("themes");
        self.themes_dir = Some(themes_dir.clone());
        self.scan_themes();
    }

    /// Scan the themes directory for available custom themes. Two layouts are
    /// supported: a flat `themes/<Name>.toml`, and a folder `themes/<Name>/` that
    /// contains `theme.toml` (plus the theme's own shaders/fonts). A folder takes
    /// precedence over a flat file of the same name.
    pub fn scan_themes(&mut self) {
        // Start with built-in themes
        self.available_themes = vec!["Dark".to_string(), "Light".to_string()];

        // Add custom themes from project directory (not available on WASM)
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(themes_dir) = &self.themes_dir {
            if themes_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(themes_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        // Folder theme: `<Name>/theme.toml`.
                        let name = if path.is_dir() {
                            if path.join("theme.toml").exists() {
                                path.file_name().and_then(|s| s.to_str())
                            } else {
                                None
                            }
                        } else if path.extension().and_then(|s| s.to_str()) == Some("toml") {
                            path.file_stem().and_then(|s| s.to_str())
                        } else {
                            None
                        };
                        if let Some(name) = name {
                            // Don't duplicate built-ins or a name already added
                            // (a folder and a flat file can share a name).
                            if name != "Dark"
                                && name != "Light"
                                && !self.available_themes.iter().any(|n| n == name)
                            {
                                self.available_themes.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    /// Folder of the active theme (folder-style themes only). `None` for built-ins
    /// and flat `.toml` themes. Use it to resolve theme-relative assets.
    pub fn active_theme_dir(&self) -> Option<&Path> {
        self.active_theme_dir.as_deref()
    }

    /// Load a theme by name
    /// Returns true if successful
    pub fn load_theme(&mut self, name: &str) -> bool {
        match name {
            "Dark" => {
                self.active_theme = Theme::dark();
                self.active_theme_name = "Dark".to_string();
                self.active_theme_dir = None;
                self.has_unsaved_changes = false;
                true
            }
            "Light" => {
                self.active_theme = Theme::light();
                self.active_theme_name = "Light".to_string();
                self.active_theme_dir = None;
                self.has_unsaved_changes = false;
                true
            }
            _ => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if let Some(themes_dir) = &self.themes_dir {
                        // Folder theme first: `<Name>/theme.toml`, remembering the
                        // folder so its shaders/fonts can be resolved.
                        let folder = themes_dir.join(name);
                        let folder_toml = folder.join("theme.toml");
                        if folder_toml.exists() {
                            if let Some(theme) = Self::load_theme_from_file(&folder_toml) {
                                self.active_theme = theme;
                                self.active_theme_name = name.to_string();
                                self.active_theme_dir = Some(folder);
                                self.has_unsaved_changes = false;
                                return true;
                            }
                        }
                        // Fall back to a flat `<Name>.toml` (no theme folder).
                        let path = themes_dir.join(format!("{}.toml", name));
                        if let Some(theme) = Self::load_theme_from_file(&path) {
                            self.active_theme = theme;
                            self.active_theme_name = name.to_string();
                            self.active_theme_dir = None;
                            self.has_unsaved_changes = false;
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    /// Load a theme from a TOML file
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load_theme_from_file(path: &Path) -> Option<Theme> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| eprintln!("Failed to read theme file {:?}: {}", path, e))
            .ok()?;
        toml::from_str(&content)
            .map_err(|e| eprintln!("Failed to parse theme file {:?}: {}", path, e))
            .ok()
    }

    /// Save the current theme to a file
    /// Returns the path if successful
    pub fn save_theme(&mut self, name: &str) -> Option<PathBuf> {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = name;
            return None;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let themes_dir = self.themes_dir.as_ref()?;

            // Create themes directory if it doesn't exist
            if !themes_dir.exists() {
                std::fs::create_dir_all(themes_dir).ok()?;
            }

            let path = themes_dir.join(format!("{}.toml", name));

            // Update theme metadata
            self.active_theme.meta.name = name.to_string();

            // Serialize and write
            let content = toml::to_string_pretty(&self.active_theme).ok()?;
            std::fs::write(&path, content).ok()?;

            // Update state
            self.active_theme_name = name.to_string();
            self.has_unsaved_changes = false;

            // Rescan to pick up new theme
            self.scan_themes();

            Some(path)
        }
    }

    /// Mark the theme as having unsaved changes
    pub fn mark_modified(&mut self) {
        self.has_unsaved_changes = true;
    }

    /// Check if a theme name is a built-in theme
    pub fn is_builtin(&self, name: &str) -> bool {
        name == "Dark" || name == "Light"
    }

    /// Get the path to a custom theme file
    #[allow(dead_code)]
    pub fn get_theme_path(&self, name: &str) -> Option<PathBuf> {
        if self.is_builtin(name) {
            return None;
        }
        self.themes_dir
            .as_ref()
            .map(|dir| dir.join(format!("{}.toml", name)))
    }

    /// Delete a custom theme
    /// Returns true if successful
    #[allow(dead_code)]
    pub fn delete_theme(&mut self, name: &str) -> bool {
        if self.is_builtin(name) {
            return false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.get_theme_path(name) {
            if std::fs::remove_file(&path).is_ok() {
                self.scan_themes();

                // If we deleted the active theme, switch to dark
                if self.active_theme_name == name {
                    self.load_theme("Dark");
                }
                return true;
            }
        }
        false
    }

    /// Duplicate a theme with a new name
    #[allow(dead_code)]
    pub fn duplicate_theme(&mut self, new_name: &str) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = new_name;
            return false;
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut theme = self.active_theme.clone();
            theme.meta.name = new_name.to_string();

            let themes_dir = match &self.themes_dir {
                Some(dir) => dir,
                None => return false,
            };

            // Create themes directory if needed
            if !themes_dir.exists()
                && std::fs::create_dir_all(themes_dir).is_err() {
                    return false;
                }

            let path = themes_dir.join(format!("{}.toml", new_name));

            // Check if name already exists
            if path.exists() {
                return false;
            }

            // Save the duplicated theme
            if let Ok(content) = toml::to_string_pretty(&theme) {
                if std::fs::write(&path, content).is_ok() {
                    self.scan_themes();
                    return true;
                }
            }
            false
        }
    }
}
