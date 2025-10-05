use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{debug, info, error};

#[derive(Debug, Serialize, Deserialize)]
pub struct ThumbnailCache {
    pub thumbnails: HashMap<String, CachedThumbnail>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedThumbnail {
    pub file_path: String,
    pub file_hash: String,
    pub thumbnail_file: String, // Path to actual PNG file
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
    pub thumbnail_file: Option<String>, // Path to PNG file
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
                    // Skip empty files or files with only whitespace
                    if content.trim().is_empty() {
                        debug!("Thumbnail cache file is empty, creating new cache");
                        return Self::new();
                    }
                    
                    match serde_json::from_str(&content) {
                        Ok(cache) => cache,
                        Err(e) => {
                            // Log the error but don't panic - just create a new cache
                            debug!("Failed to parse thumbnail cache ({}), creating new cache: {}", cache_path.display(), e);
                            Self::new()
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to read thumbnail cache file: {}", e);
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
        
        // Use atomic write: write to temp file first, then rename
        let temp_path = cache_path.with_extension("tmp");
        let content = serde_json::to_string_pretty(self)?;
        fs::write(&temp_path, content)?;
        
        // Atomic rename (on most filesystems)
        fs::rename(&temp_path, cache_path)?;
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

pub fn get_thumbnails_dir(project_name: &str) -> PathBuf {
    let base_path = crate::get_projects_path();
    base_path.join(project_name).join(".cache").join("thumbnails")
}

pub fn generate_cache_key(project_name: &str, asset_path: &str) -> String {
    format!("{}::{}", project_name, asset_path)
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
                thumbnail_file: Some(cached_thumbnail.thumbnail_file.clone()),
                cached: true,
                error: None,
            };
        }
    }
    
    // Use new modular thumbnail generation system
    use crate::modules::thumbnail_generators::{get_thumbnail_generator_for_file, ThumbnailGeneratorType};
    use crate::modules::thumbnail_generators::image_types::generate_image_thumbnail;
    use crate::modules::thumbnail_generators::model_types::{generate_model_thumbnail, generate_material_thumbnail};
    use crate::modules::thumbnail_generators::game_engine_types::{generate_hdr_environment_thumbnail, generate_game_texture_thumbnail};
    
    let generator_type = get_thumbnail_generator_for_file(&full_asset_path);
    
    let thumbnail_result = match generator_type {
        ThumbnailGeneratorType::Image => {
            generate_image_thumbnail(&request.project_name, &request.asset_path, size).await
        }
        ThumbnailGeneratorType::GameEngineHDR => {
            generate_hdr_environment_thumbnail(&request.project_name, &request.asset_path, size).await
        }
        ThumbnailGeneratorType::GameEngineTexture => {
            generate_game_texture_thumbnail(&request.project_name, &request.asset_path, size).await
        }
        ThumbnailGeneratorType::Model => {
            generate_model_thumbnail(&request.project_name, &request.asset_path, size).await
        }
        ThumbnailGeneratorType::Material => {
            generate_material_thumbnail(&request.project_name, &request.asset_path, size).await
        }
        ThumbnailGeneratorType::Generic => {
            // Fallback to original model generator for now
            generate_model_thumbnail(&request.project_name, &request.asset_path, size).await
        }
    };
    
    // Generate new thumbnail
    info!("🎨 Attempting to generate thumbnail using generator: {:?}", generator_type);
    match thumbnail_result {
        Ok(thumbnail_file) => {
            info!("🎉 Thumbnail generation successful: {}", thumbnail_file);
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
                    thumbnail_file: thumbnail_file.clone(),
                    generated_at: current_time,
                    file_size: metadata.len(),
                    last_modified,
                };
                
                cache.set_thumbnail(cache_key, cached_thumbnail);
                
                // Save cache to disk
                if let Err(e) = cache.save_to_file(&cache_path) {
                    error!("❌ Failed to save thumbnail cache to {:?}: {}", cache_path, e);
                } else {
                    info!("💾 Thumbnail cache saved to: {:?}", cache_path);
                }
            }
            
            ThumbnailResponse {
                success: true,
                thumbnail_file: Some(thumbnail_file),
                cached: false,
                error: None,
            }
        }
        Err(e) => {
            error!("💥 Thumbnail generation failed for {}: {}", request.asset_path, e);
            ThumbnailResponse {
                success: false,
                thumbnail_file: None,
                cached: false,
                error: Some(e.to_string()),
            }
        }
    }
}

// Batch generate thumbnails for all GLB models in a project
pub async fn batch_generate_thumbnails(project_name: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    println!("🔍 Starting batch thumbnail generation for project: {}", project_name);
    
    let projects_path = crate::get_projects_path();
    let project_path = projects_path.join(project_name);
    let models_path = project_path.join("assets").join("models");
    
    if !models_path.exists() {
        return Ok(vec![]);
    }
    
    let mut generated_thumbnails = Vec::new();
    let mut glb_files = Vec::new();
    
    // Find all GLB files recursively
    find_glb_files(&models_path, &mut glb_files)?;
    
    if glb_files.is_empty() {
        println!("📭 No GLB files found in {}", models_path.display());
        return Ok(vec![]);
    }
    
    println!("🎯 Found {} GLB files to process", glb_files.len());
    
    // Process GLB files with throttling to prevent memory issues
    for (index, glb_file) in glb_files.iter().enumerate() {
        let relative_path = glb_file
            .strip_prefix(&project_path)?
            .to_string_lossy()
            .replace('\\', "/");
            
        println!("🔄 Processing ({}/{}): {}", index + 1, glb_files.len(), relative_path);
        
        // Generate thumbnails in multiple sizes
        let sizes = [128, 256, 512];
        for &size in &sizes {
            use crate::modules::thumbnail_generators::model_types::generate_model_thumbnail;
            match generate_model_thumbnail(project_name, &relative_path, size).await {
                Ok(thumbnail_file) => {
                    generated_thumbnails.push(thumbnail_file);
                    println!("✅ Generated {}px thumbnail for {}", size, relative_path);
                }
                Err(e) => {
                    println!("❌ Failed to generate {}px thumbnail for {}: {}", size, relative_path, e);
                }
            }
            
            // Small delay between sizes to prevent memory pressure
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Delay between files to prevent memory buildup
        if index < glb_files.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }
    
    println!("🎉 Batch processing complete! Generated {} thumbnails", generated_thumbnails.len());
    Ok(generated_thumbnails)
}

fn find_glb_files(dir: &Path, glb_files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                find_glb_files(&path, glb_files)?;
            } else if let Some(extension) = path.extension() {
                if extension.to_string_lossy().to_lowercase() == "glb" {
                    glb_files.push(path);
                }
            }
        }
    }
    Ok(())
}

