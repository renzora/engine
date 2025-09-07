use std::path::Path;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use base64::{Engine as _, engine::general_purpose};
use tracing::{info, error, warn};

use crate::types::{WriteFileRequest, WriteBinaryFileRequest};

pub async fn read_file_content(file_path: &str) -> Result<String, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return Err("File not found".into());
    }
    
    if path.is_dir() {
        return Err("Path is a directory, not a file".into());
    }
    
    let content = fs::read_to_string(path).await?;
    info!("📖 Read file: {} ({} bytes)", file_path, content.len());
    Ok(content)
}

pub async fn read_binary_file(file_path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return Err("File not found".into());
    }
    
    if path.is_dir() {
        return Err("Path is a directory, not a file".into());
    }
    
    let content = fs::read(path).await?;
    info!("📖 Read binary file: {} ({} bytes)", file_path, content.len());
    Ok(content)
}

pub async fn write_file_content(
    file_path: &str,
    request: &WriteFileRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    // Create parent directories if requested
    if request.create_dirs.unwrap_or(false) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
    }
    
    fs::write(path, &request.content).await?;
    info!("💾 Wrote file: {} ({} bytes)", file_path, request.content.len());
    Ok(())
}

pub async fn write_binary_file_content(
    file_path: &str,
    request: &WriteBinaryFileRequest,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    // Decode base64 data
    let binary_data = general_purpose::STANDARD.decode(&request.data)?;
    
    // Create parent directories if requested
    if request.create_dirs.unwrap_or(false) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
    }
    
    fs::write(path, &binary_data).await?;
    info!("💾 Wrote binary file: {} ({} bytes)", file_path, binary_data.len());
    Ok(())
}

pub async fn delete_file_or_directory(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    
    if !path.exists() {
        return Err("Path does not exist".into());
    }
    
    if path.is_dir() {
        fs::remove_dir_all(path).await?;
        info!("🗂️ Deleted directory: {}", file_path);
    } else {
        fs::remove_file(path).await?;
        info!("🗑️ Deleted file: {}", file_path);
    }
    
    Ok(())
}

pub async fn copy_file(
    source_path: &str,
    destination_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = Path::new(source_path);
    let dest = Path::new(destination_path);
    
    if !source.exists() {
        return Err("Source file does not exist".into());
    }
    
    if source.is_dir() {
        return Err("Cannot copy directories with this function".into());
    }
    
    // Create parent directories if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    
    fs::copy(source, dest).await?;
    info!("📋 Copied file: {} -> {}", source_path, destination_path);
    Ok(())
}

pub async fn move_file(
    source_path: &str,
    destination_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = Path::new(source_path);
    let dest = Path::new(destination_path);
    
    if !source.exists() {
        return Err("Source file does not exist".into());
    }
    
    // Create parent directories if needed
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).await?;
    }
    
    // Try rename first (faster if on same filesystem)
    match fs::rename(source, dest).await {
        Ok(_) => {
            info!("🚚 Moved file: {} -> {}", source_path, destination_path);
            Ok(())
        }
        Err(_) => {
            // Fallback to copy + delete
            copy_file(source_path, destination_path).await?;
            delete_file_or_directory(source_path).await?;
            info!("🚚 Moved file (copy+delete): {} -> {}", source_path, destination_path);
            Ok(())
        }
    }
}

pub fn get_file_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).map(|s| s.to_lowercase()).as_deref() {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("json") => "application/json",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("webp") => "image/webp",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("glb") => "model/gltf-binary",
        Some("gltf") => "model/gltf+json",
        Some("txt") | Some("md") => "text/plain",
        Some("xml") => "application/xml",
        _ => "application/octet-stream",
    }
}

pub async fn get_file_size(file_path: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let path = Path::new(file_path);
    let metadata = fs::metadata(path).await?;
    Ok(metadata.len())
}

pub async fn file_exists(file_path: &str) -> bool {
    Path::new(file_path).exists()
}