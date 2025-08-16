use std::fs;
use std::path::Path;
use crate::types::{WriteFileRequest, WriteBinaryFileRequest};
use base64::{Engine as _, engine::general_purpose};
use crate::project_manager::get_projects_path;

pub fn read_file_content(file_path: &str) -> Result<String, String> {
    let projects_path = get_projects_path();
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        return Err("Asset path required".to_string());
    };
    
    // Construct path to project's assets directory
    let project_assets_path = projects_path.join(project_name).join("assets").join(&asset_path);
    
    fs::read_to_string(&project_assets_path)
        .map_err(|_| "Failed to read file".to_string())
}

pub fn write_file_content(file_path: &str, request: &WriteFileRequest) -> Result<(), String> {
    let projects_path = get_projects_path();
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        return Err("Asset path required".to_string());
    };
    
    // Construct path to project's assets directory
    let project_assets_path = projects_path.join(project_name).join("assets").join(&asset_path);
    
    if let Some(parent) = project_assets_path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return Err("Failed to create directories".to_string());
        }
    }

    fs::write(&project_assets_path, &request.content)
        .map_err(|_| "Failed to write file".to_string())
}

pub fn write_binary_file_content(file_path: &str, request: &WriteBinaryFileRequest) -> Result<(), String> {
    let projects_path = get_projects_path();
    
    if request.base64_content.is_empty() {
        return Err("Missing base64_content".to_string());
    }
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        return Err("Asset path required".to_string());
    };
    
    // Construct path to project's assets directory
    let project_assets_path = projects_path.join(project_name).join("assets").join(&asset_path);
    
    if let Some(parent) = project_assets_path.parent() {
        if fs::create_dir_all(parent).is_err() {
            return Err("Failed to create directories".to_string());
        }
    }

    let binary_data = general_purpose::STANDARD.decode(&request.base64_content)
        .map_err(|_| "Failed to decode base64".to_string())?;

    fs::write(&project_assets_path, binary_data)
        .map_err(|_| "Failed to write binary file".to_string())
}

pub fn delete_file_or_directory(file_path: &str) -> Result<(), String> {
    let projects_path = get_projects_path();
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        return Err("Asset path required".to_string());
    };
    
    // Construct path to project's assets directory
    let project_assets_path = projects_path.join(project_name).join("assets").join(&asset_path);
    
    let result = if project_assets_path.is_dir() {
        fs::remove_dir_all(&project_assets_path)
    } else {
        fs::remove_file(&project_assets_path)
    };

    result.map_err(|_| "Failed to delete".to_string())
}

pub fn get_file_content_type(file_path: &Path) -> &'static str {
    match file_path.extension().and_then(|s| s.to_str()) {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("svg") => "image/svg+xml",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("mp4") => "video/mp4",
        Some("obj") => "application/octet-stream",
        Some("fbx") => "application/octet-stream",
        Some("gltf") => "model/gltf+json",
        Some("glb") => "model/gltf-binary",
        _ => "application/octet-stream",
    }
}

pub fn read_binary_file(file_path: &str) -> Result<Vec<u8>, String> {
    let projects_path = get_projects_path();
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        return Err("Asset path required".to_string());
    };
    
    // Construct path to project's assets directory
    let project_assets_path = projects_path.join(project_name).join("assets").join(&asset_path);
    
    if !project_assets_path.exists() {
        return Err("File not found".to_string());
    }

    fs::read(&project_assets_path)
        .map_err(|_| "Failed to read binary file".to_string())
}