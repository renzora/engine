use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy_egui::egui::Color32;
use egui_phosphor::regular;

/// View mode for the asset browser content pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

/// Internal state for the asset browser panel.
pub struct AssetBrowserState {
    /// Current folder displayed in the file grid.
    pub current_folder: Option<PathBuf>,
    /// Set of expanded folders in the tree.
    pub expanded_folders: HashSet<PathBuf>,
    /// Currently selected file or folder.
    pub selected_path: Option<PathBuf>,
    /// Search/filter text.
    pub search: String,
    /// Grid zoom factor (0.5–1.5).
    pub zoom: f32,
    /// Width of the folder tree pane.
    pub tree_width: f32,
    /// Cached project root directory.
    pub project_root: Option<PathBuf>,
    /// Navigation history for back button.
    pub history: Vec<PathBuf>,
    /// Current view mode (grid or list).
    pub view_mode: ViewMode,
}

impl Default for AssetBrowserState {
    fn default() -> Self {
        Self {
            current_folder: None,
            expanded_folders: HashSet::new(),
            selected_path: None,
            search: String::new(),
            zoom: 1.0,
            tree_width: 200.0,
            project_root: None,
            history: Vec::new(),
            view_mode: ViewMode::default(),
        }
    }
}

impl AssetBrowserState {
    /// Get or initialize the project root (uses current working directory).
    pub fn root(&mut self) -> PathBuf {
        if let Some(ref root) = self.project_root {
            return root.clone();
        }
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        self.project_root = Some(root.clone());
        root
    }

    /// Navigate to a folder, pushing the previous folder onto the history stack.
    pub fn navigate_to(&mut self, folder: PathBuf) {
        if let Some(ref current) = self.current_folder {
            self.history.push(current.clone());
        }
        self.current_folder = Some(folder);
    }

    /// Go back to the previous folder.
    pub fn go_back(&mut self) {
        if let Some(prev) = self.history.pop() {
            self.current_folder = Some(prev);
        }
    }

    /// Go to the project root.
    pub fn go_home(&mut self) {
        let root = self.root();
        if self.current_folder.as_ref() != Some(&root) {
            if let Some(ref current) = self.current_folder {
                self.history.push(current.clone());
            }
            self.current_folder = Some(root);
        }
    }
}

// ── File type detection ─────────────────────────────────────────────────────

/// Returns a Phosphor icon and accent color for a given file path.
pub fn file_icon(path: &Path) -> (&'static str, Color32) {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let lower = filename.to_lowercase();

    // Special compound extensions
    if lower.ends_with(".blueprint") || lower.ends_with(".bp") {
        return (regular::BLUEPRINT, Color32::from_rgb(100, 180, 255));
    }
    if lower.ends_with(".material_bp") {
        return (regular::ATOM, Color32::from_rgb(255, 120, 200));
    }
    if lower.ends_with(".ron") {
        return (regular::FILM_SCRIPT, Color32::from_rgb(115, 200, 255));
    }
    if lower.ends_with(".video") {
        return (regular::VIDEO, Color32::from_rgb(220, 80, 80));
    }
    if lower.ends_with(".particle") {
        return (regular::SPARKLE, Color32::from_rgb(255, 180, 50));
    }
    if lower.ends_with(".level") {
        return (regular::GAME_CONTROLLER, Color32::from_rgb(100, 200, 180));
    }
    if lower.ends_with(".terrain") {
        return (regular::MOUNTAINS, Color32::from_rgb(140, 180, 100));
    }
    if lower.ends_with(".anim") {
        return (regular::FILM_SCRIPT, Color32::from_rgb(100, 180, 220));
    }

    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        // Scripts
        "rhai" => (regular::CODE, Color32::from_rgb(130, 230, 180)),
        "lua" => (regular::CODE, Color32::from_rgb(80, 130, 230)),
        "js" | "ts" => (regular::CODE, Color32::from_rgb(240, 220, 80)),

        // Shaders
        "wgsl" | "glsl" | "vert" | "frag" => (regular::GRAPHICS_CARD, Color32::from_rgb(220, 120, 255)),

        // Rust
        "rs" => (regular::FILE_RS, Color32::from_rgb(255, 130, 80)),

        // Images
        "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" => (regular::IMAGE, Color32::from_rgb(150, 230, 130)),
        "hdr" | "exr" => (regular::SUN, Color32::from_rgb(255, 220, 100)),

        // 3D Models
        "gltf" | "glb" | "obj" | "fbx" | "usd" | "usdz" => (regular::CUBE, Color32::from_rgb(255, 170, 100)),

        // Audio
        "wav" | "ogg" | "mp3" | "flac" | "opus" => (regular::MUSIC_NOTES, Color32::from_rgb(200, 130, 230)),

        // Video
        "mp4" | "avi" | "mov" | "webm" => (regular::VIDEO, Color32::from_rgb(230, 100, 100)),

        // Config
        "json" => (regular::STACK, Color32::from_rgb(180, 180, 200)),
        "toml" => (regular::GEAR, Color32::from_rgb(180, 180, 200)),
        "yaml" | "yml" => (regular::STACK, Color32::from_rgb(180, 180, 200)),

        // Text/docs
        "txt" => (regular::FILE_TEXT, Color32::from_rgb(180, 180, 200)),
        "md" => (regular::NOTE, Color32::from_rgb(180, 200, 220)),

        _ => (regular::FILE, Color32::from_rgb(150, 150, 165)),
    }
}

/// Returns a color for folder icons based on folder name.
pub fn folder_icon_color(name: &str) -> Color32 {
    match name.to_lowercase().as_str() {
        "assets" => Color32::from_rgb(255, 210, 100),
        "scenes" => Color32::from_rgb(100, 180, 255),
        "scripts" => Color32::from_rgb(130, 230, 180),
        "blueprints" => Color32::from_rgb(100, 180, 255),
        "materials" => Color32::from_rgb(255, 130, 200),
        "textures" | "images" => Color32::from_rgb(150, 230, 130),
        "models" | "meshes" => Color32::from_rgb(255, 170, 100),
        "audio" | "sounds" | "music" => Color32::from_rgb(200, 130, 230),
        "prefabs" => Color32::from_rgb(130, 180, 255),
        "src" => Color32::from_rgb(255, 130, 80),
        "shaders" => Color32::from_rgb(180, 130, 255),
        _ => Color32::from_rgb(170, 175, 190),
    }
}

pub fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.starts_with('.'))
        .unwrap_or(false)
}
