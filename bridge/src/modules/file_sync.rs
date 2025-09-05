use std::fs;
use std::path::Path;
use crate::types::{WriteFileRequest, WriteBinaryFileRequest};
use base64::{Engine as _, engine::general_purpose};
use crate::project_manager::get_projects_path;
use log::{info, warn, error, debug};

const BLACKLISTED_EXTENSIONS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "ps1", "vbs", "vbe", "js", "jse", "jar", "msi",
    "dll", "sys", "drv", "ocx", "cpl", "inf", "reg", "scf", "lnk", "url", "desktop", "app",
    "deb", "rpm", "dmg", "pkg", "apk", "ipa", "bin", "run", "out", "sh", "bash", "zsh",
    "fish", "csh", "tcsh", "py", "pl", "rb", "php", "asp", "aspx", "jsp", "cgi"
];

fn is_file_extension_allowed(file_path: &Path) -> Result<(), String> {
    if let Some(extension) = file_path.extension().and_then(|s| s.to_str()) {
        let ext_lower = extension.to_lowercase();
        if BLACKLISTED_EXTENSIONS.contains(&ext_lower.as_str()) {
            return Err(format!("File extension '{}' is not allowed for security reasons", extension));
        }
    }
    Ok(())
}

pub fn sanitize_file_name(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '.' => c,
            ' ' => '_',
            _ => '_',
        })
        .collect::<String>()
        .split('_')
        .filter(|s| !s.is_empty())
        .collect::<Vec<&str>>()
        .join("_")
        .to_lowercase()
}

pub fn read_file_content(file_path: &str) -> Result<String, String> {
    info!("📖 Reading file: {}", file_path);
    
    let full_path = if file_path.starts_with("projects/") {
        // Project file - use existing logic
        let projects_path = get_projects_path();
        
        // Parse the path to extract project name and asset path
        let path_parts: Vec<&str> = file_path.split('/').collect();
        if path_parts.len() < 2 {
            error!("❌ Invalid project path format: {} (expected projects/{{project_name}}/...)", file_path);
            return Err("Invalid project path format. Expected projects/{project_name}/...".to_string());
        }
        
        let project_name = path_parts[1];
        let asset_path = if path_parts.len() > 2 {
            path_parts[2..].join("/")
        } else {
            error!("❌ Asset path required for: {}", file_path);
            return Err("Asset path required".to_string());
        };
        
        projects_path.join(project_name).join(&asset_path)
    } else {
        // Non-project file - use engine root (for src/renscripts, etc.)
        let base_path = crate::project_manager::get_base_path();
        base_path.join(file_path)
    };
    
    debug!("📂 Full file path: {:?}", full_path);
    
    // Security validation
    if let Err(e) = is_file_extension_allowed(&full_path) {
        error!("🚫 Security: {}", e);
        return Err(e);
    }
    
    match fs::read_to_string(&full_path) {
        Ok(content) => {
            let file_size = content.len();
            info!("✅ Successfully read file: {} ({} bytes)", file_path, file_size);
            Ok(content)
        },
        Err(e) => {
            error!("❌ Failed to read file: {} - Error: {}", file_path, e);
            Err("Failed to read file".to_string())
        }
    }
}

pub fn write_file_content(file_path: &str, request: &WriteFileRequest) -> Result<(), String> {
    let projects_path = get_projects_path();
    let content_size = request.content.len();
    
    // Security validation
    let full_path = projects_path.join(file_path);
    if let Err(e) = is_file_extension_allowed(&full_path) {
        error!("🚫 Security: {}", e);
        return Err(e);
    }
    info!("💾 Writing file: {} ({} bytes)", file_path, content_size);
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        error!("❌ Invalid path format: {} (expected projects/{{project_name}}/...)", file_path);
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        error!("❌ Asset path required for: {}", file_path);
        return Err("Asset path required".to_string());
    };
    
    // Construct full path within project directory
    let project_assets_path = projects_path.join(project_name).join(&asset_path);
    debug!("📂 Full file path: {:?}", project_assets_path);
    
    if let Some(parent) = project_assets_path.parent() {
        match fs::create_dir_all(parent) {
            Ok(_) => debug!("📁 Created directory structure: {:?}", parent),
            Err(e) => {
                error!("❌ Failed to create directories for: {:?} - Error: {}", parent, e);
                return Err("Failed to create directories".to_string());
            }
        }
    }

    match fs::write(&project_assets_path, &request.content) {
        Ok(_) => {
            info!("✅ Successfully wrote file: {} ({} bytes)", file_path, content_size);
            Ok(())
        },
        Err(e) => {
            error!("❌ Failed to write file: {} - Error: {}", file_path, e);
            Err("Failed to write file".to_string())
        }
    }
}

pub fn write_binary_file_content(file_path: &str, request: &WriteBinaryFileRequest) -> Result<(), String> {
    let projects_path = get_projects_path();
    info!("💾 Writing binary file: {}", file_path);
    
    if request.base64_content.is_empty() {
        error!("❌ Missing base64_content for file: {}", file_path);
        return Err("Missing base64_content".to_string());
    }
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        error!("❌ Invalid path format: {} (expected projects/{{project_name}}/...)", file_path);
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        error!("❌ Asset path required for: {}", file_path);
        return Err("Asset path required".to_string());
    };
    
    // Construct full path within project directory
    let project_assets_path = projects_path.join(project_name).join(&asset_path);
    debug!("📂 Full binary file path: {:?}", project_assets_path);
    
    // Security validation
    if let Err(e) = is_file_extension_allowed(&project_assets_path) {
        error!("🚫 Security: {}", e);
        return Err(e);
    }
    
    if let Some(parent) = project_assets_path.parent() {
        match fs::create_dir_all(parent) {
            Ok(_) => debug!("📁 Created directory structure for binary file: {:?}", parent),
            Err(e) => {
                error!("❌ Failed to create directories for binary file: {:?} - Error: {}", parent, e);
                return Err("Failed to create directories".to_string());
            }
        }
    }

    let binary_data = match general_purpose::STANDARD.decode(&request.base64_content) {
        Ok(data) => {
            let decoded_size = data.len();
            info!("🔧 Decoded base64 data: {} bytes", decoded_size);
            data
        },
        Err(e) => {
            error!("❌ Failed to decode base64 for file: {} - Error: {}", file_path, e);
            return Err("Failed to decode base64".to_string());
        }
    };

    match fs::write(&project_assets_path, &binary_data) {
        Ok(_) => {
            info!("✅ Successfully wrote binary file: {} ({} bytes)", file_path, binary_data.len());
            Ok(())
        },
        Err(e) => {
            error!("❌ Failed to write binary file: {} - Error: {}", file_path, e);
            Err("Failed to write binary file".to_string())
        }
    }
}

pub fn delete_file_or_directory(file_path: &str) -> Result<(), String> {
    let projects_path = get_projects_path();
    info!("🗑️ Deleting: {}", file_path);
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        error!("❌ Invalid path format: {} (expected projects/{{project_name}}/...)", file_path);
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        error!("❌ Asset path required for: {}", file_path);
        return Err("Asset path required".to_string());
    };
    
    // Construct full path within project directory
    let project_assets_path = projects_path.join(project_name).join(&asset_path);
    debug!("📂 Full delete path: {:?}", project_assets_path);
    
    if !project_assets_path.exists() {
        warn!("⚠️ Delete target does not exist: {}", file_path);
        return Err("File or directory not found".to_string());
    }
    
    let is_directory = project_assets_path.is_dir();
    let result = if is_directory {
        info!("📁 Deleting directory: {}", file_path);
        fs::remove_dir_all(&project_assets_path)
    } else {
        // Get file size before deletion for logging
        let file_size = match fs::metadata(&project_assets_path) {
            Ok(metadata) => metadata.len(),
            Err(_) => 0,
        };
        info!("📄 Deleting file: {} ({} bytes)", file_path, file_size);
        fs::remove_file(&project_assets_path)
    };

    match result {
        Ok(_) => {
            info!("✅ Successfully deleted {}: {}", if is_directory { "directory" } else { "file" }, file_path);
            Ok(())
        },
        Err(e) => {
            error!("❌ Failed to delete {}: {} - Error: {}", if is_directory { "directory" } else { "file" }, file_path, e);
            Err("Failed to delete".to_string())
        }
    }
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
    info!("📖 Reading binary file: {}", file_path);
    
    // Parse the path to extract project name and asset path
    let path_parts: Vec<&str> = file_path.split('/').collect();
    if path_parts.len() < 2 || path_parts[0] != "projects" {
        error!("❌ Invalid path format: {} (expected projects/{{project_name}}/...)", file_path);
        return Err("Invalid path format. Expected projects/{project_name}/...".to_string());
    }
    
    let project_name = path_parts[1];
    let asset_path = if path_parts.len() > 2 {
        path_parts[2..].join("/")
    } else {
        error!("❌ Asset path required for: {}", file_path);
        return Err("Asset path required".to_string());
    };
    
    // Construct full path within project directory
    let project_assets_path = projects_path.join(project_name).join(&asset_path);
    debug!("📂 Full binary file path: {:?}", project_assets_path);
    
    // Security validation
    if let Err(e) = is_file_extension_allowed(&project_assets_path) {
        error!("🚫 Security: {}", e);
        return Err(e);
    }
    
    if !project_assets_path.exists() {
        warn!("⚠️ Binary file not found: {}", file_path);
        return Err("File not found".to_string());
    }

    match fs::read(&project_assets_path) {
        Ok(data) => {
            let file_size = data.len();
            let file_type = get_file_content_type(&project_assets_path);
            info!("✅ Successfully read binary file: {} ({} bytes, type: {})", file_path, file_size, file_type);
            Ok(data)
        },
        Err(e) => {
            error!("❌ Failed to read binary file: {} - Error: {}", file_path, e);
            Err("Failed to read binary file".to_string())
        }
    }
}