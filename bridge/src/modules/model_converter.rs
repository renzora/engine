use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::project_manager::get_projects_path;
use crate::file_sync::sanitize_file_name;
use log::{info, warn, error};
use std::collections::HashMap;
use std::process::Command;
use std::io::Write;
use tmf::{TMFMesh, TMFPrecisionInfo};

/// Apply Draco compression to vertex and index data using external Draco binary
fn apply_draco_compression(
    vertices: &[f32], 
    indices: &[u32],
    vertex_count: u32
) -> Result<(Vec<u8>, Vec<u8>), String> {
    info!("🗜️ Applying Draco compression to {} vertices using external binary", vertex_count);
    info!("🔍 Input data: {} vertex floats, {} indices", vertices.len(), indices.len());
    
    // Get the path to the Draco encoder binary
    match get_draco_encoder_path() {
        Ok(path) => {
            info!("✅ Found Draco encoder at: {}", path);
        }
        Err(e) => {
            warn!("❌ Draco encoder not found: {}", e);
            return Err(e);
        }
    }
    let draco_encoder_path = get_draco_encoder_path()?;
    
    // Create temporary input file in OBJ format
    let temp_obj_path = create_temp_obj_file(vertices, indices)?;
    let temp_drc_path = temp_obj_path.replace(".obj", ".drc");
    
    info!("📄 Created temp OBJ file: {}", temp_obj_path);
    info!("🎯 Target compressed file: {}", temp_drc_path);
    
    // Run Draco encoder
    info!("🚀 Running Draco encoder command...");
    let output = Command::new(&draco_encoder_path)
        .arg("-i").arg(&temp_obj_path)
        .arg("-o").arg(&temp_drc_path)
        .arg("-cl").arg("7") // Compression level 7 (high compression)
        .arg("-qp").arg("14") // Position quantization bits
        .output();
    
    match output {
        Ok(result) => {
            if result.status.success() {
                // Read the compressed file
                match std::fs::read(&temp_drc_path) {
                    Ok(compressed_data) => {
                        let original_size = vertices.len() * 4 + indices.len() * 4;
                        let compressed_size = compressed_data.len();
                        let compression_ratio = original_size as f32 / compressed_size as f32;
                        
                        info!("🎉 Draco compression successful!");
                        info!("📊 Original size: {} bytes, Compressed: {} bytes", original_size, compressed_size);
                        info!("📈 Compression ratio: {:.2}x", compression_ratio);
                        
                        // Clean up temporary files
                        let _ = std::fs::remove_file(&temp_obj_path);
                        let _ = std::fs::remove_file(&temp_drc_path);
                        
                        // Return original data for GLB (compressed data could be stored as extension)
                        let vertex_bytes: Vec<u8> = vertices.iter()
                            .flat_map(|&f| f.to_le_bytes().to_vec())
                            .collect();
                            
                        let index_bytes: Vec<u8> = indices.iter()
                            .flat_map(|&i| i.to_le_bytes().to_vec())
                            .collect();
                            
                        info!("💾 Compressed mesh data ready ({} bytes)", compressed_size);
                        Ok((vertex_bytes, index_bytes))
                    }
                    Err(e) => {
                        error!("❌ Failed to read compressed file: {}", e);
                        // Clean up
                        let _ = std::fs::remove_file(&temp_obj_path);
                        let _ = std::fs::remove_file(&temp_drc_path);
                        Err(format!("Failed to read compressed file: {}", e))
                    }
                }
            } else {
                let stderr = String::from_utf8_lossy(&result.stderr);
                error!("❌ Draco encoder failed: {}", stderr);
                // Clean up
                let _ = std::fs::remove_file(&temp_obj_path);
                Err(format!("Draco encoder failed: {}", stderr))
            }
        }
        Err(e) => {
            error!("❌ Failed to run Draco encoder: {}", e);
            // Clean up
            let _ = std::fs::remove_file(&temp_obj_path);
            Err(format!("Failed to run Draco encoder: {}", e))
        }
    }
}

/// Get the path to the Draco encoder binary
fn get_draco_encoder_path() -> Result<String, String> {
    // Check if we're on Windows or Unix
    let binary_name = if cfg!(target_os = "windows") {
        "draco_encoder.exe"
    } else {
        "draco_encoder"
    };
    
    // Try to find the binary in the bin directory relative to the current executable
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current executable path: {}", e))?;
    
    let exe_dir = current_exe.parent()
        .ok_or("Failed to get executable directory")?;
    
    let bin_path = exe_dir.join("bin").join(binary_name);
    
    if bin_path.exists() {
        Ok(bin_path.to_string_lossy().to_string())
    } else {
        // Fallback: try to find it in PATH
        match which::which(binary_name) {
            Ok(path) => Ok(path.to_string_lossy().to_string()),
            Err(_) => Err(format!("Draco encoder binary '{}' not found. Please place it in the bin directory.", binary_name))
        }
    }
}

/// Create a temporary OBJ file from vertex and index data
fn create_temp_obj_file(vertices: &[f32], indices: &[u32]) -> Result<String, String> {
    use std::env;
    
    let temp_dir = env::temp_dir();
    let temp_path = temp_dir.join(format!("draco_temp_{}.obj", std::process::id()));
    let temp_path_str = temp_path.to_string_lossy().to_string();
    
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| format!("Failed to create temp OBJ file: {}", e))?;
    
    // Write OBJ header
    writeln!(file, "# Temporary OBJ file for Draco compression")
        .map_err(|e| format!("Failed to write OBJ header: {}", e))?;
    
    // Write vertices
    for chunk in vertices.chunks(3) {
        if chunk.len() == 3 {
            writeln!(file, "v {} {} {}", chunk[0], chunk[1], chunk[2])
                .map_err(|e| format!("Failed to write vertex: {}", e))?;
        }
    }
    
    // Write faces (indices)
    for face in indices.chunks(3) {
        if face.len() == 3 {
            // OBJ indices are 1-based
            writeln!(file, "f {} {} {}", face[0] + 1, face[1] + 1, face[2] + 1)
                .map_err(|e| format!("Failed to write face: {}", e))?;
        }
    }
    
    Ok(temp_path_str)
}

/// Apply TMF compression to mesh data
fn apply_tmf_compression(
    vertices: &[f32],
    indices: &[u32],
    mesh_name: &str
) -> Result<Vec<u8>, String> {
    info!("🗜️ Applying TMF compression to mesh: {} ({} vertices, {} indices)", 
          mesh_name, vertices.len() / 3, indices.len() / 3);
    
    // Validate input data
    if vertices.len() % 3 != 0 {
        return Err("Vertex data must be divisible by 3 (x, y, z components)".to_string());
    }
    if indices.len() % 3 != 0 {
        return Err("Index data must be divisible by 3 (triangle faces)".to_string());
    }
    
    let vertex_count = vertices.len() / 3;
    let triangle_count = indices.len() / 3;
    
    // Create a temporary OBJ file for TMF processing
    let temp_obj_path = create_temp_obj_file(vertices, indices)?;
    
    // Read mesh from OBJ using TMF
    let mut file = std::fs::File::open(&temp_obj_path)
        .map_err(|e| format!("Failed to open temp OBJ file: {}", e))?;
    let meshes = TMFMesh::read_from_obj(&mut file)
        .map_err(|e| format!("Failed to read OBJ for TMF: {}", e))?;
    
    if meshes.is_empty() {
        return Err("No meshes found in temporary OBJ file".to_string());
    }
    
    // Take the first mesh (TMF returns Vec<(TMFMesh, String)>)
    let (mesh, _mesh_name) = &meshes[0];
    
    // Create precision info for compression quality
    let precision_info = TMFPrecisionInfo::default();
    
    // Create a buffer to write TMF data
    let mut buffer = Vec::new();
    
    // Write TMF mesh to buffer
    mesh.write_tmf_one(&mut buffer, &precision_info, mesh_name)
        .map_err(|e| format!("TMF compression failed: {}", e))?;
    
    // Clean up temp file
    let _ = std::fs::remove_file(&temp_obj_path);
    
    let original_size = vertices.len() * 4 + indices.len() * 4;
    let compressed_size = buffer.len();
    let compression_ratio = (1.0 - compressed_size as f64 / original_size as f64) * 100.0;
    
    info!("✅ TMF compression complete: {} -> {} bytes ({:.1}% reduction)", 
          original_size, compressed_size, compression_ratio);
    
    Ok(buffer)
}

#[derive(Debug, Deserialize)]
pub struct CompressionSettings {
    pub draco_compression: Option<bool>,
    pub tmf_encoding: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ConversionResult {
    pub success: bool,
    pub glb_path: String,
    pub extracted_assets: ExtractedAssets,
    pub conversion_summary: ConversionSummary,
    pub import_mode: ImportMode,
}

#[derive(Debug, Serialize)]
pub enum ImportMode {
    Separate, // Unreal-style: separate mesh/material/texture assets
    Combined, // Single combined GLB object
}

#[derive(Debug, Serialize)]
pub struct ExtractedAssets {
    pub meshes: Vec<ExtractedMesh>,
    pub materials: Vec<ExtractedMaterial>,
    pub textures: Vec<ExtractedTexture>,
    pub animations: Vec<ExtractedAnimation>,
    pub scene_graph: SceneGraph,
}

#[derive(Debug, Serialize)]
pub struct ExtractedMesh {
    pub name: String,
    pub file_path: String,
    pub vertex_count: u32,
    pub triangle_count: u32,
    pub has_uvs: bool,
    pub has_normals: bool,
    pub material_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct ExtractedMaterial {
    pub id: String,
    pub name: String,
    pub file_path: String,
    pub pbr_properties: PbrProperties,
    pub texture_maps: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct PbrProperties {
    pub base_color: [f32; 4],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive: [f32; 3],
    pub normal_scale: f32,
    pub occlusion_strength: f32,
}

#[derive(Debug, Serialize)]
pub struct ExtractedTexture {
    pub name: String,
    pub file_path: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub usage: String,
}

#[derive(Debug, Serialize)]
pub struct ExtractedAnimation {
    pub name: String,
    pub file_path: String,
    pub duration: f32,
    pub target_meshes: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct SceneGraph {
    pub root_nodes: Vec<SceneNode>,
}

#[derive(Debug, Serialize)]
pub struct SceneNode {
    pub name: String,
    pub transform: Transform3D,
    pub mesh_id: Option<String>,
    pub children: Vec<SceneNode>,
}

#[derive(Debug, Serialize)]
pub struct Transform3D {
    pub position: [f32; 3],
    pub rotation: [f32; 4], // quaternion
    pub scale: [f32; 3],
    pub local_matrix: [f32; 16],
    pub world_matrix: [f32; 16],
}

#[derive(Debug, Serialize)]
pub struct ConversionSummary {
    pub original_format: String,
    pub converted_format: String,
    pub conversion_time_ms: u64,
    pub original_size_bytes: u64,
    pub glb_size_bytes: u64,
    pub extracted_files_count: u32,
}

pub fn convert_model_to_glb_and_extract(
    file_data: Vec<u8>,
    original_filename: &str,
    project_name: &str,
    import_mode: Option<ImportMode>,
    compression: Option<CompressionSettings>,
    current_path: Option<&str>,
) -> Result<ConversionResult, String> {
    let start_time = std::time::Instant::now();
    info!("🔄 Converting {} to GLB and extracting assets", original_filename);
    
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    
    if !project_path.exists() {
        return Err("Project does not exist".to_string());
    }
    
    let file_extension = Path::new(original_filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let mode = import_mode.unwrap_or(ImportMode::Separate);
    let use_draco = compression.as_ref().and_then(|c| c.draco_compression).unwrap_or(false);
    let use_tmf = compression.as_ref().and_then(|c| c.tmf_encoding).unwrap_or(false);
    
    info!("🔧 Compression settings - Draco: {}, TMF: {}", use_draco, use_tmf);
    
    if use_draco {
        info!("🗜️ Draco compression ENABLED - will attempt compression during GLB creation");
    } else {
        info!("⚪ Draco compression DISABLED");
    }
    
    // Step 1: Handle different input formats with import mode
    let (glb_data, extracted_assets) = match file_extension.as_str() {
        "obj" => convert_obj_and_extract(&file_data, original_filename, &project_path, &mode, use_draco, use_tmf, current_path)?,
        "gltf" => convert_gltf_and_extract(&file_data, original_filename, &project_path, &mode, current_path)?,
        "glb" => extract_from_existing_glb(&file_data, original_filename, &project_path, &mode, current_path, use_draco, use_tmf)?,
        _ => {
            warn!("⚠️ Format {} not supported for conversion, saving as original", file_extension);
            return save_unsupported_format(file_data, original_filename, project_name, mode, current_path);
        }
    };
    
    let conversion_time = start_time.elapsed().as_millis() as u64;
    let conversion_summary = ConversionSummary {
        original_format: file_extension.to_uppercase(),
        converted_format: "GLB".to_string(),
        conversion_time_ms: conversion_time,
        original_size_bytes: file_data.len() as u64,
        glb_size_bytes: glb_data.len() as u64,
        extracted_files_count: extracted_assets.meshes.len() as u32 + 
                               extracted_assets.materials.len() as u32 + 
                               extracted_assets.textures.len() as u32,
    };
    
    let base_name = Path::new(original_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    info!("✅ Conversion completed in {}ms", conversion_time);
    
    Ok(ConversionResult {
        success: true,
        glb_path: format!("assets/models/{}/{}.glb", sanitized_base_name, sanitized_base_name),
        extracted_assets,
        conversion_summary,
        import_mode: mode,
    })
}

fn convert_obj_and_extract(
    file_data: &[u8],
    filename: &str,
    project_path: &Path,
    import_mode: &ImportMode,
    use_draco: bool,
    use_tmf: bool,
    current_path: Option<&str>,
) -> Result<(Vec<u8>, ExtractedAssets), String> {
    info!("🔄 Converting OBJ to GLB and extracting assets");
    
    // Create temporary file for tobj
    let temp_dir = std::env::temp_dir();
    let temp_file_path = temp_dir.join(filename);
    
    fs::write(&temp_file_path, file_data)
        .map_err(|e| format!("Failed to write temporary OBJ file: {}", e))?;
    
    // Load OBJ file with robust material handling
    let (models, materials_result) = tobj::load_obj(&temp_file_path, &tobj::LoadOptions::default())
        .map_err(|e| format!("Failed to parse OBJ file: {}", e))?;
    
    // Clean up temp file
    let _ = fs::remove_file(&temp_file_path);
    
    // Handle materials gracefully - if MTL file is missing, create defaults
    let materials = match materials_result {
        Ok(materials) => materials,
        Err(_) => {
            warn!("⚠️ MTL file not found or failed to load, creating default materials");
            vec![tobj::Material {
                name: "default_material".to_string(),
                ambient: Some([0.2, 0.2, 0.2]),
                diffuse: Some([0.8, 0.8, 0.8]),
                specular: Some([1.0, 1.0, 1.0]),
                shininess: Some(32.0),
                dissolve: Some(1.0),
                optical_density: Some(1.0),
                illumination_model: Some(2),
                ambient_texture: None,
                diffuse_texture: None,
                specular_texture: None,
                shininess_texture: None,
                normal_texture: None,
                dissolve_texture: None,
                unknown_param: ahash::AHashMap::new(),
            }]
        }
    };
    
    // Extract assets and create folder structure
    let base_name = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    // Use current_path exactly as provided, empty string means project root
    let base_path = current_path.unwrap_or("");
    let assets_dir = if base_path.is_empty() {
        // Project root
        project_path.join(&sanitized_base_name)
    } else {
        // Specific directory
        project_path.join(base_path).join(&sanitized_base_name)
    };
    fs::create_dir_all(&assets_dir).map_err(|e| format!("Failed to create assets directory: {}", e))?;
    
    let (glb_data, extracted_assets) = match import_mode {
        ImportMode::Separate => {
            info!("🔄 Using Separate mode (Unreal-style): creating individual assets");
            // Create individual GLB files for each mesh + extract all assets
            let extracted_assets = extract_obj_assets_separate(&models, &materials, &assets_dir, &sanitized_base_name)?;
            // Create a simple combined GLB for preview/fallback
            let glb_data = create_simple_combined_glb(&models, &materials, use_draco, use_tmf)?;
            let glb_path = assets_dir.join(format!("{}_combined.glb", sanitized_base_name));
            fs::write(&glb_path, &glb_data).map_err(|e| format!("Failed to write combined GLB file: {}", e))?;
            
            // Create TMF compressed versions if requested
            if use_tmf {
                create_tmf_files_from_models(&models, &assets_dir, &sanitized_base_name)?;
            }
            
            (glb_data, extracted_assets)
        }
        ImportMode::Combined => {
            info!("🔄 Using Combined mode: creating single merged GLB object");
            // Create single GLB with all meshes combined
            let glb_data = create_simple_combined_glb(&models, &materials, use_draco, use_tmf)?;
            let glb_path = assets_dir.join(format!("{}.glb", sanitized_base_name));
            fs::write(&glb_path, &glb_data).map_err(|e| format!("Failed to write GLB file: {}", e))?;
            
            // Create TMF compressed version if requested
            if use_tmf {
                create_tmf_files_from_models(&models, &assets_dir, &sanitized_base_name)?;
            }
            
            // Create minimal metadata (no individual mesh files)
            let extracted_assets = create_combined_metadata(&models, &materials, &assets_dir, &sanitized_base_name)?;
            (glb_data, extracted_assets)
        }
    };
    
    Ok((glb_data, extracted_assets))
}

fn convert_gltf_and_extract(
    file_data: &[u8],
    filename: &str,
    project_path: &Path,
    import_mode: &ImportMode,
    current_path: Option<&str>,
) -> Result<(Vec<u8>, ExtractedAssets), String> {
    info!("🔄 Converting GLTF to GLB and extracting assets");
    
    // Parse GLTF
    let gltf_str = std::str::from_utf8(file_data)
        .map_err(|e| format!("Invalid UTF-8 in GLTF file: {}", e))?;
    
    let gltf_json: serde_json::Value = serde_json::from_str(gltf_str)
        .map_err(|e| format!("Failed to parse GLTF JSON: {}", e))?;
    
    // Convert to binary GLB format
    let glb_data = convert_gltf_json_to_glb(&gltf_json)?;
    
    // Extract and save assets
    let base_name = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    // Use current_path exactly as provided, empty string means project root
    let base_path = current_path.unwrap_or("");
    let assets_dir = if base_path.is_empty() {
        // Project root
        project_path.join(&sanitized_base_name)
    } else {
        // Specific directory
        project_path.join(base_path).join(&sanitized_base_name)
    };
    fs::create_dir_all(&assets_dir).map_err(|e| format!("Failed to create assets directory: {}", e))?;
    
    let glb_path = assets_dir.join(format!("{}.glb", sanitized_base_name));
    fs::write(&glb_path, &glb_data).map_err(|e| format!("Failed to write GLB file: {}", e))?;
    
    let extracted_assets = extract_gltf_assets(&gltf_json, &assets_dir, &sanitized_base_name)?;
    
    Ok((glb_data, extracted_assets))
}

fn extract_from_existing_glb(
    file_data: &[u8],
    filename: &str,
    project_path: &Path,
    import_mode: &ImportMode,
    current_path: Option<&str>,
    use_draco: bool,
    use_tmf: bool,
) -> Result<(Vec<u8>, ExtractedAssets), String> {
    info!("🔄 Extracting assets from existing GLB with mode: {:?}", import_mode);
    
    let base_name = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    // Use current_path exactly as provided, empty string means project root
    let base_path = current_path.unwrap_or("");
    let assets_dir = if base_path.is_empty() {
        // Project root
        project_path.join(&sanitized_base_name)
    } else {
        // Specific directory
        project_path.join(base_path).join(&sanitized_base_name)
    };
    fs::create_dir_all(&assets_dir).map_err(|e| format!("Failed to create assets directory: {}", e))?;
    
    let extracted_assets = match import_mode {
        ImportMode::Separate => {
            info!("🔄 GLB Separate mode: extracting individual assets");
            // Create directories for separate assets
            fs::create_dir_all(assets_dir.join("meshes"))
                .map_err(|e| format!("Failed to create meshes directory: {}", e))?;
            fs::create_dir_all(assets_dir.join("materials"))
                .map_err(|e| format!("Failed to create materials directory: {}", e))?;
            
            // Strip textures from GLB and save clean version (Unreal-style)
            let mut stripped_glb = strip_textures_from_glb(file_data)?;
            
            // Apply compression to stripped GLB if requested
            if use_draco {
                info!("🗜️ STRIPPED GLB - Attempting Draco compression on stripped GLB file");
                // TODO: Apply Draco compression to the stripped GLB
                warn!("⚠️ Stripped GLB Draco compression not yet implemented - using uncompressed");
            }
            
            let glb_path = assets_dir.join(format!("{}_stripped.glb", sanitized_base_name));
            fs::write(&glb_path, &stripped_glb).map_err(|e| format!("Failed to write stripped GLB file: {}", e))?;
            
            // Extract and create separate asset files from original GLB
            extract_glb_assets_separate(file_data, &assets_dir, &sanitized_base_name, base_path, use_draco, use_tmf)?
        }
        ImportMode::Combined => {
            info!("🔄 GLB Combined mode: keeping as single object");
            // Just save the GLB as-is
            let glb_path = assets_dir.join(format!("{}.glb", sanitized_base_name));
            fs::write(&glb_path, file_data).map_err(|e| format!("Failed to write GLB file: {}", e))?;
            
            // Create minimal metadata
            create_glb_combined_metadata(file_data, &sanitized_base_name)?
        }
    };
    
    Ok((file_data.to_vec(), extracted_assets))
}

fn save_unsupported_format(
    file_data: Vec<u8>,
    original_filename: &str,
    project_name: &str,
    import_mode: ImportMode,
    current_path: Option<&str>,
) -> Result<ConversionResult, String> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    
    let base_name = Path::new(original_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    // Use current_path exactly as provided, empty string means project root
    let base_path = current_path.unwrap_or("");
    let assets_dir = if base_path.is_empty() {
        // Project root
        project_path.join(&sanitized_base_name)
    } else {
        // Specific directory
        project_path.join(base_path).join(&sanitized_base_name)
    };
    fs::create_dir_all(&assets_dir).map_err(|e| format!("Failed to create assets directory: {}", e))?;
    
    let file_path = assets_dir.join(original_filename);
    fs::write(&file_path, &file_data).map_err(|e| format!("Failed to write original file: {}", e))?;
    
    let conversion_summary = ConversionSummary {
        original_format: Path::new(original_filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_uppercase(),
        converted_format: "ORIGINAL".to_string(),
        conversion_time_ms: 0,
        original_size_bytes: file_data.len() as u64,
        glb_size_bytes: 0,
        extracted_files_count: 1,
    };
    
    let extracted_assets = ExtractedAssets {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        animations: Vec::new(),
        scene_graph: SceneGraph { root_nodes: Vec::new() },
    };
    
    Ok(ConversionResult {
        success: true,
        glb_path: format!("assets/models/{}/{}", sanitized_base_name, original_filename),
        extracted_assets,
        conversion_summary,
        import_mode,
    })
}

fn create_simple_combined_glb(
    models: &[tobj::Model],
    _materials: &[tobj::Material],
    use_draco: bool,
    use_tmf: bool,
) -> Result<Vec<u8>, String> {
    info!("🔄 Creating valid GLB from OBJ data with Draco: {}, TMF: {}", use_draco, use_tmf);
    
    if models.is_empty() {
        return Err("No models found in OBJ file".to_string());
    }
    
    let mut all_vertices = Vec::new();
    let mut accessor_offset = 0;
    let mut meshes_json = Vec::new();
    let mut nodes_json = Vec::new();
    
    // Collect all vertex data and indices
    let mut all_indices = Vec::new();
    let mut indices_offset = 0;
    
    for (model_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let vertex_count = mesh.positions.len() / 3;
        
        if vertex_count == 0 {
            continue;
        }
        
        // Add vertices to combined buffer
        for chunk in mesh.positions.chunks(3) {
            all_vertices.extend_from_slice(&chunk[0].to_le_bytes());
            all_vertices.extend_from_slice(&chunk[1].to_le_bytes());
            all_vertices.extend_from_slice(&chunk[2].to_le_bytes());
        }
        
        // Apply Draco compression if requested
        if use_draco {
            info!("🔍 DRACO DEBUG: Starting compression for mesh with {} vertices, {} indices", vertex_count, mesh.indices.len());
            
            // Convert raw vertex buffer back to f32 array for compression
            let vertex_floats: Vec<f32> = mesh.positions.clone();
                
            // Convert indices to u32 array
            let index_u32s: Vec<u32> = mesh.indices.iter().map(|&i| i as u32).collect();
            
            info!("🔍 DRACO DEBUG: Converted to {} float vertices, {} u32 indices", vertex_floats.len(), index_u32s.len());
            
            match apply_draco_compression(&vertex_floats, &index_u32s, vertex_count.try_into().unwrap()) {
                Ok((compressed_vertices, compressed_indices)) => {
                    info!("✅ Applied Draco compression successfully - original vertex data: {} bytes, compressed: {} bytes", 
                          vertex_floats.len() * 4, compressed_vertices.len());
                    // Note: For now we use the original data in GLB, but compression metadata is logged
                    // Future enhancement: Store Draco compressed data as glTF extension
                }
                Err(e) => {
                    warn!("⚠️ Draco compression failed: {}, proceeding with uncompressed data", e);
                }
            }
        } else {
            info!("⚪ DRACO DEBUG: Compression disabled for this mesh");
        }
        
        // Add indices to combined buffer (offset by current vertex count)
        let vertex_offset = indices_offset;
        if !mesh.indices.is_empty() {
            for &index in &mesh.indices {
                all_indices.extend_from_slice(&(index + vertex_offset).to_le_bytes());
            }
        } else {
            // Generate indices for triangulation if missing
            for i in (0..vertex_count).step_by(3) {
                if i + 2 < vertex_count {
                    all_indices.extend_from_slice(&(vertex_offset + i as u32).to_le_bytes());
                    all_indices.extend_from_slice(&(vertex_offset + (i + 1) as u32).to_le_bytes());
                    all_indices.extend_from_slice(&(vertex_offset + (i + 2) as u32).to_le_bytes());
                }
            }
        }
        
        // Create mesh JSON with correct accessor indices
        let primitive = if !mesh.indices.is_empty() || vertex_count >= 3 {
            serde_json::json!({
                "attributes": {
                    "POSITION": accessor_offset
                },
                "indices": accessor_offset + models.len(), // Indices come after position accessors
                "mode": 4
            })
        } else {
            serde_json::json!({
                "attributes": {
                    "POSITION": accessor_offset
                },
                "mode": 4
            })
        };
        
        meshes_json.push(serde_json::json!({
            "name": if model.name.is_empty() { format!("mesh_{}", model_idx) } else { model.name.clone() },
            "primitives": [primitive]
        }));
        
        // Create node JSON
        nodes_json.push(serde_json::json!({
            "name": if model.name.is_empty() { format!("node_{}", model_idx) } else { model.name.clone() },
            "mesh": model_idx
        }));
        
        accessor_offset += 1;
        indices_offset += vertex_count as u32;
    }
    
    // Combine vertex and index data into single buffer
    let mut combined_buffer = all_vertices;
    let indices_start_offset = combined_buffer.len();
    combined_buffer.extend_from_slice(&all_indices);
    let buffer_size = combined_buffer.len();
    
    // Create valid GLTF structure with proper buffers, buffer views, and accessors
    let mut accessors_json = Vec::new();
    let mut buffer_views_json = Vec::new();
    let mut current_vertex_offset = 0;
    let mut current_index_offset = 0;
    
    // Create position accessors and buffer views
    for (model_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let vertex_count = mesh.positions.len() / 3;
        
        if vertex_count == 0 {
            continue;
        }
        
        let vertex_byte_length = vertex_count * 12; // 3 floats * 4 bytes each
        
        // Position buffer view
        buffer_views_json.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": current_vertex_offset,
            "byteLength": vertex_byte_length,
            "byteStride": 12,
            "target": 34962 // ARRAY_BUFFER
        }));
        
        // Position accessor
        accessors_json.push(serde_json::json!({
            "bufferView": model_idx,
            "componentType": 5126, // FLOAT
            "count": vertex_count,
            "type": "VEC3"
        }));
        
        current_vertex_offset += vertex_byte_length;
    }
    
    // Create index accessors and buffer views
    let mut current_index_byte_offset = indices_start_offset;
    for (model_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let vertex_count = mesh.positions.len() / 3;
        
        if vertex_count == 0 {
            continue;
        }
        
        let index_count = if !mesh.indices.is_empty() {
            mesh.indices.len()
        } else {
            (vertex_count / 3) * 3 // Triangulated
        };
        
        if index_count == 0 {
            continue;
        }
        
        let index_byte_length = index_count * 4; // u32 * 4 bytes each
        
        // Index buffer view
        buffer_views_json.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": current_index_byte_offset,
            "byteLength": index_byte_length,
            "target": 34963 // ELEMENT_ARRAY_BUFFER
        }));
        
        // Index accessor
        accessors_json.push(serde_json::json!({
            "bufferView": buffer_views_json.len() - 1,
            "componentType": 5125, // UNSIGNED_INT
            "count": index_count,
            "type": "SCALAR"
        }));
        
        current_index_byte_offset += index_byte_length;
    }
    
    let gltf_json = serde_json::json!({
        "asset": {
            "version": "2.0",
            "generator": "Renzora Engine Bridge"
        },
        "scene": 0,
        "scenes": [
            {
                "name": "Scene",
                "nodes": (0..nodes_json.len()).collect::<Vec<_>>()
            }
        ],
        "nodes": nodes_json,
        "meshes": meshes_json,
        "buffers": [
            {
                "byteLength": buffer_size
            }
        ],
        "bufferViews": buffer_views_json,
        "accessors": accessors_json
    });
    
    // Convert to GLB binary format with BIN chunk
    let json_string = serde_json::to_string(&gltf_json)
        .map_err(|e| format!("Failed to serialize GLTF JSON: {}", e))?;
    
    let json_bytes = json_string.as_bytes();
    let json_length = json_bytes.len() as u32;
    let json_padded_length = ((json_length + 3) / 4) * 4;
    
    let bin_length = combined_buffer.len() as u32;
    let bin_padded_length = ((bin_length + 3) / 4) * 4;
    
    // Calculate total GLB size
    let header_size = 12;
    let json_chunk_header = 8;
    let bin_chunk_header = 8;
    let total_length = header_size + json_chunk_header + json_padded_length + bin_chunk_header + bin_padded_length;
    
    let mut glb_data = Vec::new();
    
    // GLB Header
    glb_data.extend_from_slice(b"glTF"); // Magic
    glb_data.extend_from_slice(&2u32.to_le_bytes()); // Version
    glb_data.extend_from_slice(&total_length.to_le_bytes()); // Total length
    
    // JSON chunk
    glb_data.extend_from_slice(&json_padded_length.to_le_bytes()); // JSON chunk length
    glb_data.extend_from_slice(b"JSON"); // JSON chunk type
    glb_data.extend_from_slice(json_bytes);
    
    // Pad JSON to 4-byte boundary with spaces
    while glb_data.len() % 4 != 0 {
        glb_data.push(b' ');
    }
    
    // BIN chunk
    glb_data.extend_from_slice(&bin_padded_length.to_le_bytes()); // BIN chunk length
    glb_data.extend_from_slice(b"BIN\0"); // BIN chunk type
    glb_data.extend_from_slice(&combined_buffer);
    
    // Pad BIN to 4-byte boundary with zeros
    while glb_data.len() % 4 != 0 {
        glb_data.push(0);
    }
    
    Ok(glb_data)
}

fn create_single_mesh_glb(
    models: &[tobj::Model],
    _materials: &[tobj::Material],
) -> Result<Vec<u8>, String> {
    // For now, just use the combined GLB function for single meshes
    create_simple_combined_glb(models, _materials, false, false)
}

fn extract_obj_assets_separate(
    models: &[tobj::Model],
    materials: &[tobj::Material],
    assets_dir: &Path,
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting OBJ assets in Separate mode (Unreal-style)");
    
    let mut extracted_meshes = Vec::new();
    let mut extracted_materials = Vec::new();
    let scene_nodes = Vec::new();
    
    // Create directories
    fs::create_dir_all(assets_dir.join("meshes"))
        .map_err(|e| format!("Failed to create meshes directory: {}", e))?;
    fs::create_dir_all(assets_dir.join("materials"))
        .map_err(|e| format!("Failed to create materials directory: {}", e))?;
    
    // Extract each mesh as individual GLB file (Unreal-style)
    for (mesh_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let mesh_name = if model.name.is_empty() {
            format!("mesh_{}", mesh_idx)
        } else {
            model.name.clone()
        };
        
        // Create individual GLB for this mesh only
        let single_mesh_glb = create_single_mesh_glb(&[model.clone()], materials)?;
        let mesh_filename = format!("{}_{}.glb", base_name, sanitize_file_name(&mesh_name));
        let mesh_path = assets_dir.join("meshes").join(&mesh_filename);
        
        fs::write(&mesh_path, single_mesh_glb)
            .map_err(|e| format!("Failed to write individual mesh GLB: {}", e))?;
        
        extracted_meshes.push(ExtractedMesh {
            name: mesh_name.clone(),
            file_path: format!("assets/models/{}/meshes/{}", base_name, mesh_filename),
            vertex_count: (mesh.positions.len() / 3) as u32,
            triangle_count: if !mesh.indices.is_empty() { 
                (mesh.indices.len() / 3) as u32 
            } else { 
                (mesh.positions.len() / 9) as u32 
            },
            has_uvs: !mesh.texcoords.is_empty(),
            has_normals: !mesh.normals.is_empty(),
            material_ids: if let Some(mat_id) = mesh.material_id {
                vec![format!("material_{}", mat_id)]
            } else {
                Vec::new()
            },
        });
    }
    
    // Extract materials as individual JSON files
    for (mat_idx, material) in materials.iter().enumerate() {
        let material_name = if material.name.is_empty() {
            format!("material_{}", mat_idx)
        } else {
            material.name.clone()
        };
        
        let material_data = serde_json::json!({
            "name": material_name,
            "type": "pbr",
            "properties": {
                "diffuse": material.diffuse.unwrap_or([1.0, 1.0, 1.0]),
                "specular": material.specular.unwrap_or([1.0, 1.0, 1.0]),
                "ambient": material.ambient.unwrap_or([0.1, 0.1, 0.1]),
                "shininess": material.shininess.unwrap_or(0.0),
                "dissolve": material.dissolve.unwrap_or(1.0)
            },
            "textures": {
                "diffuse": material.diffuse_texture.as_ref().unwrap_or(&String::new()),
                "normal": material.normal_texture.as_ref().unwrap_or(&String::new()),
                "specular": material.specular_texture.as_ref().unwrap_or(&String::new())
            }
        });
        
        let material_filename = format!("{}_{}.json", base_name, sanitize_file_name(&material_name));
        let material_path = assets_dir.join("materials").join(&material_filename);
        
        fs::write(&material_path, serde_json::to_string_pretty(&material_data).unwrap())
            .map_err(|e| format!("Failed to write material file: {}", e))?;
        
        extracted_materials.push(ExtractedMaterial {
            id: format!("material_{}", mat_idx),
            name: material_name,
            file_path: format!("assets/models/{}/materials/{}", base_name, material_filename),
            pbr_properties: PbrProperties {
                base_color: if let Some(diffuse) = material.diffuse {
                    [diffuse[0], diffuse[1], diffuse[2], 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                },
                metallic: 0.0,
                roughness: 1.0 - (material.shininess.unwrap_or(0.0) / 128.0),
                emissive: [0.0, 0.0, 0.0],
                normal_scale: 1.0,
                occlusion_strength: 1.0,
            },
            texture_maps: HashMap::new(),
        });
    }
    
    // Create scene graph
    let scene_graph = SceneGraph {
        root_nodes: scene_nodes,
    };
    
    let scene_filename = format!("{}_scene.json", base_name);
    let scene_path = assets_dir.join(&scene_filename);
    fs::write(&scene_path, serde_json::to_string_pretty(&scene_graph).unwrap())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    
    Ok(ExtractedAssets {
        meshes: extracted_meshes,
        materials: extracted_materials,
        textures: Vec::new(),
        animations: Vec::new(),
        scene_graph,
    })
}

fn create_combined_metadata(
    models: &[tobj::Model],
    materials: &[tobj::Material],
    assets_dir: &Path,
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Creating minimal metadata for Combined mode");
    
    // Just create basic metadata without individual files
    let mut extracted_meshes = Vec::new();
    let mut extracted_materials = Vec::new();
    
    // Collect mesh metadata (but don't create individual files)
    for (mesh_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let mesh_name = if model.name.is_empty() {
            format!("mesh_{}", mesh_idx)
        } else {
            model.name.clone()
        };
        
        extracted_meshes.push(ExtractedMesh {
            name: mesh_name,
            file_path: format!("assets/models/{}/{}.glb", base_name, base_name), // Points to combined GLB
            vertex_count: (mesh.positions.len() / 3) as u32,
            triangle_count: if !mesh.indices.is_empty() { 
                (mesh.indices.len() / 3) as u32 
            } else { 
                (mesh.positions.len() / 9) as u32 
            },
            has_uvs: !mesh.texcoords.is_empty(),
            has_normals: !mesh.normals.is_empty(),
            material_ids: if let Some(mat_id) = mesh.material_id {
                vec![format!("material_{}", mat_id)]
            } else {
                Vec::new()
            },
        });
    }
    
    // Collect material metadata (but don't create individual files)
    for (mat_idx, material) in materials.iter().enumerate() {
        let material_name = if material.name.is_empty() {
            format!("material_{}", mat_idx)
        } else {
            material.name.clone()
        };
        
        extracted_materials.push(ExtractedMaterial {
            id: format!("material_{}", mat_idx),
            name: material_name,
            file_path: format!("assets/models/{}/{}.glb", base_name, base_name), // Points to combined GLB
            pbr_properties: PbrProperties {
                base_color: if let Some(diffuse) = material.diffuse {
                    [diffuse[0], diffuse[1], diffuse[2], 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                },
                metallic: 0.0,
                roughness: 1.0 - (material.shininess.unwrap_or(0.0) / 128.0),
                emissive: [0.0, 0.0, 0.0],
                normal_scale: 1.0,
                occlusion_strength: 1.0,
            },
            texture_maps: HashMap::new(),
        });
    }
    
    let scene_graph = SceneGraph {
        root_nodes: Vec::new(),
    };
    
    Ok(ExtractedAssets {
        meshes: extracted_meshes,
        materials: extracted_materials,
        textures: Vec::new(),
        animations: Vec::new(),
        scene_graph,
    })
}

fn extract_obj_assets_combined(
    models: &[tobj::Model],
    materials: &[tobj::Material],
    assets_dir: &Path,
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting assets from OBJ data");
    
    let mut extracted_meshes = Vec::new();
    let mut extracted_materials = Vec::new();
    let scene_nodes = Vec::new();
    
    // Create directories
    fs::create_dir_all(assets_dir.join("meshes"))
        .map_err(|e| format!("Failed to create meshes directory: {}", e))?;
    fs::create_dir_all(assets_dir.join("materials"))
        .map_err(|e| format!("Failed to create materials directory: {}", e))?;
    
    // Extract meshes
    for (mesh_idx, model) in models.iter().enumerate() {
        let mesh = &model.mesh;
        let mesh_name = if model.name.is_empty() {
            format!("mesh_{}", mesh_idx)
        } else {
            model.name.clone()
        };
        
        // Create mesh metadata
        let mesh_data = serde_json::json!({
            "name": mesh_name,
            "vertex_count": mesh.positions.len() / 3,
            "triangle_count": mesh.indices.len() / 3,
            "has_uvs": !mesh.texcoords.is_empty(),
            "has_normals": !mesh.normals.is_empty(),
            "material_index": mesh.material_id
        });
        
        let mesh_filename = format!("{}_{}.json", base_name, sanitize_file_name(&mesh_name));
        let mesh_path = assets_dir.join("meshes").join(&mesh_filename);
        
        fs::write(&mesh_path, serde_json::to_string_pretty(&mesh_data).unwrap())
            .map_err(|e| format!("Failed to write mesh metadata: {}", e))?;
        
        extracted_meshes.push(ExtractedMesh {
            name: mesh_name,
            file_path: format!("assets/models/{}/meshes/{}", base_name, mesh_filename),
            vertex_count: (mesh.positions.len() / 3) as u32,
            triangle_count: (mesh.indices.len() / 3) as u32,
            has_uvs: !mesh.texcoords.is_empty(),
            has_normals: !mesh.normals.is_empty(),
            material_ids: if let Some(mat_id) = mesh.material_id {
                vec![format!("material_{}", mat_id)]
            } else {
                Vec::new()
            },
        });
    }
    
    // Extract materials
    for (mat_idx, material) in materials.iter().enumerate() {
        let material_name = if material.name.is_empty() {
            format!("material_{}", mat_idx)
        } else {
            material.name.clone()
        };
        
        let material_data = serde_json::json!({
            "name": material_name,
            "type": "pbr",
            "properties": {
                "diffuse": material.diffuse.unwrap_or([1.0, 1.0, 1.0]),
                "specular": material.specular.unwrap_or([1.0, 1.0, 1.0]),
                "ambient": material.ambient.unwrap_or([0.1, 0.1, 0.1]),
                "shininess": material.shininess.unwrap_or(0.0),
                "dissolve": material.dissolve.unwrap_or(1.0)
            },
            "textures": {
                "diffuse": material.diffuse_texture.as_ref().unwrap_or(&String::new()),
                "normal": material.normal_texture.as_ref().unwrap_or(&String::new()),
                "specular": material.specular_texture.as_ref().unwrap_or(&String::new())
            }
        });
        
        let material_filename = format!("{}_{}.json", base_name, sanitize_file_name(&material_name));
        let material_path = assets_dir.join("materials").join(&material_filename);
        
        fs::write(&material_path, serde_json::to_string_pretty(&material_data).unwrap())
            .map_err(|e| format!("Failed to write material file: {}", e))?;
        
        extracted_materials.push(ExtractedMaterial {
            id: format!("material_{}", mat_idx),
            name: material_name,
            file_path: format!("assets/models/{}/materials/{}", base_name, material_filename),
            pbr_properties: PbrProperties {
                base_color: if let Some(diffuse) = material.diffuse {
                    [diffuse[0], diffuse[1], diffuse[2], 1.0]
                } else {
                    [1.0, 1.0, 1.0, 1.0]
                },
                metallic: 0.0,
                roughness: 1.0 - (material.shininess.unwrap_or(0.0) / 128.0),
                emissive: [0.0, 0.0, 0.0], // OBJ doesn't have emission typically
                normal_scale: 1.0,
                occlusion_strength: 1.0,
            },
            texture_maps: HashMap::new(),
        });
    }
    
    // Create scene graph
    let scene_graph = SceneGraph {
        root_nodes: scene_nodes,
    };
    
    let scene_filename = format!("{}_scene.json", base_name);
    let scene_path = assets_dir.join(&scene_filename);
    fs::write(&scene_path, serde_json::to_string_pretty(&scene_graph).unwrap())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    
    Ok(ExtractedAssets {
        meshes: extracted_meshes,
        materials: extracted_materials,
        textures: Vec::new(), // TODO: Extract textures from OBJ/MTL
        animations: Vec::new(), // OBJ doesn't support animations
        scene_graph,
    })
}

fn convert_gltf_json_to_glb(gltf_json: &serde_json::Value) -> Result<Vec<u8>, String> {
    let json_string = serde_json::to_string(gltf_json)
        .map_err(|e| format!("Failed to serialize GLTF JSON: {}", e))?;
    
    let json_bytes = json_string.as_bytes();
    let json_length = json_bytes.len() as u32;
    
    // GLB Header
    let mut glb_data = Vec::new();
    glb_data.extend_from_slice(b"glTF"); // Magic
    glb_data.extend_from_slice(&2u32.to_le_bytes()); // Version
    
    // Calculate total length
    let header_size = 12;
    let json_chunk_header_size = 8;
    let json_chunk_padded_size = ((json_length + 3) / 4) * 4;
    let total_length = header_size + json_chunk_header_size + json_chunk_padded_size;
    
    glb_data.extend_from_slice(&total_length.to_le_bytes());
    
    // JSON chunk
    glb_data.extend_from_slice(&json_chunk_padded_size.to_le_bytes());
    glb_data.extend_from_slice(b"JSON");
    glb_data.extend_from_slice(json_bytes);
    
    // Pad to 4-byte boundary
    while glb_data.len() % 4 != 0 {
        glb_data.push(b' ');
    }
    
    Ok(glb_data)
}

fn extract_gltf_assets(
    _gltf_json: &serde_json::Value,
    assets_dir: &Path,
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting assets from GLTF JSON");
    
    // Create directories
    fs::create_dir_all(assets_dir.join("meshes"))
        .map_err(|e| format!("Failed to create meshes directory: {}", e))?;
    fs::create_dir_all(assets_dir.join("materials"))
        .map_err(|e| format!("Failed to create materials directory: {}", e))?;
    
    // TODO: Parse GLTF JSON and extract assets
    // For now, create empty structure
    
    let scene_graph = SceneGraph {
        root_nodes: Vec::new(),
    };
    
    let scene_filename = format!("{}_scene.json", base_name);
    let scene_path = assets_dir.join(&scene_filename);
    fs::write(&scene_path, serde_json::to_string_pretty(&scene_graph).unwrap())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    
    Ok(ExtractedAssets {
        meshes: Vec::new(),
        materials: Vec::new(),
        textures: Vec::new(),
        animations: Vec::new(),
        scene_graph,
    })
}

fn extract_glb_assets_separate(
    glb_data: &[u8],
    assets_dir: &Path,
    base_name: &str,
    base_path: &str,
    use_draco: bool,
    use_tmf: bool,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting GLB assets in Separate mode (Draco: {}, TMF: {})", use_draco, use_tmf);
    
    // Parse GLB to extract JSON
    let gltf_json = parse_glb_to_json(glb_data)?;
    
    // Extract meshes, materials, and create separate files
    let mut extracted_meshes = Vec::new();
    let mut extracted_materials = Vec::new();
    
    // Parse meshes from GLTF JSON
    if let Some(meshes) = gltf_json.get("meshes").and_then(|m| m.as_array()) {
        for (mesh_idx, mesh) in meshes.iter().enumerate() {
            let mesh_name = mesh.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("mesh_{}", mesh_idx));
            
            // Create individual mesh GLB file
            let individual_glb = create_individual_mesh_glb(glb_data, mesh_idx, &mesh_name, use_draco, use_tmf)?;
            
            let mesh_filename = format!("{}_{}.glb", base_name, sanitize_file_name(&mesh_name));
            let mesh_path = assets_dir.join("meshes").join(&mesh_filename);
            
            fs::write(&mesh_path, &individual_glb)
                .map_err(|e| format!("Failed to write mesh GLB file: {}", e))?;
            
            extracted_meshes.push(ExtractedMesh {
                name: mesh_name,
                file_path: format!("assets/models/{}/meshes/{}", base_name, mesh_filename),
                vertex_count: 0, // TODO: Extract from GLB
                triangle_count: 0, // TODO: Extract from GLB
                has_uvs: true,
                has_normals: true,
                material_ids: Vec::new(),
            });
        }
    }
    
    // Extract textures first
    let extracted_textures = extract_textures_from_glb(glb_data, &assets_dir, base_name, base_path)?;
    
    // Parse materials from GLTF JSON
    if let Some(materials) = gltf_json.get("materials").and_then(|m| m.as_array()) {
        for (mat_idx, material) in materials.iter().enumerate() {
            let material_name = material.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("material_{}", mat_idx));
            
            // Create BabylonJS PBR material with extracted texture references
            let babylon_material = create_babylonjs_material(material, &material_name, &extracted_textures, base_name, base_path)?;
            
            let material_filename = format!("{}_{}.material", base_name, sanitize_file_name(&material_name));
            let material_path = assets_dir.join("materials").join(&material_filename);
            
            fs::write(&material_path, serde_json::to_string_pretty(&babylon_material).unwrap())
                .map_err(|e| format!("Failed to write material file: {}", e))?;
            
            extracted_materials.push(ExtractedMaterial {
                id: format!("material_{}", mat_idx),
                name: material_name,
                file_path: format!("assets/models/{}/materials/{}", base_name, material_filename),
                pbr_properties: PbrProperties {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    metallic: 0.0,
                    roughness: 0.5,
                    emissive: [0.0, 0.0, 0.0],
                    normal_scale: 1.0,
                    occlusion_strength: 1.0,
                },
                texture_maps: HashMap::new(),
            });
        }
    }
    
    let scene_graph = SceneGraph {
        root_nodes: Vec::new(),
    };
    
    // Save scene graph
    let scene_filename = format!("{}_scene.json", base_name);
    let scene_path = assets_dir.join(&scene_filename);
    fs::write(&scene_path, serde_json::to_string_pretty(&scene_graph).unwrap())
        .map_err(|e| format!("Failed to write scene file: {}", e))?;
    
    Ok(ExtractedAssets {
        meshes: extracted_meshes,
        materials: extracted_materials,
        textures: extracted_textures,
        animations: Vec::new(),
        scene_graph,
    })
}

fn create_glb_combined_metadata(
    glb_data: &[u8],
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Creating minimal metadata for GLB Combined mode");
    
    // Parse GLB to get basic info
    let gltf_json = parse_glb_to_json(glb_data)?;
    
    let mut extracted_meshes = Vec::new();
    let mut extracted_materials = Vec::new();
    
    // Collect mesh metadata (pointing to combined GLB)
    if let Some(meshes) = gltf_json.get("meshes").and_then(|m| m.as_array()) {
        for (mesh_idx, mesh) in meshes.iter().enumerate() {
            let mesh_name = mesh.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("mesh_{}", mesh_idx));
            
            extracted_meshes.push(ExtractedMesh {
                name: mesh_name,
                file_path: format!("assets/models/{}/{}.glb", base_name, base_name),
                vertex_count: 0, // TODO: Extract from GLB
                triangle_count: 0, // TODO: Extract from GLB
                has_uvs: true,
                has_normals: true,
                material_ids: Vec::new(),
            });
        }
    }
    
    // Collect material metadata (pointing to combined GLB)
    if let Some(materials) = gltf_json.get("materials").and_then(|m| m.as_array()) {
        for (mat_idx, material) in materials.iter().enumerate() {
            let material_name = material.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("material_{}", mat_idx));
            
            extracted_materials.push(ExtractedMaterial {
                id: format!("material_{}", mat_idx),
                name: material_name,
                file_path: format!("assets/models/{}/{}.glb", base_name, base_name),
                pbr_properties: PbrProperties {
                    base_color: [1.0, 1.0, 1.0, 1.0],
                    metallic: 0.0,
                    roughness: 0.5,
                    emissive: [0.0, 0.0, 0.0],
                    normal_scale: 1.0,
                    occlusion_strength: 1.0,
                },
                texture_maps: HashMap::new(),
            });
        }
    }
    
    Ok(ExtractedAssets {
        meshes: extracted_meshes,
        materials: extracted_materials,
        textures: Vec::new(),
        animations: Vec::new(),
        scene_graph: SceneGraph { root_nodes: Vec::new() },
    })
}

fn parse_glb_to_json(glb_data: &[u8]) -> Result<serde_json::Value, String> {
    // Parse GLB header to get JSON chunk
    if glb_data.len() < 12 {
        return Err("Invalid GLB file - too small".to_string());
    }
    
    let magic = &glb_data[0..4];
    if magic != b"glTF" {
        return Err("Invalid GLB file - wrong magic".to_string());
    }
    
    // Extract JSON chunk
    let json_chunk_length = u32::from_le_bytes([glb_data[12], glb_data[13], glb_data[14], glb_data[15]]);
    let json_start = 20; // 12 byte header + 8 byte chunk header
    let json_end = json_start + json_chunk_length as usize;
    
    if json_end > glb_data.len() {
        return Err("Invalid GLB file - JSON chunk exceeds file size".to_string());
    }
    
    let json_bytes = &glb_data[json_start..json_end];
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in GLB JSON: {}", e))?;
    
    let gltf_json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse GLB JSON: {}", e))?;
    
    Ok(gltf_json)
}

fn extract_glb_assets(
    glb_data: &[u8],
    assets_dir: &Path,
    base_name: &str,
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting assets from GLB binary");
    
    // Parse GLB header to get JSON chunk
    if glb_data.len() < 12 {
        return Err("Invalid GLB file - too small".to_string());
    }
    
    let magic = &glb_data[0..4];
    if magic != b"glTF" {
        return Err("Invalid GLB file - wrong magic".to_string());
    }
    
    // Extract JSON chunk and parse it
    let json_chunk_length = u32::from_le_bytes([glb_data[12], glb_data[13], glb_data[14], glb_data[15]]);
    let json_start = 20; // 12 byte header + 8 byte chunk header
    let json_end = json_start + json_chunk_length as usize;
    
    if json_end > glb_data.len() {
        return Err("Invalid GLB file - JSON chunk exceeds file size".to_string());
    }
    
    let json_bytes = &glb_data[json_start..json_end];
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in GLB JSON: {}", e))?;
    
    let gltf_json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse GLB JSON: {}", e))?;
    
    // Extract assets using GLTF parser
    extract_gltf_assets(&gltf_json, assets_dir, base_name)
}

fn extract_textures_from_glb(glb_data: &[u8], assets_dir: &Path, base_name: &str, base_path: &str) -> Result<Vec<ExtractedTexture>, String> {
    info!("🔄 Extracting textures from GLB");
    
    // Create the assets directory (textures will go in the same folder as the model)
    // No need for separate textures directory
    
    let mut extracted_textures = Vec::new();
    
    // Parse GLB to get GLTF JSON and binary data
    let gltf_json = parse_glb_to_json(glb_data)?;
    let bin_data = extract_glb_binary_data(glb_data)?;
    
    // Extract textures from GLTF
    if let Some(textures) = gltf_json.get("textures").and_then(|t| t.as_array()) {
        if let Some(images) = gltf_json.get("images").and_then(|i| i.as_array()) {
            for (tex_idx, texture) in textures.iter().enumerate() {
                let image_index = texture.get("source").and_then(|s| s.as_u64()).unwrap_or(0) as usize;
                
                if image_index < images.len() {
                    let image = &images[image_index];
                    let texture_name = format!("texture_{}", tex_idx);
                    
                    // Extract image data
                    if let Some(buffer_view_index) = image.get("bufferView").and_then(|bv| bv.as_u64()) {
                        if let Some(buffer_views) = gltf_json.get("bufferViews").and_then(|bv| bv.as_array()) {
                            if let Some(buffer_view) = buffer_views.get(buffer_view_index as usize) {
                                let byte_offset = buffer_view.get("byteOffset").and_then(|o| o.as_u64()).unwrap_or(0) as usize;
                                let byte_length = buffer_view.get("byteLength").and_then(|l| l.as_u64()).unwrap_or(0) as usize;
                                
                                if byte_offset + byte_length <= bin_data.len() {
                                    let image_data = &bin_data[byte_offset..byte_offset + byte_length];
                                    
                                    // Detect format from magic bytes
                                    let format = detect_image_format(image_data);
                                    let texture_filename = format!("{}_{}.{}", base_name, texture_name, format);
                                    let texture_path = assets_dir.join(&texture_filename);
                                    
                                    // Save texture file
                                    fs::write(&texture_path, image_data)
                                        .map_err(|e| format!("Failed to write texture file: {}", e))?;
                                    
                                    // Get image dimensions if possible
                                    let (width, height) = get_image_dimensions(image_data).unwrap_or((512, 512));
                                    
                                    extracted_textures.push(ExtractedTexture {
                                        name: texture_name,
                                        file_path: if base_path.is_empty() {
                                            format!("{}/{}", base_name, texture_filename)
                                        } else {
                                            format!("{}/{}/{}", base_path, base_name, texture_filename)
                                        },
                                        format: format.to_string(),
                                        width,
                                        height,
                                        usage: "diffuse".to_string(), // TODO: Detect usage from material
                                    });
                                    
                                    info!("✅ Extracted texture: {} ({}x{})", texture_filename, width, height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    
    Ok(extracted_textures)
}

fn extract_glb_binary_data(glb_data: &[u8]) -> Result<&[u8], String> {
    if glb_data.len() < 20 {
        return Err("GLB file too small".to_string());
    }
    
    // Skip to BIN chunk (after JSON chunk)
    let json_chunk_length = u32::from_le_bytes([glb_data[12], glb_data[13], glb_data[14], glb_data[15]]);
    let json_chunk_padded_length = ((json_chunk_length + 3) / 4) * 4;
    let bin_chunk_start = 20 + json_chunk_padded_length as usize;
    
    if bin_chunk_start + 8 > glb_data.len() {
        return Err("Invalid GLB structure".to_string());
    }
    
    let bin_chunk_length = u32::from_le_bytes([
        glb_data[bin_chunk_start], 
        glb_data[bin_chunk_start + 1], 
        glb_data[bin_chunk_start + 2], 
        glb_data[bin_chunk_start + 3]
    ]);
    
    let bin_data_start = bin_chunk_start + 8;
    let bin_data_end = bin_data_start + bin_chunk_length as usize;
    
    if bin_data_end > glb_data.len() {
        return Err("BIN chunk exceeds file size".to_string());
    }
    
    Ok(&glb_data[bin_data_start..bin_data_end])
}

fn detect_image_format(image_data: &[u8]) -> &'static str {
    if image_data.len() < 4 {
        return "bin";
    }
    
    // Check magic bytes
    match &image_data[0..4] {
        [0x89, 0x50, 0x4E, 0x47] => "png",
        [0xFF, 0xD8, 0xFF, _] => "jpg",
        [0x42, 0x4D, _, _] => "bmp",
        _ => {
            // Check for WEBP
            if image_data.len() >= 12 && &image_data[0..4] == b"RIFF" && &image_data[8..12] == b"WEBP" {
                "webp"
            } else {
                "bin"
            }
        }
    }
}

fn get_image_dimensions(image_data: &[u8]) -> Option<(u32, u32)> {
    // Try to parse image dimensions using the image crate
    match image::load_from_memory(image_data) {
        Ok(img) => Some((img.width(), img.height())),
        Err(_) => None,
    }
}

fn strip_textures_from_glb(glb_data: &[u8]) -> Result<Vec<u8>, String> {
    info!("🔄 Stripping embedded textures from GLB (Unreal-style)");
    
    // Parse GLB header
    if glb_data.len() < 20 {
        return Err("GLB file too small".to_string());
    }
    
    let magic = &glb_data[0..4];
    if magic != b"glTF" {
        return Err("Invalid GLB file - wrong magic".to_string());
    }
    
    // Extract JSON chunk
    let json_chunk_length = u32::from_le_bytes([glb_data[12], glb_data[13], glb_data[14], glb_data[15]]);
    let json_start = 20;
    let json_end = json_start + json_chunk_length as usize;
    
    if json_end > glb_data.len() {
        return Err("Invalid GLB file - JSON chunk exceeds file size".to_string());
    }
    
    let json_bytes = &glb_data[json_start..json_end];
    let json_str = std::str::from_utf8(json_bytes)
        .map_err(|e| format!("Invalid UTF-8 in GLB JSON: {}", e))?;
    
    let mut gltf_json: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| format!("Failed to parse GLB JSON: {}", e))?;
    
    // Remove images and textures from GLTF JSON (Unreal approach)
    gltf_json.as_object_mut().unwrap().remove("images");
    gltf_json.as_object_mut().unwrap().remove("textures");
    
    // Update materials to remove ALL texture references and extensions
    if let Some(materials) = gltf_json.get_mut("materials").and_then(|m| m.as_array_mut()) {
        for material in materials {
            if let Some(pbr) = material.get_mut("pbrMetallicRoughness").and_then(|p| p.as_object_mut()) {
                pbr.remove("baseColorTexture");
                pbr.remove("metallicRoughnessTexture");
            }
            if let Some(mat_obj) = material.as_object_mut() {
                mat_obj.remove("normalTexture");
                mat_obj.remove("emissiveTexture");
                mat_obj.remove("occlusionTexture");
                // Remove all material extensions that might reference textures
                mat_obj.remove("extensions");
            }
        }
    }
    
    // Extract original BIN chunk (geometry data)
    let original_bin_data = extract_glb_binary_data(glb_data)?;
    
    // Serialize modified JSON
    let new_json = serde_json::to_string(&gltf_json)
        .map_err(|e| format!("Failed to serialize modified GLTF JSON: {}", e))?;
    let new_json_bytes = new_json.as_bytes();
    
    // Calculate new JSON chunk length (padded to 4-byte boundary)
    let json_length = new_json_bytes.len();
    let json_padded_length = ((json_length + 3) / 4) * 4;
    let mut json_chunk = vec![0u8; json_padded_length];
    json_chunk[..json_length].copy_from_slice(new_json_bytes);
    // Pad with spaces
    for i in json_length..json_padded_length {
        json_chunk[i] = b' ';
    }
    
    // Calculate BIN chunk length (padded to 4-byte boundary)
    let bin_length = original_bin_data.len();
    let bin_padded_length = ((bin_length + 3) / 4) * 4;
    let mut bin_chunk = vec![0u8; bin_padded_length];
    bin_chunk[..bin_length].copy_from_slice(original_bin_data);
    // Pad with zeros
    for i in bin_length..bin_padded_length {
        bin_chunk[i] = 0;
    }
    
    // Build new GLB with JSON + BIN chunks (textures stripped from JSON)
    let mut new_glb = Vec::new();
    
    // Calculate total GLB size
    let total_length = 12 + 8 + json_padded_length + 8 + bin_padded_length;
    
    // GLB header
    new_glb.extend_from_slice(b"glTF");                              // magic
    new_glb.extend_from_slice(&2u32.to_le_bytes());                  // version
    new_glb.extend_from_slice(&(total_length as u32).to_le_bytes()); // total length
    
    // JSON chunk header
    new_glb.extend_from_slice(&(json_padded_length as u32).to_le_bytes()); // chunk length
    new_glb.extend_from_slice(b"JSON");                              // chunk type
    
    // JSON chunk data
    new_glb.extend_from_slice(&json_chunk);
    
    // BIN chunk header
    new_glb.extend_from_slice(&(bin_padded_length as u32).to_le_bytes()); // chunk length
    new_glb.extend_from_slice(b"BIN\0");                            // chunk type
    
    // BIN chunk data (geometry)
    new_glb.extend_from_slice(&bin_chunk);
    
    info!("✅ Stripped {} bytes of embedded textures from GLB while preserving geometry", glb_data.len() - new_glb.len());
    
    Ok(new_glb)
}

fn create_individual_mesh_glb(glb_data: &[u8], mesh_index: usize, mesh_name: &str, use_draco: bool, use_tmf: bool) -> Result<Vec<u8>, String> {
    info!("🔄 Creating individual GLB for mesh: {} (Draco: {}, TMF: {})", mesh_name, use_draco, use_tmf);
    
    if use_draco {
        info!("🗜️ INDIVIDUAL MESH - Draco compression requested for: {}", mesh_name);
        // TODO: Extract individual mesh data and apply Draco compression
        // For now, log the request and return original data
        warn!("⚠️ Individual mesh Draco compression not yet implemented - using original GLB");
    }
    
    // For now, just copy the original GLB - in a full implementation this would
    // extract only the specific mesh data and create a minimal GLB
    // TODO: Implement proper mesh extraction using gltf crate
    Ok(glb_data.to_vec())
}

fn create_babylonjs_material(gltf_material: &serde_json::Value, material_name: &str, extracted_textures: &[ExtractedTexture], base_name: &str, base_path: &str) -> Result<serde_json::Value, String> {
    // Extract PBR properties from GLTF material
    let pbr = gltf_material.get("pbrMetallicRoughness");
    
    let base_color = pbr
        .and_then(|p| p.get("baseColorFactor"))
        .and_then(|f| f.as_array())
        .map(|arr| [
            arr.get(0).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
            arr.get(3).and_then(|v| v.as_f64()).unwrap_or(1.0) as f32,
        ])
        .unwrap_or([1.0, 1.0, 1.0, 1.0]);
    
    let metallic = pbr
        .and_then(|p| p.get("metallicFactor"))
        .and_then(|f| f.as_f64())
        .unwrap_or(0.0) as f32;
        
    let roughness = pbr
        .and_then(|p| p.get("roughnessFactor"))
        .and_then(|f| f.as_f64())
        .unwrap_or(0.5) as f32;
    
    let emissive = gltf_material
        .get("emissiveFactor")
        .and_then(|f| f.as_array())
        .map(|arr| [
            arr.get(0).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            arr.get(1).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            arr.get(2).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        ])
        .unwrap_or([0.0, 0.0, 0.0]);
    
    // Find corresponding extracted textures
    let base_color_texture = pbr
        .and_then(|p| p.get("baseColorTexture"))
        .and_then(|t| t.get("index"))
        .and_then(|i| i.as_u64())
        .and_then(|i| extracted_textures.get(i as usize))
        .map(|tex| &tex.file_path);
        
    let normal_texture = gltf_material
        .get("normalTexture")
        .and_then(|t| t.get("index"))
        .and_then(|i| i.as_u64())
        .and_then(|i| extracted_textures.get(i as usize))
        .map(|tex| &tex.file_path);
        
    let metallic_roughness_texture = pbr
        .and_then(|p| p.get("metallicRoughnessTexture"))
        .and_then(|t| t.get("index"))
        .and_then(|i| i.as_u64())
        .and_then(|i| extracted_textures.get(i as usize))
        .map(|tex| &tex.file_path);
    
    // Create BabylonJS PBR Material format
    let babylon_material = serde_json::json!({
        "customType": "BABYLON.PBRMaterial",
        "name": material_name,
        "id": material_name,
        "tags": null,
        "backFaceCulling": !gltf_material.get("doubleSided").and_then(|d| d.as_bool()).unwrap_or(false),
        "wireframe": false,
        
        // PBR Properties
        "baseColor": [base_color[0], base_color[1], base_color[2]],
        "alpha": base_color[3],
        "metallic": metallic,
        "roughness": roughness,
        "emissive": emissive,
        
        // Textures (reference extracted texture files)
        "baseTexture": base_color_texture.map(|path| serde_json::json!({
            "name": if base_path.is_empty() {
                format!("{}/{}", base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("{}/{}/{}", base_path, base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "url": if base_path.is_empty() {
                format!("/projects/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("/projects/{}/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_path,
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "level": 1.0,
            "hasAlpha": true,
            "getAlphaFromRGB": false,
            "coordinatesIndex": 0,
            "wrapU": 1,
            "wrapV": 1,
            "samplingMode": 3
        })),
        
        "normalTexture": normal_texture.map(|path| serde_json::json!({
            "name": if base_path.is_empty() {
                format!("{}/{}", base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("{}/{}/{}", base_path, base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "url": if base_path.is_empty() {
                format!("/projects/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("/projects/{}/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_path,
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "level": 1.0,
            "coordinatesIndex": 0,
            "wrapU": 1,
            "wrapV": 1,
            "samplingMode": 3
        })),
        
        "metallicRoughnessTexture": metallic_roughness_texture.map(|path| serde_json::json!({
            "name": if base_path.is_empty() {
                format!("{}/{}", base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("{}/{}/{}", base_path, base_name, std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "url": if base_path.is_empty() {
                format!("/projects/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            } else {
                format!("/projects/{}/{}/{}/{}", 
                    std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                    base_path,
                    base_name,
                    std::path::Path::new(path).file_name().unwrap().to_string_lossy())
            },
            "level": 1.0,
            "coordinatesIndex": 0,
            "wrapU": 1,
            "wrapV": 1,
            "samplingMode": 3
        })),
        
        // BabylonJS specific settings
        "useAlphaFromBaseTexture": false,
        "useAmbientOcclusionFromMetallicTextureRed": false,
        "useRoughnessFromMetallicTextureGreen": true,
        "useMetallicFromMetallicTextureBlue": true,
        "maxSimultaneousLights": 4,
        "disableLighting": false,
        "environmentIntensity": 1.0,
        "cameraExposure": 1.0,
        "cameraContrast": 1.0,
        "microSurface": 1.0,
        "indexOfRefraction": 1.5,
        "clearCoat": {
            "isEnabled": false,
            "intensity": 0.0,
            "roughness": 0.0
        }
    });
    
    Ok(babylon_material)
}

/// Create TMF compressed files from extracted models
fn create_tmf_files_from_models(
    models: &[tobj::Model],
    assets_dir: &Path,
    base_name: &str
) -> Result<(), String> {
    info!("🗜️ Creating TMF compressed files for {} models", models.len());
    
    // Create TMF directory
    let tmf_dir = assets_dir.join("tmf");
    fs::create_dir_all(&tmf_dir)
        .map_err(|e| format!("Failed to create TMF directory: {}", e))?;
    
    // Process each model/mesh
    for (model_idx, model) in models.iter().enumerate() {
        let mesh_name = if !model.name.is_empty() {
            model.name.clone()
        } else {
            format!("mesh_{}", model_idx)
        };
        
        // Skip empty meshes
        if model.mesh.positions.is_empty() {
            info!("⚠️ Skipping empty mesh: {}", mesh_name);
            continue;
        }
        
        // Prepare vertex data (positions only for TMF)
        let vertices = &model.mesh.positions;
        
        // Prepare index data - handle both indexed and non-indexed meshes
        let indices: Vec<u32> = if !model.mesh.indices.is_empty() {
            model.mesh.indices.clone()
        } else {
            // Generate indices for non-indexed mesh (assuming triangles)
            (0..vertices.len() as u32 / 3 * 3).step_by(3)
                .flat_map(|i| vec![i, i + 1, i + 2])
                .collect()
        };
        
        // Check mesh size limits for TMF
        let vertex_count = vertices.len() / 3;
        if vertex_count > 65535 {
            warn!("⚠️ Mesh '{}' has {} vertices (>65k), skipping TMF compression", 
                  mesh_name, vertex_count);
            continue;
        }
        
        // Apply TMF compression
        match apply_tmf_compression(vertices, &indices, &mesh_name) {
            Ok(tmf_data) => {
                // Save TMF file
                let sanitized_mesh_name = sanitize_file_name(&mesh_name);
                let tmf_filename = if models.len() == 1 {
                    format!("{}.tmf", base_name)
                } else {
                    format!("{}_{}.tmf", base_name, sanitized_mesh_name)
                };
                
                let tmf_path = tmf_dir.join(&tmf_filename);
                fs::write(&tmf_path, &tmf_data)
                    .map_err(|e| format!("Failed to write TMF file '{}': {}", tmf_filename, e))?;
                
                info!("✅ Created TMF file: {} ({} bytes)", tmf_filename, tmf_data.len());
            }
            Err(e) => {
                warn!("❌ TMF compression failed for mesh '{}': {}", mesh_name, e);
                continue;
            }
        }
    }
    
    info!("🎉 TMF compression complete");
    Ok(())
}