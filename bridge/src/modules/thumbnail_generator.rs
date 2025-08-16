use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailCache {
    pub thumbnails: HashMap<String, CachedThumbnail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedThumbnail {
    pub file_path: String,
    pub file_hash: String,
    pub thumbnail_data: String, // Base64 encoded image
    pub generated_at: u64,
    pub file_size: u64,
    pub last_modified: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailRequest {
    pub project_name: String,
    pub asset_path: String,
    pub size: Option<u32>, // Optional thumbnail size (default 512)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailResponse {
    pub success: bool,
    pub thumbnail_data: Option<String>, // Base64 encoded PNG
    pub cached: bool,
    pub error: Option<String>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            thumbnails: HashMap::new(),
        }
    }

    pub fn load_from_file(cache_path: &Path) -> Self {
        if cache_path.exists() {
            match fs::read_to_string(cache_path) {
                Ok(content) => {
                    match serde_json::from_str(&content) {
                        Ok(cache) => cache,
                        Err(e) => {
                            println!("Failed to parse thumbnail cache: {}", e);
                            Self::new()
                        }
                    }
                }
                Err(e) => {
                    println!("Failed to read thumbnail cache: {}", e);
                    Self::new()
                }
            }
        } else {
            Self::new()
        }
    }

    pub fn save_to_file(&self, cache_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        fs::write(cache_path, content)?;
        Ok(())
    }

    pub fn get_thumbnail(&self, key: &str) -> Option<&CachedThumbnail> {
        self.thumbnails.get(key)
    }

    pub fn set_thumbnail(&mut self, key: String, thumbnail: CachedThumbnail) {
        self.thumbnails.insert(key, thumbnail);
    }

    pub fn is_valid(&self, key: &str, file_path: &Path) -> bool {
        if let Some(cached) = self.get_thumbnail(key) {
            if file_path.exists() {
                if let Ok(metadata) = fs::metadata(file_path) {
                    let current_modified = metadata.modified()
                        .unwrap_or(std::time::UNIX_EPOCH)
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    
                    return cached.last_modified == current_modified && 
                           cached.file_size == metadata.len();
                }
            }
        }
        false
    }

    pub fn cleanup_old_entries(&mut self, max_age_days: u64) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let max_age_seconds = max_age_days * 24 * 60 * 60;
        
        self.thumbnails.retain(|_, thumbnail| {
            current_time - thumbnail.generated_at < max_age_seconds
        });
    }
}

pub fn get_cache_path(project_name: &str) -> PathBuf {
    let base_path = crate::get_projects_path();
    base_path.join(project_name).join(".cache").join("thumbnails.json")
}

pub fn generate_cache_key(project_name: &str, asset_path: &str) -> String {
    format!("{}::{}", project_name, asset_path)
}

// For now, we'll create a placeholder thumbnail generator
// This would be replaced with actual 3D rendering logic
pub async fn generate_model_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        return Err("Asset file not found".into());
    }

    // For now, return a placeholder thumbnail
    // TODO: Implement actual 3D model rendering using wgpu or similar
    generate_placeholder_thumbnail(size)
}

fn generate_placeholder_thumbnail(size: u32) -> Result<String, Box<dyn std::error::Error>> {
    // Create a simple placeholder SVG image
    let placeholder_svg = format!(
        r#"<svg width="{}" height="{}" xmlns="http://www.w3.org/2000/svg">
            <rect width="100%" height="100%" fill="{}"/>
            <rect x="20%" y="20%" width="60%" height="60%" fill="{}" rx="10"/>
            <circle cx="50%" cy="40%" r="8%" fill="{}"/>
            <rect x="35%" y="55%" width="30%" height="8%" fill="{}" rx="4"/>
            <text x="50%" y="75%" text-anchor="middle" fill="{}" font-family="Arial" font-size="12">3D Model</text>
        </svg>"#,
        size, size, "#4a5568", "#718096", "#e2e8f0", "#e2e8f0", "#e2e8f0"
    );
    
    // Convert SVG to base64 (placeholder)
    let base64_data = general_purpose::STANDARD.encode(placeholder_svg.as_bytes());
    Ok(format!("data:image/svg+xml;base64,{}", base64_data))
}

pub async fn get_or_generate_thumbnail(request: ThumbnailRequest) -> ThumbnailResponse {
    let cache_key = generate_cache_key(&request.project_name, &request.asset_path);
    let cache_path = get_cache_path(&request.project_name);
    let size = request.size.unwrap_or(512);
    
    // Load cache
    let mut cache = ThumbnailCache::load_from_file(&cache_path);
    
    // Clean up old entries (older than 30 days)
    cache.cleanup_old_entries(30);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(&request.project_name).join(&request.asset_path);
    
    // Check if we have a valid cached thumbnail
    if cache.is_valid(&cache_key, &full_asset_path) {
        if let Some(cached_thumbnail) = cache.get_thumbnail(&cache_key) {
            return ThumbnailResponse {
                success: true,
                thumbnail_data: Some(cached_thumbnail.thumbnail_data.clone()),
                cached: true,
                error: None,
            };
        }
    }
    
    // Generate new thumbnail
    match generate_model_thumbnail(&request.project_name, &request.asset_path, size).await {
        Ok(thumbnail_data) => {
            // Cache the generated thumbnail
            if let Ok(metadata) = fs::metadata(&full_asset_path) {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let last_modified = metadata.modified()
                    .unwrap_or(std::time::UNIX_EPOCH)
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                
                let cached_thumbnail = CachedThumbnail {
                    file_path: request.asset_path.clone(),
                    file_hash: format!("{}", current_time), // Simple timestamp as hash placeholder
                    thumbnail_data: thumbnail_data.clone(),
                    generated_at: current_time,
                    file_size: metadata.len(),
                    last_modified,
                };
                
                cache.set_thumbnail(cache_key, cached_thumbnail);
                
                // Save cache to disk
                if let Err(e) = cache.save_to_file(&cache_path) {
                    println!("Failed to save thumbnail cache: {}", e);
                }
            }
            
            ThumbnailResponse {
                success: true,
                thumbnail_data: Some(thumbnail_data),
                cached: false,
                error: None,
            }
        }
        Err(e) => ThumbnailResponse {
            success: false,
            thumbnail_data: None,
            cached: false,
            error: Some(e.to_string()),
        }
    }
}