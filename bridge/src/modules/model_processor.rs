use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::project_manager::get_projects_path;
use crate::file_sync::sanitize_file_name;
use log::{info, warn, error};

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelImportSettings {
    pub general: GeneralSettings,
    pub skeletal_meshes: SkeletalMeshSettings,
    pub static_meshes: StaticMeshSettings,
    pub animations: AnimationSettings,
    pub materials: MaterialSettings,
    pub advanced: AdvancedSettings,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub use_source_name: bool,
    pub scene_name_sub_folder: bool,
    pub asset_type_sub_folders: bool,
    pub offset_translation: [f32; 3],
    pub offset_rotation: [f32; 3],
    pub offset_uniform_scale: f32,
    pub force_all_mesh_type: String,
    pub auto_detect_mesh_type: bool,
    pub import_lods: bool,
    pub bake_meshes: bool,
    pub bake_pivot_meshes: bool,
    pub keep_sections_separate: bool,
    pub vertex_color_import: String,
    pub vertex_override_color: String,
    pub import_sockets: bool,
    pub build: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SkeletalMeshSettings {
    pub import_skeletal_meshes: bool,
    pub import_content_type: String,
    pub import_morph_targets: bool,
    pub merge_morph_targets_with_same_name: bool,
    pub import_vertex_attributes: bool,
    pub update_skeleton_reference_pose: bool,
    pub create_physics_asset: bool,
    pub import_meshes_in_bone_hierarchy: bool,
    pub add_curve_metadata_to_skeleton: bool,
    pub convert_static_with_morph_to_skeletal: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticMeshSettings {
    pub import_static_meshes: bool,
    pub combine_static_meshes: bool,
    pub lod_group: String,
    pub auto_compute_lod_screen_sizes: bool,
    pub generate_collision: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnimationSettings {
    pub import_animations: bool,
    pub import_bone_tracks: bool,
    pub animation_length: String,
    pub frame_import_range: [i32; 2],
    pub use_30hz_to_bake_bone_animation: bool,
    pub custom_bone_animation_sample_rate: u32,
    pub snap_to_closest_frame_boundary: bool,
    pub import_curves: bool,
    pub animation_only: bool,
    pub skeleton: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialSettings {
    pub import_textures: bool,
    pub detect_normal_map_texture: bool,
    pub flip_normal_map_texture: bool,
    pub flip_normal_map_green_channel: bool,
    pub import_udims: bool,
    pub import_sparse_volume_textures: bool,
    pub import_animated_sparse_volume_textures: bool,
    pub file_extensions_for_long_lat_cubemap: String,
    pub prefer_compressed_source_data: bool,
    pub allow_non_power_of_two: bool,
    pub draco_compression: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdvancedSettings {
    pub file_units: String,
    pub file_axis_direction: String,
    pub use_settings_for_subsequent_files: bool,
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
    
    // Create import summary
    let import_summary = ImportSummary {
        original_file: original_filename.to_string(),
        sanitized_file: sanitized_filename.clone(),
        imported_at: chrono::Utc::now().to_rfc3339(),
        settings,
        file_size,
        target_path: format!("{}/{}", base_path, sanitized_filename),
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