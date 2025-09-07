use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use tracing::{info, error, warn};
use chrono::{DateTime, Utc};
use serde_json;

use crate::types::{ProjectInfo, FileInfo};
use crate::state::get_projects_path;

pub async fn list_projects() -> Result<Vec<ProjectInfo>, Box<dyn std::error::Error>> {
    let projects_path = get_projects_path();
    
    if !projects_path.exists() {
        return Ok(vec![]);
    }
    
    let mut projects = Vec::new();
    let mut entries = fs::read_dir(&projects_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        
        if path.is_dir() {
            match load_project_info(&path).await {
                Ok(project_info) => projects.push(project_info),
                Err(e) => {
                    warn!("Failed to load project at {:?}: {}", path, e);
                }
            }
        }
    }
    
    // Sort projects by name
    projects.sort_by(|a, b| a.name.cmp(&b.name));
    
    info!("📋 Listed {} projects", projects.len());
    Ok(projects)
}

pub async fn create_project(
    name: &str,
    template: &str,
) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(name);
    
    if project_path.exists() {
        return Err(format!("Project '{}' already exists", name).into());
    }
    
    // Create project directory structure
    fs::create_dir_all(&project_path).await?;
    fs::create_dir_all(project_path.join("assets")).await?;
    fs::create_dir_all(project_path.join("scenes")).await?;
    fs::create_dir_all(project_path.join("scripts")).await?;
    fs::create_dir_all(project_path.join("settings")).await?;
    
    let now = Utc::now();
    
    // Create project.json
    let project_config = serde_json::json!({
        "name": name,
        "version": "1.0.0",
        "engine_version": env!("CARGO_PKG_VERSION"),
        "template": template,
        "created": now,
        "modified": now,
        "settings": {
            "renderer": "babylon",
            "physics": "cannon",
            "audio": "webaudio"
        }
    });
    
    let config_path = project_path.join("project.json");
    fs::write(&config_path, serde_json::to_string_pretty(&project_config)?).await?;
    
    // Create a sample scene if using basic template
    if template == "basic" {
        let scene_content = serde_json::json!({
            "name": "Main Scene",
            "objects": [],
            "lights": [
                {
                    "type": "directional",
                    "name": "Sun",
                    "position": [0, 10, 0],
                    "intensity": 1.0
                }
            ],
            "camera": {
                "type": "perspective",
                "position": [0, 5, 10],
                "target": [0, 0, 0]
            }
        });
        
        let scene_path = project_path.join("scenes").join("main.scene");
        fs::write(&scene_path, serde_json::to_string_pretty(&scene_content)?).await?;
    }
    
    // Load and return the created project
    let project_info = load_project_info(&project_path).await?;
    info!("🎮 Created project: {}", name);
    
    Ok(project_info)
}

pub async fn load_project_info(project_path: &Path) -> Result<ProjectInfo, Box<dyn std::error::Error>> {
    let config_path = project_path.join("project.json");
    
    if !config_path.exists() {
        return Err("project.json not found".into());
    }
    
    let config_content = fs::read_to_string(&config_path).await?;
    let config: serde_json::Value = serde_json::from_str(&config_content)?;
    
    let name = config["name"].as_str().unwrap_or("Unknown").to_string();
    let created = config["created"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    let modified = config["modified"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);
    
    // Get project settings
    let settings = config["settings"]
        .as_object()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .collect::<HashMap<String, serde_json::Value>>();
    
    // List all files in project
    let files = list_project_files(project_path).await?;
    
    Ok(ProjectInfo {
        name,
        path: project_path.to_string_lossy().to_string(),
        created,
        modified,
        files,
        settings,
    })
}

async fn list_project_files(project_path: &Path) -> Result<Vec<FileInfo>, Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    collect_files_recursive(project_path, project_path, &mut files).await?;
    Ok(files)
}

fn collect_files_recursive<'a>(
    base_path: &'a Path,
    current_path: &'a Path,
    files: &'a mut Vec<FileInfo>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error>>> + 'a>> {
    Box::pin(async move {
    let mut entries = fs::read_dir(current_path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let metadata = entry.metadata().await?;
        
        let relative_path = path.strip_prefix(base_path)?;
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let file_info = FileInfo {
            name: name.clone(),
            path: relative_path.to_string_lossy().to_string(),
            is_directory: metadata.is_dir(),
            size: if metadata.is_file() { Some(metadata.len()) } else { None },
            modified: metadata.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_else(Utc::now)),
            extension: if metadata.is_file() {
                path.extension().and_then(|ext| ext.to_str()).map(|s| format!(".{}", s))
            } else {
                None
            },
        };
        
        files.push(file_info);
        
        // Recurse into subdirectories
        if metadata.is_dir() {
            collect_files_recursive(base_path, &path, files).await?;
        }
    }
    
    Ok(())
    })
}

pub async fn list_directory_contents(dir_path: &str) -> Result<Vec<FileInfo>, Box<dyn std::error::Error>> {
    let path = Path::new(dir_path);
    
    if !path.exists() {
        return Err("Directory does not exist".into());
    }
    
    if !path.is_dir() {
        return Err("Path is not a directory".into());
    }
    
    let mut files = Vec::new();
    let mut entries = fs::read_dir(path).await?;
    
    while let Some(entry) = entries.next_entry().await? {
        let entry_path = entry.path();
        let metadata = entry.metadata().await?;
        
        let name = entry_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();
        
        let file_info = FileInfo {
            name: name.clone(),
            path: entry_path.to_string_lossy().to_string(),
            is_directory: metadata.is_dir(),
            size: if metadata.is_file() { Some(metadata.len()) } else { None },
            modified: metadata.modified().ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0).unwrap_or_else(Utc::now)),
            extension: if metadata.is_file() {
                entry_path.extension().and_then(|ext| ext.to_str()).map(|s| format!(".{}", s))
            } else {
                None
            },
        };
        
        files.push(file_info);
    }
    
    // Sort: directories first, then files, both alphabetically
    files.sort_by(|a, b| {
        match (a.is_directory, b.is_directory) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });
    
    info!("📁 Listed directory: {} ({} items)", dir_path, files.len());
    Ok(files)
}

pub async fn delete_project(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let projects_path = get_projects_path();
    let project_path = projects_path.join(name);
    
    if !project_path.exists() {
        return Err(format!("Project '{}' does not exist", name).into());
    }
    
    fs::remove_dir_all(&project_path).await?;
    info!("🗑️ Deleted project: {}", name);
    Ok(())
}