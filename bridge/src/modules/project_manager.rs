use std::path::PathBuf;
use std::fs;
use std::env;
use crate::types::{ProjectInfo, FileInfo};
use log::{info, error, debug};

pub fn get_base_path() -> PathBuf {
    env::current_dir().unwrap_or_else(|_| PathBuf::from(".."))
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

pub fn create_project(name: &str, template: &str) -> Result<ProjectInfo, String> {
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
    
    let standard_folders = match template {
        "basic" => vec!["models", "textures", "materials", "scripts", "audio", "video", "images"],
        _ => vec!["models", "textures", "materials", "scripts", "audio", "video", "images"],
    };
    
    for folder in standard_folders {
        let folder_path = assets_path.join(folder);
        if let Err(e) = fs::create_dir_all(&folder_path) {
            return Err(format!("Failed to create assets folder '{}': {}", folder, e));
        }
    }
    
    // Create a comprehensive project.json file
    let project_file_path = project_path.join("project.json");
    let project_config = serde_json::json!({
        "name": name,
        "version": "1.0.0",
        "created": chrono::Utc::now().to_rfc3339(),
        "last_modified": chrono::Utc::now().to_rfc3339(),
        "template": template,
        "description": "",
        "author": "",
        "engine_version": "1.0.0",
        "settings": {
            "render": {
                "resolution": { "width": 1920, "height": 1080 },
                "quality": "high"
            },
            "physics": {
                "enabled": true,
                "gravity": -9.81
            }
        },
        "assets_directory": "assets"
    });
    
    if let Err(e) = fs::write(&project_file_path, serde_json::to_string_pretty(&project_config).unwrap()) {
        println!("Warning: Failed to create project.json: {}", e);
    }
    
    // Return the created project info
    Ok(ProjectInfo {
        name: name.to_string(),
        path: format!("projects/{}", name),
        files: Vec::new(),
    })
}