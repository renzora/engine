use std::collections::HashSet;
use std::path::{Path, PathBuf};

use bevy_egui::egui::{self, Color32};
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
    /// Currently selected file or folder (kept for compatibility).
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
    /// Set to `true` when the import button is clicked (consumed by the panel).
    pub import_clicked: bool,

    // === Multi-selection ===
    /// All selected items (for multi-selection).
    pub selected_assets: HashSet<PathBuf>,
    /// Anchor for Shift+click range selection.
    pub selection_anchor: Option<PathBuf>,
    /// Item order in current view for range selection.
    pub visible_item_order: Vec<PathBuf>,

    // === Inline rename ===
    /// Asset being renamed.
    pub renaming_asset: Option<PathBuf>,
    /// Text input buffer for rename.
    pub rename_buffer: String,
    /// Track focus request for rename TextEdit.
    pub rename_focus_set: bool,

    // === Marquee/drag selection ===
    /// Start position of drag selection.
    pub marquee_start: Option<egui::Pos2>,
    /// Current drag position.
    pub marquee_current: Option<egui::Pos2>,
    /// Item positions for hit testing.
    pub item_rects: Vec<(PathBuf, egui::Rect)>,
    /// Selection state saved when marquee started (so items leaving the marquee get deselected).
    pub pre_marquee_selection: HashSet<PathBuf>,

    // === Context menu ===
    /// Context menu open position (None = closed).
    pub context_menu_pos: Option<egui::Pos2>,

    // === File drops from desktop ===
    /// Files dropped from the OS that need to be copied into the target folder.
    pub pending_file_imports: Vec<PathBuf>,
    /// True when OS files are hovering over the panel.
    pub drop_hover: bool,
    /// Target folder for file drops (set when hovering over a tree folder).
    pub drop_target_folder: Option<PathBuf>,
    /// Rects of tree folder rows for drop hit-testing.
    pub tree_folder_rects: Vec<(PathBuf, egui::Rect)>,

    // === Pending operations ===
    /// Pending rename operation (old_path, new_name).
    pub pending_rename: Option<(PathBuf, String)>,
    /// Pending delete operation.
    pub pending_delete: Vec<PathBuf>,
    /// Last error message.
    pub last_error: Option<String>,
    /// Error auto-clear timer.
    pub error_timeout: f32,

    // === Create dialogs ===
    pub show_create_folder_dialog: bool,
    pub new_folder_name: String,
    pub show_create_script_dialog: bool,
    pub new_script_name: String,
    pub show_create_scene_dialog: bool,
    pub new_scene_name: String,
    pub show_create_material_dialog: bool,
    pub new_material_name: String,
    pub show_create_shader_dialog: bool,
    pub new_shader_name: String,
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
            import_clicked: false,
            selected_assets: HashSet::new(),
            selection_anchor: None,
            visible_item_order: Vec::new(),
            renaming_asset: None,
            rename_buffer: String::new(),
            rename_focus_set: false,
            marquee_start: None,
            marquee_current: None,
            item_rects: Vec::new(),
            pre_marquee_selection: HashSet::new(),
            context_menu_pos: None,
            pending_file_imports: Vec::new(),
            drop_hover: false,
            drop_target_folder: None,
            tree_folder_rects: Vec::new(),
            pending_rename: None,
            pending_delete: Vec::new(),
            last_error: None,
            error_timeout: 0.0,
            show_create_folder_dialog: false,
            new_folder_name: String::new(),
            show_create_script_dialog: false,
            new_script_name: String::new(),
            show_create_scene_dialog: false,
            new_scene_name: String::new(),
            show_create_material_dialog: false,
            new_material_name: String::new(),
            show_create_shader_dialog: false,
            new_shader_name: String::new(),
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
        self.selected_assets.clear();
        self.selected_path = None;
        self.selection_anchor = None;
    }

    /// Returns true if the given path is selected.
    pub fn is_selected(&self, path: &Path) -> bool {
        self.selected_assets.contains(path)
    }

    /// Handle click on an item with modifier key support.
    pub fn handle_click(&mut self, path: &Path, ctrl: bool, shift: bool) {
        if ctrl {
            // Toggle selection
            let p = path.to_path_buf();
            if self.selected_assets.contains(&p) {
                self.selected_assets.remove(&p);
                self.selected_path = self.selected_assets.iter().next().cloned();
            } else {
                self.selected_assets.insert(p.clone());
                self.selected_path = Some(p);
            }
        } else if shift {
            // Range selection using visible_item_order
            if let Some(ref anchor) = self.selection_anchor.clone() {
                let anchor_idx = self.visible_item_order.iter().position(|p| p == anchor);
                let current_idx = self.visible_item_order.iter().position(|p| p == path);
                if let (Some(start), Some(end)) = (anchor_idx, current_idx) {
                    let (start, end) = if start <= end { (start, end) } else { (end, start) };
                    self.selected_assets.clear();
                    for idx in start..=end {
                        if let Some(p) = self.visible_item_order.get(idx) {
                            self.selected_assets.insert(p.clone());
                        }
                    }
                    self.selected_path = Some(path.to_path_buf());
                }
            } else {
                self.selected_assets.clear();
                self.selected_assets.insert(path.to_path_buf());
                self.selection_anchor = Some(path.to_path_buf());
                self.selected_path = Some(path.to_path_buf());
            }
        } else {
            // Single select — clear others
            self.selected_assets.clear();
            self.selected_assets.insert(path.to_path_buf());
            self.selection_anchor = Some(path.to_path_buf());
            self.selected_path = Some(path.to_path_buf());
        }
    }

    /// Clear all selection.
    pub fn clear_selection(&mut self) {
        self.selected_assets.clear();
        self.selected_path = None;
        self.selection_anchor = None;
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
    if lower.ends_with(".material_bp") || lower.ends_with(".material") {
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

/// File extensions that can be imported by simple copy (non-3D assets).
/// 3D models (gltf, glb, obj, fbx, etc.) are handled by `renzora_import_ui`.
const COPYABLE_EXTENSIONS: &[&str] = &[
    // Images
    "png", "jpg", "jpeg", "bmp", "tga", "webp", "hdr", "exr",
    // Audio
    "wav", "ogg", "mp3", "flac", "opus",
    // Video
    "mp4", "avi", "mov", "webm",
    // Scripts
    "rhai", "lua", "js", "ts",
    // Shaders
    "wgsl", "glsl", "vert", "frag",
    // Data
    "json", "toml", "yaml", "yml", "ron", "txt", "md",
    // Engine formats
    "blueprint", "bp", "material", "material_bp", "anim",
    "video", "particle", "level", "terrain", "texture",
];

/// Returns true if this file extension can be imported by simple copy.
pub fn is_copyable_asset(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            let lower = ext.to_lowercase();
            COPYABLE_EXTENSIONS.contains(&lower.as_str())
        })
        .unwrap_or(false)
}

/// Returns true if this file is any kind of importable asset (copy or 3D model).
pub fn is_droppable_file(path: &Path) -> bool {
    if is_copyable_asset(path) {
        return true;
    }
    // 3D model formats handled by renzora_import
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(),
            "gltf" | "glb" | "obj" | "stl" | "ply" | "fbx" | "usd" | "usdz"
        ))
        .unwrap_or(false)
}

/// Returns true if this file is a 3D model that needs conversion (not a simple copy).
pub fn is_3d_model(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|ext| matches!(ext.to_lowercase().as_str(),
            "gltf" | "glb" | "obj" | "stl" | "ply" | "fbx" | "usd" | "usdz"
        ))
        .unwrap_or(false)
}

/// Files that should be hidden from the asset browser.
const HIDDEN_FILES: &[&str] = &["project.toml"];

pub fn is_hidden(path: &Path) -> bool {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
    name.starts_with('.')
        || HIDDEN_FILES.iter().any(|&h| name.eq_ignore_ascii_case(h))
}
