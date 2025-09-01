use std::fs;
use std::path::Path;
use serde::{Serialize, Deserialize};
use crate::project_manager::get_projects_path;
use crate::file_sync::sanitize_file_name;
use log::{info, warn};
use std::collections::HashMap;

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
    
    // Step 1: Handle different input formats with import mode
    let (glb_data, extracted_assets) = match file_extension.as_str() {
        "obj" => convert_obj_and_extract(&file_data, original_filename, &project_path, &mode, use_draco, use_tmf)?,
        "gltf" => convert_gltf_and_extract(&file_data, original_filename, &project_path, &mode)?,
        "glb" => extract_from_existing_glb(&file_data, original_filename, &project_path, &mode)?,
        _ => {
            warn!("⚠️ Format {} not supported for conversion, saving as original", file_extension);
            return save_unsupported_format(file_data, original_filename, project_name, mode);
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
    
    let assets_dir = project_path.join("assets").join("models").join(&sanitized_base_name);
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
            (glb_data, extracted_assets)
        }
        ImportMode::Combined => {
            info!("🔄 Using Combined mode: creating single merged GLB object");
            // Create single GLB with all meshes combined
            let glb_data = create_simple_combined_glb(&models, &materials, use_draco, use_tmf)?;
            let glb_path = assets_dir.join(format!("{}.glb", sanitized_base_name));
            fs::write(&glb_path, &glb_data).map_err(|e| format!("Failed to write GLB file: {}", e))?;
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
    
    let assets_dir = project_path.join("assets").join("models").join(&sanitized_base_name);
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
) -> Result<(Vec<u8>, ExtractedAssets), String> {
    info!("🔄 Extracting assets from existing GLB with mode: {:?}", import_mode);
    
    let base_name = Path::new(filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    let assets_dir = project_path.join("assets").join("models").join(&sanitized_base_name);
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
            let stripped_glb = strip_textures_from_glb(file_data)?;
            let glb_path = assets_dir.join(format!("{}_stripped.glb", sanitized_base_name));
            fs::write(&glb_path, &stripped_glb).map_err(|e| format!("Failed to write stripped GLB file: {}", e))?;
            
            // Extract and create separate asset files from original GLB
            extract_glb_assets_separate(file_data, &assets_dir, &sanitized_base_name)?
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
) -> Result<ConversionResult, String> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    
    let base_name = Path::new(original_filename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("model");
    let sanitized_base_name = sanitize_file_name(base_name);
    
    let assets_dir = project_path.join("assets").join("models").join(&sanitized_base_name);
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
        
        // TODO: Apply Draco compression here if use_draco is true
        if use_draco {
            info!("🗜️ Would apply Draco compression to {} vertices", vertex_count);
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
) -> Result<ExtractedAssets, String> {
    info!("🔄 Extracting GLB assets in Separate mode");
    
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
            let individual_glb = create_individual_mesh_glb(glb_data, mesh_idx, &mesh_name)?;
            
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
    let extracted_textures = extract_textures_from_glb(glb_data, &assets_dir, base_name)?;
    
    // Parse materials from GLTF JSON
    if let Some(materials) = gltf_json.get("materials").and_then(|m| m.as_array()) {
        for (mat_idx, material) in materials.iter().enumerate() {
            let material_name = material.get("name")
                .and_then(|n| n.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| format!("material_{}", mat_idx));
            
            // Create BabylonJS PBR material with extracted texture references
            let babylon_material = create_babylonjs_material(material, &material_name, &extracted_textures, base_name)?;
            
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

fn extract_textures_from_glb(glb_data: &[u8], assets_dir: &Path, base_name: &str) -> Result<Vec<ExtractedTexture>, String> {
    info!("🔄 Extracting textures from GLB");
    
    // Create textures directory
    fs::create_dir_all(assets_dir.join("textures"))
        .map_err(|e| format!("Failed to create textures directory: {}", e))?;
    
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
                                    let texture_path = assets_dir.join("textures").join(&texture_filename);
                                    
                                    // Save texture file
                                    fs::write(&texture_path, image_data)
                                        .map_err(|e| format!("Failed to write texture file: {}", e))?;
                                    
                                    // Get image dimensions if possible
                                    let (width, height) = get_image_dimensions(image_data).unwrap_or((512, 512));
                                    
                                    extracted_textures.push(ExtractedTexture {
                                        name: texture_name,
                                        file_path: format!("assets/models/{}/textures/{}", base_name, texture_filename),
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

fn create_individual_mesh_glb(glb_data: &[u8], mesh_index: usize, mesh_name: &str) -> Result<Vec<u8>, String> {
    info!("🔄 Creating individual GLB for mesh: {}", mesh_name);
    
    // For now, just copy the original GLB - in a full implementation this would
    // extract only the specific mesh data and create a minimal GLB
    // TODO: Implement proper mesh extraction using gltf crate
    Ok(glb_data.to_vec())
}

fn create_babylonjs_material(gltf_material: &serde_json::Value, material_name: &str, extracted_textures: &[ExtractedTexture], base_name: &str) -> Result<serde_json::Value, String> {
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
            "name": format!("assets/models/{}/textures/", base_name) + &std::path::Path::new(path).file_name().unwrap().to_string_lossy(),
            "url": format!("/projects/{}/assets/models/{}/textures/{}", 
                std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                base_name,
                std::path::Path::new(path).file_name().unwrap().to_string_lossy()),
            "level": 1.0,
            "hasAlpha": true,
            "getAlphaFromRGB": false,
            "coordinatesIndex": 0,
            "wrapU": 1,
            "wrapV": 1,
            "samplingMode": 3
        })),
        
        "normalTexture": normal_texture.map(|path| serde_json::json!({
            "name": format!("assets/models/{}/textures/", base_name) + &std::path::Path::new(path).file_name().unwrap().to_string_lossy(),
            "url": format!("/projects/{}/assets/models/{}/textures/{}", 
                std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                base_name,
                std::path::Path::new(path).file_name().unwrap().to_string_lossy()),
            "level": 1.0,
            "coordinatesIndex": 0,
            "wrapU": 1,
            "wrapV": 1,
            "samplingMode": 3
        })),
        
        "metallicRoughnessTexture": metallic_roughness_texture.map(|path| serde_json::json!({
            "name": format!("assets/models/{}/textures/", base_name) + &std::path::Path::new(path).file_name().unwrap().to_string_lossy(),
            "url": format!("/projects/{}/assets/models/{}/textures/{}", 
                std::env::var("PROJECT_NAME").unwrap_or_else(|_| "current".to_string()),
                base_name,
                std::path::Path::new(path).file_name().unwrap().to_string_lossy()),
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