use std::path::PathBuf;
use std::fs;
use std::env;
use crate::types::{ProjectInfo, FileInfo};
use log::{info, error, debug, warn};
use base64::{Engine as _, engine::general_purpose};
use chrono;

pub fn get_base_path() -> PathBuf {
    // Check for ENGINE_ROOT environment variable first
    if let Ok(engine_root) = env::var("ENGINE_ROOT") {
        return PathBuf::from(engine_root);
    }
    
    // Fall back to current directory, but if we're in the bridge subdirectory,
    // go up one level to find the engine root
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    // If current directory is named "bridge", go up one level
    if current_dir.file_name().and_then(|name| name.to_str()) == Some("bridge") {
        current_dir.parent().map(|p| p.to_path_buf()).unwrap_or(current_dir)
    } else {
        current_dir
    }
}

pub fn get_projects_path() -> PathBuf {
    get_base_path().join("projects")
}

pub fn list_projects() -> Result<Vec<ProjectInfo>, String> {
    let projects_path = get_projects_path();
    let mut projects = Vec::new();
    info!("📋 Listing projects from: {:?}", projects_path);

    if !projects_path.exists() {
        info!("📁 Projects directory doesn't exist, creating it...");
        if let Err(e) = fs::create_dir_all(&projects_path) {
            error!("❌ Failed to create projects directory: {}", e);
            return Err("Failed to create projects directory".to_string());
        }
        info!("✅ Created projects directory");
    }

    match fs::read_dir(&projects_path) {
        Ok(entries) => {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        debug!("📁 Found project: {}", name);
                        projects.push(ProjectInfo {
                            name: name.to_string(),
                            path: format!("projects/{}", name),
                            files: Vec::new(),
                        });
                    }
                }
            }
            info!("✅ Listed {} projects", projects.len());
            Ok(projects)
        },
        Err(e) => {
            error!("❌ Failed to read projects directory: {}", e);
            Err("Failed to read projects directory".to_string())
        }
    }
}

pub fn list_directory_contents(dir_path: &str) -> Result<Vec<FileInfo>, String> {
    let base_path = get_base_path();
    
    // Allow access to any directory under the base path (engine root)
    let target_path = base_path.join(dir_path);
    
    if !target_path.exists() {
        return Ok(Vec::new()); // Return empty list if directory doesn't exist
    }

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(&target_path) {
        for entry in entries.flatten() {
            let file_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = file_path.is_dir();
            let size = if is_dir { 0 } else { 
                file_path.metadata().map(|m| m.len()).unwrap_or(0) 
            };
            
            let relative_path = format!("{}/{}", dir_path, name);
            
            files.push(FileInfo {
                name,
                path: relative_path,
                is_directory: is_dir,
                size,
            });
        }
    }

    Ok(files)
}

pub fn create_project(name: &str, template: &str, settings: Option<&serde_json::Value>) -> Result<ProjectInfo, String> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(name);
    
    // Check if project already exists
    if project_path.exists() {
        return Err(format!("Project '{}' already exists", name));
    }
    
    // Validate project name
    if name.is_empty() || name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|']) {
        return Err("Invalid project name. Avoid special characters.".to_string());
    }
    
    // Create project directory structure
    if let Err(e) = fs::create_dir_all(&project_path) {
        return Err(format!("Failed to create project directory: {}", e));
    }
    
    // Create assets directory and standard subfolders
    let assets_path = project_path.join("assets");
    if let Err(e) = fs::create_dir_all(&assets_path) {
        return Err(format!("Failed to create assets directory: {}", e));
    }
    
    // Create scenes directory
    let scenes_path = project_path.join("scenes");
    if let Err(e) = fs::create_dir_all(&scenes_path) {
        return Err(format!("Failed to create scenes directory: {}", e));
    }
    
    // Determine folders to create based on settings or template
    let folders_to_create = if let Some(settings) = settings {
        if let Some(folders_obj) = settings.get("folders") {
            // Use custom folder settings
            let mut folders = Vec::new();
            if folders_obj.get("models").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("models");
            }
            if folders_obj.get("textures").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("textures");
            }
            if folders_obj.get("materials").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("materials");
            }
            if folders_obj.get("scripts").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("scripts");
            }
            if folders_obj.get("audio").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("audio");
            }
            if folders_obj.get("video").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("video");
            }
            if folders_obj.get("images").and_then(|v| v.as_bool()).unwrap_or(false) {
                folders.push("images");
            }
            folders
        } else {
            // Default based on template
            match template {
                "minimal" => vec!["models", "materials", "scripts"],
                "game" => vec!["models", "textures", "materials", "scripts", "audio", "video", "images"],
                _ => vec!["models", "textures", "materials", "scripts", "audio"],
            }
        }
    } else {
        // Default based on template
        match template {
            "minimal" => vec!["models", "materials", "scripts"],
            "game" => vec!["models", "textures", "materials", "scripts", "audio", "video", "images"],
            _ => vec!["models", "textures", "materials", "scripts", "audio"],
        }
    };
    
    for folder in folders_to_create {
        let folder_path = assets_path.join(folder);
        if let Err(e) = fs::create_dir_all(&folder_path) {
            return Err(format!("Failed to create assets folder '{}': {}", folder, e));
        }
    }
    
    // Create project.json with custom settings
    let project_file_path = project_path.join("project.json");
    
    let physics_enabled = settings
        .and_then(|s| s.get("physics"))
        .and_then(|p| p.as_bool())
        .unwrap_or(true);
    
    let resolution = settings
        .and_then(|s| s.get("resolution"))
        .map(|r| (
            r.get("width").and_then(|w| w.as_u64()).unwrap_or(1920),
            r.get("height").and_then(|h| h.as_u64()).unwrap_or(1080)
        ))
        .unwrap_or((1920, 1080));
    
    let project_config = serde_json::json!({
        "name": name,
        "version": "1.0.0",
        "created": chrono::Utc::now().to_rfc3339(),
        "last_modified": chrono::Utc::now().to_rfc3339(),
        "template": template,
        "description": "",
        "author": "",
        "engine_version": "1.0.0",
        "currentScene": "main",
        "settings": {
            "render": {
                "resolution": { "width": resolution.0, "height": resolution.1 },
                "quality": "high"
            },
            "physics": {
                "enabled": physics_enabled,
                "gravity": -9.81
            }
        },
        "assets_directory": "assets"
    });
    
    if let Err(e) = fs::write(&project_file_path, serde_json::to_string_pretty(&project_config).unwrap()) {
        println!("Warning: Failed to create project.json: {}", e);
    }
    
    // Create default scene file with default camera
    let default_scene_path = scenes_path.join("main.json");
    let default_scene = serde_json::json!({
        "hierarchy": [{
            "id": "scene-root",
            "name": "main",
            "type": "scene",
            "expanded": true,
            "children": [{
                "id": 1,
                "name": "camera",
                "type": "camera",
                "lightType": null,
                "visible": true,
                "expanded": true,
                "babylonData": {
                    "name": "camera",
                    "id": "camera",
                    "uniqueId": 1,
                    "type": "UniversalCamera",
                    "position": [7, 8, -7],
                    "rotation": [0, 0, 0],
                    "target": [0, 2, 0],
                    "fov": 0.8,
                    "minZ": 1,
                    "maxZ": 10000,
                    "metadata": {
                        "properties": {},
                        "originalProperties": {
                            "position": [7, 8, -7],
                            "rotation": [0, 0, 0],
                            "scale": [1, 1, 1]
                        }
                    },
                    "__engineObjectId": 1,
                    "__engineClassName": "UniversalCamera",
                    "__engineName": "camera",
                    "__attachedScripts": []
                }
            }]
        }],
        "lighting": {
            "sunIntensity": 4.0,
            "skyIntensity": 4.0,
            "rimIntensity": 0.4,
            "timeOfDay": 12.0,
            "timeEnabled": true
        },
        "settings": {
            "backgroundColor": "#1a202c",
            "enableGrid": true,
            "gridSize": 10
        },
        "metadata": {
            "name": "main",
            "created": chrono::Utc::now().to_rfc3339(),
            "engineVersion": "1.0.0"
        }
    });
    
    if let Err(e) = fs::write(&default_scene_path, serde_json::to_string_pretty(&default_scene).unwrap()) {
        println!("Warning: Failed to create default scene: {}", e);
    } else {
        info!("✅ Created default scene: main.json");
    }
    
    // Return the created project info
    Ok(ProjectInfo {
        name: name.to_string(),
        path: format!("projects/{}", name),
        files: Vec::new(),
    })
}

pub fn delete_project(project_name: &str) -> Result<(), String> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    
    // Check if project exists
    if !project_path.exists() {
        return Err(format!("Project '{}' does not exist", project_name));
    }
    
    // Validate project name to prevent directory traversal
    if project_name.is_empty() || project_name.contains(['/', '\\', ':', '*', '?', '"', '<', '>', '|', '.']) {
        return Err("Invalid project name".to_string());
    }
    
    // Remove the entire project directory
    if let Err(e) = fs::remove_dir_all(&project_path) {
        return Err(format!("Failed to delete project directory: {}", e));
    }
    
    info!("✅ Successfully deleted project: {}", project_name);
    Ok(())
}

pub fn load_scene_with_assets(project_name: &str, scene_name: &str) -> Result<serde_json::Value, String> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(project_name);
    let scene_path = project_path.join("scenes").join(format!("{}.json", scene_name));
    
    info!("🔍 Loading scene bundle for project '{}', scene '{}'", project_name, scene_name);
    info!("📁 Scene file path: {:?}", scene_path);
    
    // Read scene file
    let scene_content = match fs::read_to_string(&scene_path) {
        Ok(content) => {
            info!("📄 Scene file read successfully, size: {} bytes", content.len());
            content
        },
        Err(e) => {
            error!("❌ Failed to read scene file: {}", e);
            return Err(format!("Failed to read scene file: {}", e));
        }
    };
    
    // Parse scene JSON to find asset references - use streaming/targeted parsing
    let scene_json: serde_json::Value = match serde_json::from_str(&scene_content) {
        Ok(json) => {
            info!("✅ Scene JSON parsed successfully");
            json
        },
        Err(e) => {
            error!("❌ Failed to parse scene JSON: {}", e);
            return Err(format!("Failed to parse scene JSON: {}", e));
        }
    };
    
    // Log basic scene structure
    if let Some(hierarchy) = scene_json.get("hierarchy") {
        info!("🌳 Scene hierarchy found with {} items", 
              hierarchy.as_array().map_or(0, |arr| arr.len()));
    } else {
        warn!("⚠️ No hierarchy found in scene JSON");
    }
    
    // Extract asset paths using improved recursive search with depth limits
    let mut asset_paths = std::collections::HashSet::new();
    extract_asset_paths_recursive(&scene_json, &mut asset_paths);
    
    info!("🔍 Found {} asset references in scene '{}'", asset_paths.len(), scene_name);
    if !asset_paths.is_empty() {
        info!("📄 Asset paths found: {:?}", asset_paths);
    } else {
        warn!("⚠️ No asset paths found in scene '{}' - asset extraction may have failed", scene_name);
    }
    
    // Read each referenced asset file
    let mut bundled_assets = serde_json::Map::new();
    for asset_path in &asset_paths {
        let full_path = project_path.join(asset_path);
        info!("📂 Resolving asset path: '{}' -> '{:?}'", asset_path, full_path);
        if full_path.exists() {
            match fs::read(&full_path) {
                Ok(asset_data) => {
                    // Convert to base64 for JSON transport
                    let base64_data = general_purpose::STANDARD.encode(&asset_data);
                    bundled_assets.insert(asset_path.clone(), serde_json::Value::String(base64_data));
                    info!("📦 Bundled asset: {}", asset_path);
                },
                Err(e) => {
                    warn!("⚠️ Failed to read asset {}: {}", asset_path, e);
                }
            }
        } else {
            warn!("⚠️ Asset not found: {}", asset_path);
        }
    }
    
    // Extract and compile all RenScripts used in the scene
    let mut script_paths = std::collections::HashSet::new();
    extract_script_paths_recursive(&scene_json, &mut script_paths);
    
    info!("🔍 Found {} script references in scene '{}'", script_paths.len(), scene_name);
    
    // Compile all scripts found in the scene
    let mut compiled_scripts = serde_json::Map::new();
    for script_path in &script_paths {
        info!("📜 Compiling script: {}", script_path);
        match crate::modules::renscript_compiler::compile_renscript(script_path) {
            Ok(compiled_js) => {
                compiled_scripts.insert(script_path.clone(), serde_json::Value::String(compiled_js));
                info!("✅ Compiled script: {}", script_path);
            },
            Err(e) => {
                warn!("⚠️ Failed to compile script {}: {}", script_path, e);
                // Include error info so client can handle it
                compiled_scripts.insert(
                    script_path.clone(), 
                    serde_json::Value::Object({
                        let mut error_obj = serde_json::Map::new();
                        error_obj.insert("error".to_string(), serde_json::Value::String(e));
                        error_obj
                    })
                );
            }
        }
    }
    
    // Create bundled response with scene data + assets + compiled scripts
    let bundled_response = serde_json::json!({
        "scene": scene_json,
        "assets": bundled_assets,
        "scripts": compiled_scripts,
        "project": project_name,
        "sceneName": scene_name,
        "bundledAt": chrono::Utc::now().to_rfc3339(),
        "assetCount": bundled_assets.len(),
        "scriptCount": compiled_scripts.len()
    });
    
    info!("✅ Created scene bundle with {} assets and {} scripts for scene '{}'", bundled_assets.len(), compiled_scripts.len(), scene_name);
    Ok(bundled_response)
}

fn extract_asset_paths_recursive(json: &serde_json::Value, asset_paths: &mut std::collections::HashSet<String>) {
    // Use a depth limit to prevent infinite recursion and limit memory usage
    extract_asset_paths_with_depth(json, asset_paths, 0, 10);
}

fn extract_asset_paths_with_depth(json: &serde_json::Value, asset_paths: &mut std::collections::HashSet<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        debug!("🛑 Reached max recursion depth {}, stopping", max_depth);
        return;
    }

    match json {
        serde_json::Value::Object(map) => {
            // Look for asset source references (can be at top level or in metadata)
            if let Some(asset_source) = map.get("assetSource") {
                if let Some(path) = asset_source.as_str() {
                    if !path.is_empty() {
                        info!("📦 Found assetSource: '{}'", path);
                        asset_paths.insert(path.to_string());
                    }
                }
            }
            
            // Check metadata for asset paths - this is the most common location
            if let Some(metadata) = map.get("metadata") {
                if let Some(metadata_obj) = metadata.as_object() {
                    // Check for direct assetSource in metadata
                    if let Some(asset_source) = metadata_obj.get("assetSource") {
                        if let Some(path) = asset_source.as_str() {
                            if !path.is_empty() {
                                info!("📦 Found metadata.assetSource: '{}'", path);
                                asset_paths.insert(path.to_string());
                            }
                        }
                    }
                    
                    // Check for originalAssetData.path (from asset loader) - most common
                    if let Some(original_asset_data) = metadata_obj.get("originalAssetData") {
                        if let Some(asset_data_obj) = original_asset_data.as_object() {
                            if let Some(asset_path) = asset_data_obj.get("path") {
                                if let Some(path) = asset_path.as_str() {
                                    if !path.is_empty() {
                                        info!("📦 Found metadata.originalAssetData.path: '{}'", path);
                                        asset_paths.insert(path.to_string());
                                    }
                                }
                            }
                        }
                    }
                    
                    // Check for materialSource in metadata
                    if let Some(material_source) = metadata_obj.get("materialSource") {
                        if let Some(path) = material_source.as_str() {
                            if !path.is_empty() {
                                info!("🎨 Found metadata.materialSource: '{}'", path);
                                asset_paths.insert(path.to_string());
                            }
                        }
                    }
                }
            }
            
            // Look for material source paths at top level
            if let Some(material_source) = map.get("materialSource") {
                if let Some(path) = material_source.as_str() {
                    if !path.is_empty() {
                        info!("🎨 Found materialSource: '{}'", path);
                        asset_paths.insert(path.to_string());
                    }
                }
            }
            
            // Look for texture paths in various fields
            let texture_fields = ["baseTexture", "diffuseTexture", "normalTexture", "emissiveTexture", 
                                  "metallicTexture", "roughnessTexture", "occlusionTexture", "hdriPath"];
            for &texture_field in &texture_fields {
                if let Some(texture_path) = map.get(texture_field) {
                    if let Some(path) = texture_path.as_str() {
                        if !path.is_empty() && (path.contains("assets/") || !path.starts_with("http")) {
                            info!("🖼️ Found texture field '{}': '{}'", texture_field, path);
                            // Clean up the path - remove "assets/" prefix if present
                            let clean_path = path.strip_prefix("assets/").unwrap_or(path);
                            asset_paths.insert(clean_path.to_string());
                        }
                    }
                }
            }
            
            // Only recurse into specific important keys to avoid processing huge Babylon data
            let important_keys = ["hierarchy", "children", "babylonData", "metadata"];
            for &key in &important_keys {
                if let Some(value) = map.get(key) {
                    debug!("🔄 Recursing into important key: '{}'", key);
                    extract_asset_paths_with_depth(value, asset_paths, depth + 1, max_depth);
                }
            }
            
            // If we're at the top level, also check the hierarchy array
            if depth == 0 {
                for (key, value) in map.iter() {
                    if key == "hierarchy" {
                        debug!("🔄 Processing top-level hierarchy");
                        extract_asset_paths_with_depth(value, asset_paths, depth + 1, max_depth);
                    }
                }
            }
        },
        serde_json::Value::Array(array) => {
            debug!("🔍 Processing array with {} items at depth {}", array.len(), depth);
            for (index, item) in array.iter().enumerate() {
                debug!("🔄 Processing array index: {} at depth {}", index, depth);
                extract_asset_paths_with_depth(item, asset_paths, depth + 1, max_depth);
            }
        },
        _ => {
            // Don't log every primitive value to avoid spam
        }
    }
}

fn extract_script_paths_recursive(json: &serde_json::Value, script_paths: &mut std::collections::HashSet<String>) {
    extract_script_paths_with_depth(json, script_paths, 0, 10);
}

fn extract_script_paths_with_depth(json: &serde_json::Value, script_paths: &mut std::collections::HashSet<String>, depth: usize, max_depth: usize) {
    if depth > max_depth {
        debug!("🛑 Reached max recursion depth {}, stopping script extraction", max_depth);
        return;
    }

    match json {
        serde_json::Value::Object(map) => {
            // Look for __attachedScripts in babylon data
            if let Some(attached_scripts) = map.get("__attachedScripts") {
                if let Some(scripts_array) = attached_scripts.as_array() {
                    for script_obj in scripts_array {
                        if let Some(script_map) = script_obj.as_object() {
                            if let Some(script_path) = script_map.get("path") {
                                if let Some(path_str) = script_path.as_str() {
                                    if !path_str.is_empty() {
                                        info!("📜 Found attached script: '{}'", path_str);
                                        script_paths.insert(path_str.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            
            // Recurse into important sections
            let important_keys = ["hierarchy", "children", "babylonData"];
            for &key in &important_keys {
                if let Some(value) = map.get(key) {
                    extract_script_paths_with_depth(value, script_paths, depth + 1, max_depth);
                }
            }
        },
        serde_json::Value::Array(array) => {
            for item in array {
                extract_script_paths_with_depth(item, script_paths, depth + 1, max_depth);
            }
        },
        _ => {}
    }
}