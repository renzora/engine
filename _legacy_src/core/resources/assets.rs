#![allow(dead_code)]

use bevy::prelude::*;
use bevy_egui::egui;
use std::collections::HashSet;
use std::path::PathBuf;

/// State for the asset browser panel
#[derive(Resource, Default)]
pub struct AssetBrowserState {
    /// Current folder being viewed in assets panel
    pub current_folder: Option<PathBuf>,
    /// Currently selected asset (kept for compatibility)
    pub selected_asset: Option<PathBuf>,
    /// Asset being dragged (single, kept for compatibility with viewport drops)
    pub dragging_asset: Option<PathBuf>,

    // === Multi-selection ===
    /// All selected items (for multi-selection)
    pub selected_assets: HashSet<PathBuf>,
    /// Anchor for Shift+click range selection
    pub selection_anchor: Option<PathBuf>,
    /// Item order in current view for range selection
    pub visible_item_order: Vec<PathBuf>,

    // === Inline rename ===
    /// Asset being renamed
    pub renaming_asset: Option<PathBuf>,
    /// Text input buffer for rename
    pub rename_buffer: String,
    /// Track focus request for rename TextEdit
    pub rename_focus_set: bool,

    // === Drag-drop move ===
    /// Assets being dragged (multi-selection aware)
    pub dragging_assets: Vec<PathBuf>,
    /// Folder hover target for drop
    pub drop_target_folder: Option<PathBuf>,

    // === Marquee/drag selection ===
    /// Start position of drag selection
    pub marquee_start: Option<egui::Pos2>,
    /// Current drag position
    pub marquee_current: Option<egui::Pos2>,
    /// Item positions for hit testing
    pub item_rects: Vec<(PathBuf, egui::Rect)>,

    // === Operations ===
    /// Pending rename operation (old_path, new_name)
    pub pending_rename: Option<(PathBuf, String)>,
    /// Pending move operation (source_paths, target_folder)
    pub pending_move: Option<(Vec<PathBuf>, PathBuf)>,
    /// Last error message
    pub last_error: Option<String>,
    /// Error auto-clear timer
    pub error_timeout: f32,
    /// Pending asset drop (path, 3D position) - for viewport drops
    pub pending_asset_drop: Option<(PathBuf, Vec3)>,
    /// Files to import to assets folder (dropped in assets panel, NOT to spawn in scene)
    pub pending_file_imports: Vec<PathBuf>,
    /// Bounds of the assets panel [x, y, width, height] for detecting drops
    pub panel_bounds: [f32; 4],
    /// Files that should be spawned in the scene (dropped in viewport, queued by UI)
    pub files_to_spawn: Vec<PathBuf>,
    /// Pending image drop (path, position, is_2d_mode) - for image drops to viewport
    pub pending_image_drop: Option<PendingImageDrop>,
    /// Pending scene drop to hierarchy (scene path, parent entity)
    pub pending_scene_drop: Option<(PathBuf, Option<Entity>)>,
    /// Pending material blueprint drop (path, cursor position for picking)
    pub pending_material_drop: Option<PendingMaterialDrop>,
    /// Pending HDR/EXR file to apply as skybox (from viewport drop)
    pub pending_skybox_drop: Option<PathBuf>,
    /// Pending .particle file drop to viewport (path, position)
    pub pending_effect_drop: Option<(PathBuf, Vec3)>,
    /// Pending audio file drop to viewport (path, position) → spawns Audio Player entity
    pub pending_audio_drop: Option<(PathBuf, Vec3)>,
    /// Pending script/blueprint drops from hierarchy drag (script path, target entity)
    pub pending_script_drops: Vec<(PathBuf, Entity)>,
    /// Pending audio file drop to hierarchy (audio path, optional parent entity)
    pub pending_audio_hierarchy_drop: Option<(PathBuf, Option<Entity>)>,
    /// Search filter text
    pub search: String,
    /// Current view mode (grid or list)
    pub view_mode: AssetViewMode,
    /// Zoom level for grid view
    pub zoom: f32,
    /// Show create script dialog
    pub show_create_script_dialog: bool,
    /// New script name being entered
    pub new_script_name: String,
    /// Request to import an asset
    pub import_asset_requested: bool,
    /// Show create folder dialog
    pub show_create_folder_dialog: bool,
    /// New folder name being entered
    pub new_folder_name: String,
    /// Show create material dialog
    pub show_create_material_dialog: bool,
    /// New material name being entered
    pub new_material_name: String,
    /// Show create scene dialog
    pub show_create_scene_dialog: bool,
    /// New scene name being entered
    pub new_scene_name: String,
    /// Show create video project dialog
    pub show_create_video_dialog: bool,
    /// New video project name being entered
    pub new_video_name: String,
    /// Show create audio project dialog
    pub show_create_audio_dialog: bool,
    /// New audio project name being entered
    pub new_audio_name: String,
    /// Show create animation dialog
    pub show_create_animation_dialog: bool,
    /// New animation name being entered
    pub new_animation_name: String,
    /// Show create texture dialog
    pub show_create_texture_dialog: bool,
    /// New texture name being entered
    pub new_texture_name: String,
    /// Show create particle FX dialog
    pub show_create_particle_dialog: bool,
    /// New particle FX name being entered
    pub new_particle_name: String,
    /// Show create level dialog
    pub show_create_level_dialog: bool,
    /// New level name being entered
    pub new_level_name: String,
    /// Show create terrain dialog
    pub show_create_terrain_dialog: bool,
    /// New terrain name being entered
    pub new_terrain_name: String,
    /// Show create shader dialog
    pub show_create_shader_dialog: bool,
    /// New shader name being entered
    pub new_shader_name: String,
    /// Show create material blueprint dialog
    pub show_create_material_blueprint_dialog: bool,
    /// New material blueprint name being entered
    pub new_material_blueprint_name: String,
    /// Show create script blueprint dialog
    pub show_create_script_blueprint_dialog: bool,
    /// New script blueprint name being entered
    pub new_script_blueprint_name: String,
    /// Context menu open state and position
    pub context_menu_pos: Option<bevy::math::Vec2>,
    /// Currently open submenu in context menu
    pub context_submenu: Option<String>,
    /// Requested layout switch (processed by main UI loop)
    pub requested_layout: Option<String>,
    /// Pending blueprint to open (processed by main UI loop)
    pub pending_blueprint_open: Option<PathBuf>,
    /// Show model import settings dialog
    pub show_import_dialog: bool,
    /// Import settings state
    pub import_settings: ModelImportSettings,
    /// Files pending import (selected via file dialog)
    pub pending_import_files: Vec<PathBuf>,
    /// Import status tracking
    pub import_status: ImportStatus,
    /// Expanded folders in tree view
    pub expanded_folders: HashSet<PathBuf>,
    /// Width of the tree panel in split view
    pub tree_panel_width: f32,
    /// Ground position under cursor during model drag (updated every frame by viewport)
    pub drag_ground_position: Option<Vec3>,
    /// Surface hit position during drag (set by raycast system, overrides ground plane)
    pub drag_surface_position: Option<Vec3>,
    /// Surface normal at drag hit point (Y-up when on ground plane)
    pub drag_surface_normal: Vec3,
    /// Pending shape drop from Shape Library panel (mesh type, 3D position)
    pub pending_shape_drop: Option<(crate::component_system::MeshPrimitiveType, Vec3)>,
    /// Surface normal at shape drop point (for proper placement on angled surfaces)
    pub pending_shape_drop_normal: Vec3,
}

/// Pending image drop information
#[derive(Clone, Debug)]
pub struct PendingImageDrop {
    /// Path to the image file
    pub path: PathBuf,
    /// 3D position (for 3D mode) or 2D position (for 2D mode)
    pub position: Vec3,
    /// Whether this drop is in 2D mode
    pub is_2d_mode: bool,
}

/// Pending material blueprint drop information
#[derive(Clone, Debug)]
pub struct PendingMaterialDrop {
    /// Path to the .material_bp file
    pub path: PathBuf,
    /// Cursor position in viewport coordinates for entity picking
    pub cursor_pos: bevy::math::Vec2,
}

/// Settings for importing 3D models
#[derive(Clone, Debug)]
pub struct ModelImportSettings {
    // === Transform ===
    /// Scale factor to apply to imported models
    pub scale: f32,
    /// Rotation offset in degrees (X, Y, Z)
    pub rotation_offset: (f32, f32, f32),
    /// Translation offset
    pub translation_offset: (f32, f32, f32),
    /// Whether to flip Y and Z coordinates (some formats use different up axes)
    pub convert_axes: ConvertAxes,

    // === Mesh ===
    /// How to handle mesh extraction
    pub mesh_handling: MeshHandling,
    /// Whether to combine meshes into a single mesh
    pub combine_meshes: bool,
    /// Whether to generate LODs automatically
    pub generate_lods: bool,
    /// Number of LOD levels to generate
    pub lod_count: u32,
    /// LOD reduction percentage per level
    pub lod_reduction: f32,

    // === Normals & Tangents ===
    /// How to handle normals
    pub normal_import: NormalImportMethod,
    /// How to handle tangents
    pub tangent_import: TangentImportMethod,
    /// Smoothing angle for computed normals (degrees)
    pub smoothing_angle: f32,

    // === Materials & Textures ===
    /// Whether to import materials
    pub import_materials: bool,
    /// Whether to extract and copy textures
    pub extract_textures: bool,
    /// Texture extraction subfolder name
    pub texture_subfolder: String,
    /// Whether to import vertex colors
    pub import_vertex_colors: bool,

    // === Animation ===
    /// Whether to import animations
    pub import_animations: bool,
    /// Whether to import as skeletal mesh
    pub import_as_skeletal: bool,
    /// Whether to import skeleton/bones
    pub import_skeleton: bool,

    // === Compression ===
    /// Whether to apply Draco compression (for glTF export)
    pub draco_compression: bool,
    /// Draco compression level (0-10, higher = smaller file, slower)
    pub draco_compression_level: u32,
    /// Draco quantization bits for positions (8-16)
    pub draco_position_bits: u32,
    /// Draco quantization bits for normals (8-16)
    pub draco_normal_bits: u32,
    /// Draco quantization bits for UVs (8-16)
    pub draco_uv_bits: u32,

    // === Physics ===
    /// Whether to generate collision shapes
    pub generate_colliders: bool,
    /// Type of collider to generate
    pub collider_type: ColliderImportType,
    /// Whether to use a simplified mesh for collision
    pub simplify_collision: bool,
    /// Collision mesh simplification ratio (0.0-1.0)
    pub collision_simplification: f32,

    // === Lightmapping ===
    /// Whether to generate lightmap UVs
    pub generate_lightmap_uvs: bool,
    /// Lightmap UV channel index
    pub lightmap_uv_channel: u32,
    /// Minimum lightmap resolution
    pub lightmap_resolution: u32,
}

impl Default for ModelImportSettings {
    fn default() -> Self {
        Self {
            // Transform
            scale: 1.0,
            rotation_offset: (0.0, 0.0, 0.0),
            translation_offset: (0.0, 0.0, 0.0),
            convert_axes: ConvertAxes::None,

            // Mesh
            mesh_handling: MeshHandling::KeepHierarchy,
            combine_meshes: false,
            generate_lods: false,
            lod_count: 3,
            lod_reduction: 50.0,

            // Normals & Tangents
            normal_import: NormalImportMethod::Import,
            tangent_import: TangentImportMethod::Import,
            smoothing_angle: 60.0,

            // Materials & Textures
            import_materials: true,
            extract_textures: true,
            texture_subfolder: "textures".to_string(),
            import_vertex_colors: true,

            // Animation
            import_animations: true,
            import_as_skeletal: false,
            import_skeleton: true,

            // Compression
            draco_compression: false,
            draco_compression_level: 7,
            draco_position_bits: 14,
            draco_normal_bits: 10,
            draco_uv_bits: 12,

            // Physics
            generate_colliders: false,
            collider_type: ColliderImportType::ConvexHull,
            simplify_collision: true,
            collision_simplification: 0.5,

            // Lightmapping
            generate_lightmap_uvs: false,
            lightmap_uv_channel: 1,
            lightmap_resolution: 64,
        }
    }
}

impl ModelImportSettings {
    /// Auto-configure defaults based on source file extension.
    pub fn apply_format_defaults(&mut self, ext: &str) {
        match ext {
            "fbx" => {
                // FBX files typically use Z-Up (Blender, Maya, 3ds Max)
                self.convert_axes = ConvertAxes::ZUpToYUp;
            }
            "obj" => {
                // OBJ is ambiguous but many exporters use Y-Up already
                self.convert_axes = ConvertAxes::None;
            }
            "usd" | "usdz" => {
                // USD uses Y-Up by default
                self.convert_axes = ConvertAxes::None;
            }
            _ => {}
        }
    }

    /// Return a human-readable description for the format.
    pub fn format_description(ext: &str) -> &'static str {
        match ext {
            "fbx" => "FBX (Autodesk) — typically Z-Up. Axis conversion auto-enabled.",
            "obj" => "OBJ (Wavefront) — Y-Up by convention. Sidecar .mtl + textures will be copied.",
            "usd" | "usdz" => "USD (Universal Scene Description) — Y-Up by default.",
            "glb" | "gltf" => "glTF/GLB — native engine format. No conversion needed.",
            _ => "Unknown format",
        }
    }
}

/// How to handle coordinate system conversion
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConvertAxes {
    /// No conversion
    #[default]
    None,
    /// Convert from Z-up to Y-up (Blender, 3ds Max default)
    ZUpToYUp,
    /// Convert from Y-up to Z-up
    YUpToZUp,
    /// Flip X axis
    FlipX,
    /// Flip Z axis (front/back)
    FlipZ,
}

/// How to handle mesh extraction from the source file
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MeshHandling {
    /// Keep the original hierarchy, reference the source file
    #[default]
    KeepHierarchy,
    /// Extract each mesh as a separate asset file
    ExtractMeshes,
    /// Flatten hierarchy but keep meshes separate
    FlattenHierarchy,
    /// Combine all meshes into a single mesh asset
    CombineAll,
}

/// How to handle normal vectors during import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum NormalImportMethod {
    /// Import normals from file
    #[default]
    Import,
    /// Compute normals (smooth)
    ComputeSmooth,
    /// Compute normals (flat/faceted)
    ComputeFlat,
    /// Import and recompute tangent space
    ImportAndRecompute,
}

/// How to handle tangent vectors during import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TangentImportMethod {
    /// Import tangents from file
    #[default]
    Import,
    /// Compute tangents using MikkTSpace algorithm
    ComputeMikkTSpace,
    /// Don't import tangents
    None,
}

/// Type of collider to generate on import
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ColliderImportType {
    /// Convex hull collider (faster, less accurate)
    #[default]
    ConvexHull,
    /// Trimesh collider (slower, more accurate)
    Trimesh,
    /// Axis-aligned bounding box
    AABB,
    /// Oriented bounding box
    OBB,
    /// Capsule (auto-fit)
    Capsule,
    /// Sphere (auto-fit)
    Sphere,
    /// Use decomposed convex hulls (V-HACD)
    Decomposed,
    /// Use simplified mesh
    SimplifiedMesh,
}

impl AssetBrowserState {
    pub fn new() -> Self {
        Self {
            zoom: 1.0,
            tree_panel_width: 220.0,
            ..Default::default()
        }
    }

    /// Navigate to a folder
    pub fn navigate_to(&mut self, path: PathBuf) {
        self.current_folder = Some(path);
        self.selected_asset = None;
    }

    /// Navigate up one folder level
    pub fn navigate_up(&mut self) {
        if let Some(current) = &self.current_folder {
            if let Some(parent) = current.parent() {
                self.current_folder = Some(parent.to_path_buf());
                self.selected_asset = None;
            }
        }
    }

    /// Select an asset
    pub fn select(&mut self, path: PathBuf) {
        self.selected_asset = Some(path);
    }

    /// Clear selection
    pub fn clear_selection(&mut self) {
        self.selected_asset = None;
    }

    /// Start dragging an asset
    pub fn start_drag(&mut self, path: PathBuf) {
        self.dragging_asset = Some(path);
    }

    /// End dragging
    pub fn end_drag(&mut self) {
        self.dragging_asset = None;
    }

    /// Check if search matches a filename
    pub fn matches_search(&self, filename: &str) -> bool {
        if self.search.is_empty() {
            return true;
        }
        filename.to_lowercase().contains(&self.search.to_lowercase())
    }
}

/// Result of importing a single file
#[derive(Clone, Debug)]
pub struct ImportFileResult {
    pub filename: String,
    pub success: bool,
    pub message: String,
    pub output_size: Option<u64>,
}

/// Status tracker for batch imports
#[derive(Clone, Debug, Default)]
pub struct ImportStatus {
    pub results: Vec<ImportFileResult>,
    pub completed: bool,
    pub show_results: bool,
}

/// View mode for asset browser
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum AssetViewMode {
    #[default]
    Grid,
    List,
}
