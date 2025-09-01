use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::project_manager::get_projects_path;
use crate::file_sync::sanitize_file_name;
use log::{info, warn, error};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelImportSettings {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub skeletal_meshes: SkeletalMeshSettings,
    #[serde(default)]
    pub static_meshes: StaticMeshSettings,
    #[serde(default)]
    pub animations: AnimationSettings,
    #[serde(default)]
    pub materials: MaterialSettings,
    #[serde(default)]
    pub advanced: AdvancedSettings,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GeneralSettings {
    #[serde(default)]
    pub use_source_name: bool,
    #[serde(default)]
    pub scene_name_sub_folder: bool,
    #[serde(default = "default_true")]
    pub asset_type_sub_folders: bool,
    #[serde(default)]
    pub offset_translation: [f32; 3],
    #[serde(default)]
    pub offset_rotation: [f32; 3],
    #[serde(default = "default_scale")]
    pub offset_uniform_scale: f32,
    #[serde(default = "default_mesh_type")]
    pub force_all_mesh_type: String,
    #[serde(default = "default_true")]
    pub auto_detect_mesh_type: bool,
    #[serde(default = "default_true")]
    pub import_lods: bool,
    #[serde(default)]
    pub bake_meshes: bool,
    #[serde(default)]
    pub bake_pivot_meshes: bool,
    #[serde(default = "default_true")]
    pub keep_sections_separate: bool,
    #[serde(default = "default_vertex_color")]
    pub vertex_color_import: String,
    #[serde(default = "default_white")]
    pub vertex_override_color: String,
    #[serde(default)]
    pub import_sockets: bool,
    #[serde(default = "default_true")]
    pub build: bool,
}

fn default_true() -> bool { true }
fn default_scale() -> f32 { 1.0 }
fn default_mesh_type() -> String { "none".to_string() }
fn default_vertex_color() -> String { "replace".to_string() }
fn default_white() -> String { "#ffffff".to_string() }
fn default_content_type() -> String { "geometry_and_skin_weights".to_string() }
fn default_timeline() -> String { "source_timeline".to_string() }
fn default_frame_range() -> [i32; 2] { [0, -1] }
fn default_sample_rate() -> u32 { 30 }
fn default_skeleton() -> String { "create_new".to_string() }
fn default_hdr_ext() -> String { "hdr,exr".to_string() }
fn default_units() -> String { "meters".to_string() }
fn default_axis() -> String { "y_up".to_string() }

#[derive(Debug, Serialize, Deserialize)]
pub struct SkeletalMeshSettings {
    #[serde(default = "default_true")]
    pub import_skeletal_meshes: bool,
    #[serde(default = "default_content_type")]
    pub import_content_type: String,
    #[serde(default = "default_true")]
    pub import_morph_targets: bool,
    #[serde(default)]
    pub merge_morph_targets_with_same_name: bool,
    #[serde(default = "default_true")]
    pub import_vertex_attributes: bool,
    #[serde(default)]
    pub update_skeleton_reference_pose: bool,
    #[serde(default)]
    pub create_physics_asset: bool,
    #[serde(default)]
    pub import_meshes_in_bone_hierarchy: bool,
    #[serde(default)]
    pub add_curve_metadata_to_skeleton: bool,
    #[serde(default)]
    pub convert_static_with_morph_to_skeletal: bool,
}

impl Default for SkeletalMeshSettings {
    fn default() -> Self {
        Self {
            import_skeletal_meshes: true,
            import_content_type: "geometry_and_skin_weights".to_string(),
            import_morph_targets: true,
            merge_morph_targets_with_same_name: false,
            import_vertex_attributes: true,
            update_skeleton_reference_pose: false,
            create_physics_asset: false,
            import_meshes_in_bone_hierarchy: false,
            add_curve_metadata_to_skeleton: false,
            convert_static_with_morph_to_skeletal: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticMeshSettings {
    #[serde(default = "default_true")]
    pub import_static_meshes: bool,
    #[serde(default)]
    pub combine_static_meshes: bool,
    #[serde(default = "default_mesh_type")]
    pub lod_group: String,
    #[serde(default = "default_true")]
    pub auto_compute_lod_screen_sizes: bool,
    #[serde(default)]
    pub generate_collision: bool,
}

impl Default for StaticMeshSettings {
    fn default() -> Self {
        Self {
            import_static_meshes: true,
            combine_static_meshes: false,
            lod_group: "none".to_string(),
            auto_compute_lod_screen_sizes: true,
            generate_collision: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationSettings {
    #[serde(default = "default_true")]
    pub import_animations: bool,
    #[serde(default = "default_true")]
    pub import_bone_tracks: bool,
    #[serde(default = "default_timeline")]
    pub animation_length: String,
    #[serde(default = "default_frame_range")]
    pub frame_import_range: [i32; 2],
    #[serde(default)]
    pub use_30hz_to_bake_bone_animation: bool,
    #[serde(default = "default_sample_rate")]
    pub custom_bone_animation_sample_rate: u32,
    #[serde(default)]
    pub snap_to_closest_frame_boundary: bool,
    #[serde(default = "default_true")]
    pub import_curves: bool,
    #[serde(default)]
    pub animation_only: bool,
    #[serde(default = "default_skeleton")]
    pub skeleton: String,
}

impl Default for AnimationSettings {
    fn default() -> Self {
        Self {
            import_animations: true,
            import_bone_tracks: true,
            animation_length: "source_timeline".to_string(),
            frame_import_range: [0, -1],
            use_30hz_to_bake_bone_animation: false,
            custom_bone_animation_sample_rate: 30,
            snap_to_closest_frame_boundary: false,
            import_curves: true,
            animation_only: false,
            skeleton: "create_new".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSettings {
    #[serde(default = "default_true")]
    pub import_textures: bool,
    #[serde(default = "default_true")]
    pub detect_normal_map_texture: bool,
    #[serde(default)]
    pub flip_normal_map_texture: bool,
    #[serde(default)]
    pub flip_normal_map_green_channel: bool,
    #[serde(default)]
    pub import_udims: bool,
    #[serde(default)]
    pub import_sparse_volume_textures: bool,
    #[serde(default)]
    pub import_animated_sparse_volume_textures: bool,
    #[serde(default = "default_hdr_ext")]
    pub file_extensions_for_long_lat_cubemap: String,
    #[serde(default = "default_true")]
    pub prefer_compressed_source_data: bool,
    #[serde(default = "default_true")]
    pub allow_non_power_of_two: bool,
    #[serde(default)]
    pub draco_compression: bool,
}

impl Default for MaterialSettings {
    fn default() -> Self {
        Self {
            import_textures: true,
            detect_normal_map_texture: true,
            flip_normal_map_texture: false,
            flip_normal_map_green_channel: false,
            import_udims: false,
            import_sparse_volume_textures: false,
            import_animated_sparse_volume_textures: false,
            file_extensions_for_long_lat_cubemap: "hdr,exr".to_string(),
            prefer_compressed_source_data: true,
            allow_non_power_of_two: true,
            draco_compression: false,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdvancedSettings {
    #[serde(default = "default_units")]
    pub file_units: String,
    #[serde(default = "default_axis")]
    pub file_axis_direction: String,
    #[serde(default = "default_true")]
    pub use_settings_for_subsequent_files: bool,
}

impl Default for AdvancedSettings {
    fn default() -> Self {
        Self {
            file_units: "meters".to_string(),
            file_axis_direction: "y_up".to_string(),
            use_settings_for_subsequent_files: true,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ModelProcessResult {
    pub success: bool,
    pub saved_assets: Vec<SavedAsset>,
    pub folder_structure: FolderStructure,
    pub import_summary: ImportSummary,
}

#[derive(Debug, Serialize)]
pub struct SavedAsset {
    pub asset_type: String,
    pub original_name: String,
    pub sanitized_name: String,
    pub path: String,
    pub size_bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct FolderStructure {
    pub base_path: String,
    pub created_folders: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ImportSummary {
    pub original_file: String,
    pub sanitized_file: String,
    pub imported_at: String,
    pub settings: ModelImportSettings,
    pub file_size: u64,
    pub target_path: String,
    pub scene_analysis: Option<SceneAnalysis>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SceneAnalysis {
    pub mesh_hierarchy: Vec<MeshNode>,
    pub animation_catalog: Vec<AnimationInfo>,
    pub material_library: Vec<MaterialInfo>,
    pub texture_dependencies: Vec<TextureDependency>,
    pub bone_structure: Option<SkeletonInfo>,
    pub scene_bounds: BoundingBox,
    pub performance_metrics: PerformanceMetrics,
    pub lod_levels: Vec<LodLevel>,
    pub physics_assets: Vec<PhysicsAsset>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MeshNode {
    pub name: String,
    pub sanitized_name: String,
    pub id: String,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub transform: Transform3D,
    pub geometry_info: GeometryInfo,
    pub material_assignments: Vec<String>,
    pub animation_targets: Vec<String>,
    pub physics_properties: Option<PhysicsProperties>,
    pub lod_group: Option<String>,
    pub mesh_type: MeshType,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Transform3D {
    pub position: [f32; 3],
    pub rotation: [f32; 4], // quaternion
    pub scale: [f32; 3],
    pub local_matrix: [f32; 16],
    pub world_matrix: [f32; 16],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeometryInfo {
    pub vertex_count: u32,
    pub face_count: u32,
    pub triangle_count: u32,
    pub has_uvs: bool,
    pub has_normals: bool,
    pub has_tangents: bool,
    pub has_vertex_colors: bool,
    pub vertex_attributes: Vec<String>,
    pub bounding_box: BoundingBox,
    pub surface_area: f32,
    pub volume: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoundingBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub size: [f32; 3],
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationInfo {
    pub name: String,
    pub sanitized_name: String,
    pub duration: f32,
    pub frame_rate: f32,
    pub frame_count: u32,
    pub start_frame: f32,
    pub end_frame: f32,
    pub target_meshes: Vec<String>,
    pub target_bones: Vec<String>,
    pub animation_tracks: Vec<AnimationTrack>,
    pub is_looping: bool,
    pub blend_mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationTrack {
    pub target_name: String,
    pub property: String, // position, rotation, scale, etc.
    pub keyframe_count: u32,
    pub interpolation: String,
    pub has_curves: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialInfo {
    pub name: String,
    pub sanitized_name: String,
    pub id: String,
    pub material_type: String,
    pub shader_type: String,
    pub texture_slots: Vec<TextureSlot>,
    pub properties: MaterialProperties,
    pub transparency: TransparencyInfo,
    pub assigned_to_meshes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextureSlot {
    pub slot_name: String, // diffuse, normal, specular, etc.
    pub texture_name: String,
    pub texture_path: String,
    pub uv_channel: u32,
    pub wrap_mode: String,
    pub filter_mode: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialProperties {
    pub diffuse_color: [f32; 4],
    pub specular_color: [f32; 3],
    pub emissive_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub normal_scale: f32,
    pub opacity: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransparencyInfo {
    pub is_transparent: bool,
    pub blend_mode: String,
    pub alpha_cutoff: f32,
    pub two_sided: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TextureDependency {
    pub texture_name: String,
    pub sanitized_name: String,
    pub file_path: String,
    pub format: String,
    pub dimensions: [u32; 2],
    pub file_size: u64,
    pub compression: String,
    pub mip_levels: u32,
    pub used_by_materials: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkeletonInfo {
    pub name: String,
    pub bone_count: u32,
    pub root_bones: Vec<BoneInfo>,
    pub bone_hierarchy: Vec<BoneInfo>,
    pub bind_pose_transforms: Vec<Transform3D>,
    pub inverse_bind_matrices: Vec<[f32; 16]>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BoneInfo {
    pub name: String,
    pub id: String,
    pub parent_id: Option<String>,
    pub children_ids: Vec<String>,
    pub transform: Transform3D,
    pub influenced_vertices: u32,
    pub weight_influence: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_vertices: u32,
    pub total_triangles: u32,
    pub total_materials: u32,
    pub total_textures: u32,
    pub memory_estimate_mb: f32,
    pub draw_calls_estimate: u32,
    pub complexity_score: f32, // 0-100
    pub optimization_suggestions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LodLevel {
    pub level: u32,
    pub distance: f32,
    pub vertex_reduction: f32,
    pub triangle_reduction: f32,
    pub meshes: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhysicsAsset {
    pub name: String,
    pub collision_type: String, // box, sphere, convex, triangle_mesh
    pub physics_material: String,
    pub mass: f32,
    pub friction: f32,
    pub restitution: f32,
    pub bounds: BoundingBox,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PhysicsProperties {
    pub has_collision: bool,
    pub collision_type: String,
    pub is_static: bool,
    pub mass: f32,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MeshType {
    Static,
    Skeletal,
    Instanced,
    Terrain,
    Particle,
    UI,
}

pub fn extract_model_settings(file_data: &[u8], filename: &str) -> ModelImportSettings {
    info!("🔍 Extracting intelligent settings from model file: {}", filename);
    
    let _file_extension = std::path::Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let file_size = file_data.len();
    let is_large_file = file_size > 10 * 1024 * 1024; // 10MB+
    let is_animated = filename.to_lowercase().contains("anim") || 
                     filename.to_lowercase().contains("walk") || 
                     filename.to_lowercase().contains("run") ||
                     filename.to_lowercase().contains("dance");
    let is_character = filename.to_lowercase().contains("character") ||
                      filename.to_lowercase().contains("woman") ||
                      filename.to_lowercase().contains("man") ||
                      filename.to_lowercase().contains("human");
    
    // TODO: Could analyze actual file contents to detect animations, bones, materials
    // For now, use intelligent defaults based on filename and size
    
    ModelImportSettings {
        general: GeneralSettings {
            use_source_name: true,
            scene_name_sub_folder: true,
            asset_type_sub_folders: true,
            offset_translation: [0.0, 0.0, 0.0],
            offset_rotation: [0.0, 0.0, 0.0],
            offset_uniform_scale: 1.0,
            force_all_mesh_type: "none".to_string(),
            auto_detect_mesh_type: true,
            import_lods: true,
            bake_meshes: false,
            bake_pivot_meshes: false,
            keep_sections_separate: is_large_file || is_character,
            vertex_color_import: "replace".to_string(),
            vertex_override_color: "#ffffff".to_string(),
            import_sockets: false,
            build: true,
        },
        skeletal_meshes: SkeletalMeshSettings {
            import_skeletal_meshes: is_character || is_animated,
            import_content_type: "geometry_and_skin_weights".to_string(),
            import_morph_targets: is_character,
            merge_morph_targets_with_same_name: false,
            import_vertex_attributes: true,
            update_skeleton_reference_pose: false,
            create_physics_asset: is_character,
            import_meshes_in_bone_hierarchy: false,
            add_curve_metadata_to_skeleton: false,
            convert_static_with_morph_to_skeletal: false,
        },
        static_meshes: StaticMeshSettings {
            import_static_meshes: true,
            combine_static_meshes: !is_character && is_large_file,
            lod_group: if is_large_file { "auto".to_string() } else { "none".to_string() },
            auto_compute_lod_screen_sizes: true,
            generate_collision: !is_character,
        },
        animations: AnimationSettings {
            import_animations: is_animated || is_character,
            import_bone_tracks: is_character,
            animation_length: "source_timeline".to_string(),
            frame_import_range: [0, -1],
            use_30hz_to_bake_bone_animation: false,
            custom_bone_animation_sample_rate: 30,
            snap_to_closest_frame_boundary: false,
            import_curves: is_animated,
            animation_only: false,
            skeleton: if is_character { "create_new".to_string() } else { "none".to_string() },
        },
        materials: MaterialSettings {
            import_textures: true,
            detect_normal_map_texture: true,
            flip_normal_map_texture: false,
            flip_normal_map_green_channel: false,
            import_udims: false,
            import_sparse_volume_textures: false,
            import_animated_sparse_volume_textures: false,
            file_extensions_for_long_lat_cubemap: "hdr,exr".to_string(),
            prefer_compressed_source_data: is_large_file,
            allow_non_power_of_two: true,
            draco_compression: is_large_file,
        },
        advanced: AdvancedSettings {
            file_units: "meters".to_string(),
            file_axis_direction: "y_up".to_string(),
            use_settings_for_subsequent_files: true,
        },
    }
}

pub fn process_model_import(
    file_data: Vec<u8>,
    original_filename: &str,
    project_name: &str,
    settings: ModelImportSettings,
) -> Result<ModelProcessResult, String> {
    info!("🎨 Processing model import: {} for project: {}", original_filename, project_name);
    
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    
    if !project_path.exists() {
        error!("❌ Project does not exist: {}", project_name);
        return Err("Project does not exist".to_string());
    }
    
    // Sanitize the original filename
    let file_extension = Path::new(original_filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let base_name = Path::new(original_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    
    let sanitized_base_name = sanitize_file_name(base_name);
    let sanitized_filename = format!("{}.{}", sanitized_base_name, file_extension);
    
    // Create folder structure based on settings
    let mut base_path = String::from("assets");
    
    if settings.general.asset_type_sub_folders {
        match file_extension.as_str() {
            "fbx" | "obj" | "gltf" | "glb" | "dae" | "3ds" | "blend" | "max" => {
                base_path.push_str("/models");
            }
            "png" | "jpg" | "jpeg" | "tga" | "hdr" | "exr" => {
                base_path.push_str("/textures");
            }
            _ => {}
        }
    }
    
    if settings.general.scene_name_sub_folder {
        base_path.push_str(&format!("/{}", sanitized_base_name));
    }
    
    let target_dir = project_path.join(&base_path);
    let target_file_path = target_dir.join(&sanitized_filename);
    
    // Create directories
    if let Err(e) = fs::create_dir_all(&target_dir) {
        error!("❌ Failed to create directories: {:?} - Error: {}", target_dir, e);
        return Err("Failed to create directories".to_string());
    }
    
    info!("📁 Created directory structure: {:?}", target_dir);
    
    // Write the model file
    if let Err(e) = fs::write(&target_file_path, &file_data) {
        error!("❌ Failed to write model file: {:?} - Error: {}", target_file_path, e);
        return Err("Failed to write model file".to_string());
    }
    
    let file_size = file_data.len() as u64;
    info!("✅ Successfully wrote model file: {:?} ({} bytes)", target_file_path, file_size);
    
    // Create saved assets record
    let saved_assets = vec![
        SavedAsset {
            asset_type: "model".to_string(),
            original_name: original_filename.to_string(),
            sanitized_name: sanitized_filename.clone(),
            path: format!("{}/{}", base_path, sanitized_filename),
            size_bytes: file_size,
        }
    ];
    
    // Create folder structure record
    let folder_structure = FolderStructure {
        base_path: base_path.clone(),
        created_folders: vec![base_path.clone()],
    };
    
    // Create import summary (scene analysis will be added by client-side BabylonJS)
    let import_summary = ImportSummary {
        original_file: original_filename.to_string(),
        sanitized_file: sanitized_filename.clone(),
        imported_at: chrono::Utc::now().to_rfc3339(),
        settings,
        file_size,
        target_path: format!("{}/{}", base_path, sanitized_filename),
        scene_analysis: None, // Will be populated by client-side analysis
    };
    
    // Save import summary JSON
    let summary_filename = format!("{}_import_summary.json", sanitized_base_name);
    let summary_path = target_dir.join(&summary_filename);
    let summary_json = match serde_json::to_string_pretty(&import_summary) {
        Ok(json) => json,
        Err(e) => {
            error!("❌ Failed to serialize import summary: {}", e);
            return Err("Failed to create import summary".to_string());
        }
    };
    
    if let Err(e) = fs::write(&summary_path, summary_json) {
        warn!("⚠️ Failed to write import summary: {:?} - Error: {}", summary_path, e);
    } else {
        info!("📄 Created import summary: {:?}", summary_path);
    }
    
    Ok(ModelProcessResult {
        success: true,
        saved_assets,
        folder_structure,
        import_summary,
    })
}