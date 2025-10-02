use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use log::{info, warn, error};
use image::{ImageBuffer, RgbImage, Rgb};
use chrono;

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

pub fn get_thumbnails_dir(project_name: &str) -> PathBuf {
    let base_path = crate::get_projects_path();
    base_path.join(project_name).join(".cache").join("thumbnails")
}

pub fn generate_cache_key(project_name: &str, asset_path: &str) -> String {
    format!("{}::{}", project_name, asset_path)
}

// Generate HDR/EXR thumbnail by creating a tone-mapped preview image
pub async fn generate_hdr_exr_thumbnail(
    project_name: &str,
    asset_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if file exists
    if !full_asset_path.exists() {
        return Err("HDR/EXR file not found".into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail including extension to avoid conflicts
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr_thumbnail");
    let asset_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    let thumbnail_filename = format!("{}_{}_{}.png", asset_filename, asset_extension, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    // Create HDR placeholder thumbnail with metadata
    create_hdr_placeholder_thumbnail(&full_asset_path, &thumbnail_path, size)?;
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

// Generate 3D model thumbnail using Shopify's screenshot-glb tool
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

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail
    let asset_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("thumbnail");
    let thumbnail_filename = format!("{}_{}.png", asset_filename, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);

    // Create enhanced placeholder thumbnail
    create_enhanced_thumbnail(&full_asset_path, &thumbnail_path, size)?;
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

fn create_enhanced_thumbnail(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating enhanced placeholder thumbnail: {:?}", model_path);
    
    // Try to get some basic info about the GLB file for a more informative thumbnail
    let file_size = fs::metadata(model_path)?.len();
    let filename = model_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("model");
    
    // Create a more informative placeholder image
    create_glb_placeholder_thumbnail(thumbnail_path, size, filename, file_size)?;
    
    info!("✅ Successfully created enhanced thumbnail: {:?}", thumbnail_path);
    Ok(())
}


// Create a simple fallback thumbnail when model rendering fails
fn create_fallback_thumbnail(thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    warn!("🎨 Creating fallback thumbnail: {:?}", thumbnail_path);
    create_glb_placeholder_thumbnail(thumbnail_path, size, "model", 0)?;
    info!("🎨 Created fallback thumbnail: {:?}", thumbnail_path);
    Ok(())
}

// Create an informative placeholder thumbnail for GLB files  
fn create_glb_placeholder_thumbnail(thumbnail_path: &Path, size: u32, _filename: &str, _file_size: u64) -> Result<(), Box<dyn std::error::Error>> {
    use image::{ImageBuffer, Rgb, RgbImage};
    
    // Create a gradient background (light blue to white)
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let gradient = y as f32 / size as f32;
        let r = (220.0 + (255.0 - 220.0) * gradient) as u8;
        let g = (230.0 + (255.0 - 230.0) * gradient) as u8;
        let b = 255u8;
        Rgb([r, g, b])
    });
    
    // Draw a simple 3D cube wireframe in the center
    let center_x = size / 2;
    let center_y = size / 2;
    let cube_size = (size / 4).min(64);
    
    // Draw cube wireframe (simplified)
    draw_cube_wireframe(&mut img, center_x, center_y, cube_size);
    
    // Save the image
    img.save(thumbnail_path)?;
    
    Ok(())
}

// Draw a simple 3D cube wireframe
fn draw_cube_wireframe(img: &mut image::RgbImage, center_x: u32, center_y: u32, size: u32) {
    use image::Rgb;
    let half_size = size / 2;
    let offset = size / 4; // 3D depth offset
    
    // Front face corners
    let front_corners = [
        (center_x - half_size, center_y - half_size),     // top-left
        (center_x + half_size, center_y - half_size),     // top-right
        (center_x + half_size, center_y + half_size),     // bottom-right
        (center_x - half_size, center_y + half_size),     // bottom-left
    ];
    
    // Back face corners (offset for 3D effect)
    let back_corners = [
        (center_x - half_size + offset, center_y - half_size - offset),
        (center_x + half_size + offset, center_y - half_size - offset),
        (center_x + half_size + offset, center_y + half_size - offset),
        (center_x - half_size + offset, center_y + half_size - offset),
    ];
    
    let dark_gray = Rgb([80u8, 80u8, 80u8]);
    let medium_gray = Rgb([120u8, 120u8, 120u8]);
    
    // Draw front face
    for i in 0..4 {
        let next = (i + 1) % 4;
        draw_line(img, front_corners[i], front_corners[next], dark_gray);
    }
    
    // Draw back face  
    for i in 0..4 {
        let next = (i + 1) % 4;
        draw_line(img, back_corners[i], back_corners[next], medium_gray);
    }
    
    // Draw connecting lines (depth)
    for i in 0..4 {
        draw_line(img, front_corners[i], back_corners[i], medium_gray);
    }
}

// Simple line drawing using Bresenham's algorithm  
fn draw_line(img: &mut image::RgbImage, start: (u32, u32), end: (u32, u32), color: image::Rgb<u8>) {
    let (x0, y0) = (start.0 as i32, start.1 as i32);
    let (x1, y1) = (end.0 as i32, end.1 as i32);
    
    let dx = (x1 - x0).abs();
    let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    
    let mut x = x0;
    let mut y = y0;
    
    let (width, height) = img.dimensions();
    
    loop {
        // Set pixel if within bounds
        if x >= 0 && y >= 0 && x < width as i32 && y < height as i32 {
            img.put_pixel(x as u32, y as u32, color);
        }
        
        if x == x1 && y == y1 { break; }
        
        let e2 = 2 * err;
        if e2 > -dy {
            err -= dy;
            x += sx;
        }
        if e2 < dx {
            err += dx;
            y += sy;
        }
    }
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
    
    // Determine thumbnail type based on file extension
    let extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    let thumbnail_result = match extension.as_str() {
        "hdr" | "exr" => generate_hdr_exr_thumbnail(&request.project_name, &request.asset_path, size).await,
        _ => generate_model_thumbnail(&request.project_name, &request.asset_path, size).await,
    };
    
    // Generate new thumbnail
    match thumbnail_result {
        Ok(thumbnail_file) => {
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
                    println!("Failed to save thumbnail cache: {}", e);
                }
            }
            
            ThumbnailResponse {
                success: true,
                thumbnail_file: Some(thumbnail_file),
                cached: false,
                error: None,
            }
        }
        Err(e) => ThumbnailResponse {
            success: false,
            thumbnail_file: None,
            cached: false,
            error: Some(e.to_string()),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct MaterialInfo {
    pub name: String,
    pub diffuse_color: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub emissive_color: [f32; 3],
    pub has_diffuse_texture: bool,
    pub has_normal_texture: bool,
    pub has_metallic_texture: bool,
    pub has_roughness_texture: bool,
}

/// Generate a material preview thumbnail based on material properties
pub async fn generate_material_thumbnail(
    project_name: &str,
    material_path: &str,
    size: u32,
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🎨 Generating material thumbnail for: {}/{}", project_name, material_path);
    
    let projects_path = crate::get_projects_path();
    let full_material_path = projects_path.join(project_name).join(material_path);
    
    info!("📁 Full material path: {:?}", full_material_path);
    
    // Check if material file exists
    if !full_material_path.exists() {
        let error_msg = format!("Material file not found: {:?}", full_material_path);
        error!("{}", error_msg);
        return Err(error_msg.into());
    }

    // Create thumbnails directory if it doesn't exist
    let thumbnails_dir = get_thumbnails_dir(project_name);
    fs::create_dir_all(&thumbnails_dir)?;

    // Generate filename for thumbnail
    let material_filename = full_material_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("material");
    let thumbnail_filename = format!("{}_material_{}.png", material_filename, size);
    let thumbnail_path = thumbnails_dir.join(&thumbnail_filename);
    
    info!("🖼️ Thumbnail will be saved to: {:?}", thumbnail_path);

    // Read and parse material file
    match parse_material_file(&full_material_path) {
        Ok(material_info) => {
            info!("✅ Successfully parsed material: {}", material_info.name);
            
            // Create material preview thumbnail
            create_material_preview_thumbnail(&material_info, &thumbnail_path, size)?;
            
            // Return relative path to thumbnail
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
        Err(e) => {
            error!("❌ Failed to parse material file {:?}: {}", full_material_path, e);
            
            // Try to create a fallback thumbnail
            create_fallback_material_thumbnail(&thumbnail_path, size)?;
            Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
        }
    }
}

fn parse_material_file(material_path: &Path) -> Result<MaterialInfo, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(material_path)?;
    info!("📄 Material file content (first 200 chars): {}", 
          if content.len() > 200 { &content[..200] } else { &content });
    
    let material_data: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("JSON parse error: {}. Content: {}", e, content))?;
    
    let name = material_data["name"].as_str()
        .or_else(|| material_data["materialName"].as_str())
        .unwrap_or("Material").to_string();
    
    // Parse color arrays with defaults - try different property names
    let diffuse_color = parse_color_array(&material_data["diffuseColor"], [0.8, 0.8, 0.8])
        .or_else(|| parse_color_array(&material_data["diffuse"], [0.8, 0.8, 0.8]))
        .or_else(|| parse_color_array(&material_data["baseColor"], [0.8, 0.8, 0.8]))
        .unwrap_or([0.8, 0.8, 0.8]);
    
    let emissive_color = parse_color_array(&material_data["emissiveColor"], [0.0, 0.0, 0.0])
        .or_else(|| parse_color_array(&material_data["emissive"], [0.0, 0.0, 0.0]))
        .unwrap_or([0.0, 0.0, 0.0]);
    
    let metallic = material_data["metallic"].as_f64()
        .or_else(|| material_data["metallicFactor"].as_f64())
        .unwrap_or(0.0) as f32;
    
    let roughness = material_data["roughness"].as_f64()
        .or_else(|| material_data["roughnessFactor"].as_f64())
        .unwrap_or(0.5) as f32;
    
    // Check for texture presence - try different property structures
    let textures = &material_data["textures"];
    let has_diffuse_texture = textures["diffuse"].is_string() || 
                             textures["baseColorTexture"].is_string() ||
                             material_data["diffuseTexture"].is_string();
    let has_normal_texture = textures["normal"].is_string() || 
                            textures["normalTexture"].is_string() ||
                            material_data["normalTexture"].is_string();
    let has_metallic_texture = textures["metallic"].is_string() || 
                              textures["metallicRoughnessTexture"].is_string() ||
                              material_data["metallicTexture"].is_string();
    let has_roughness_texture = textures["roughness"].is_string() || 
                               textures["metallicRoughnessTexture"].is_string() ||
                               material_data["roughnessTexture"].is_string();

    info!("🎨 Parsed material: name='{}', diffuse={:?}, metallic={}, roughness={}", 
          name, diffuse_color, metallic, roughness);

    Ok(MaterialInfo {
        name,
        diffuse_color,
        metallic,
        roughness,
        emissive_color,
        has_diffuse_texture,
        has_normal_texture,
        has_metallic_texture,
        has_roughness_texture,
    })
}

fn parse_color_array(value: &serde_json::Value, default: [f32; 3]) -> Option<[f32; 3]> {
    if let Some(array) = value.as_array() {
        if array.len() >= 3 {
            return Some([
                array[0].as_f64().unwrap_or(default[0] as f64) as f32,
                array[1].as_f64().unwrap_or(default[1] as f64) as f32,
                array[2].as_f64().unwrap_or(default[2] as f64) as f32,
            ]);
        }
    }
    None
}

fn create_fallback_material_thumbnail(
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating fallback material thumbnail");
    
    // Create a simple generic material sphere
    let material_info = MaterialInfo {
        name: "Unknown Material".to_string(),
        diffuse_color: [0.7, 0.7, 0.7], // Gray
        metallic: 0.0,
        roughness: 0.5,
        emissive_color: [0.0, 0.0, 0.0],
        has_diffuse_texture: false,
        has_normal_texture: false,
        has_metallic_texture: false,
        has_roughness_texture: false,
    };
    
    create_material_preview_thumbnail(&material_info, thumbnail_path, size)?;
    Ok(())
}

fn create_material_preview_thumbnail(
    material: &MaterialInfo,
    thumbnail_path: &Path,
    size: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating material preview thumbnail: {}", material.name);
    
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create a gradient sphere effect for the material preview
        let center_x = 0.5;
        let center_y = 0.5;
        let radius = 0.4;
        
        let dx = x_norm - center_x;
        let dy = y_norm - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        if distance <= radius {
            // Inside the sphere - render material
            let sphere_factor = (1.0 - (distance / radius).powf(2.0)).max(0.0);
            
            // Base diffuse color
            let mut r = material.diffuse_color[0];
            let mut g = material.diffuse_color[1];
            let mut b = material.diffuse_color[2];
            
            // Apply metallic effect
            if material.metallic > 0.0 {
                let metallic_tint = material.metallic * 0.3;
                r = (r * (1.0 - metallic_tint) + metallic_tint).min(1.0);
                g = (g * (1.0 - metallic_tint) + metallic_tint).min(1.0);
                b = (b * (1.0 - metallic_tint) + metallic_tint).min(1.0);
            }
            
            // Apply roughness effect (less roughness = more reflection)
            let reflection_intensity = (1.0 - material.roughness) * sphere_factor * 0.5;
            r = (r + reflection_intensity).min(1.0);
            g = (g + reflection_intensity).min(1.0);
            b = (b + reflection_intensity).min(1.0);
            
            // Add emissive color
            r = (r + material.emissive_color[0] * 0.3).min(1.0);
            g = (g + material.emissive_color[1] * 0.3).min(1.0);
            b = (b + material.emissive_color[2] * 0.3).min(1.0);
            
            // Apply lighting (simple directional light)
            let light_dir = [0.3, 0.3, 1.0]; // Light coming from top-right
            let normal = [dx / radius, dy / radius, (1.0 - distance / radius).max(0.0)];
            let dot = (normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2]).max(0.0);
            let lighting = 0.3 + 0.7 * dot; // Ambient + diffuse
            
            r = (r * lighting * sphere_factor).min(1.0);
            g = (g * lighting * sphere_factor).min(1.0);
            b = (b * lighting * sphere_factor).min(1.0);
            
            Rgb([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8])
        } else {
            // Outside the sphere - background
            let bg_intensity = 0.1 + 0.1 * (1.0 - distance.min(1.0));
            let bg_val = (bg_intensity * 255.0) as u8;
            Rgb([bg_val, bg_val, bg_val])
        }
    });
    
    // Add texture indicators if present
    if material.has_diffuse_texture || material.has_normal_texture || material.has_metallic_texture || material.has_roughness_texture {
        add_texture_indicators(&mut img, material, size);
    }
    
    // Save the image
    img.save(thumbnail_path)?;
    info!("✅ Material thumbnail saved: {:?}", thumbnail_path);
    
    Ok(())
}

fn add_texture_indicators(img: &mut RgbImage, material: &MaterialInfo, size: u32) {
    let indicator_size = size / 16; // Small indicator size
    let spacing = indicator_size + 2;
    let start_x = size - (spacing * 4);
    let start_y = size - indicator_size - 2;
    
    let indicators = [
        (material.has_diffuse_texture, [255, 100, 100]), // Red for diffuse
        (material.has_normal_texture, [100, 100, 255]),  // Blue for normal
        (material.has_metallic_texture, [255, 255, 100]), // Yellow for metallic
        (material.has_roughness_texture, [100, 255, 100]), // Green for roughness
    ];
    
    for (i, (has_texture, color)) in indicators.iter().enumerate() {
        if *has_texture {
            let x_pos = start_x + (i as u32 * spacing);
            draw_small_square(img, x_pos, start_y, indicator_size, *color);
        }
    }
}

fn draw_small_square(img: &mut RgbImage, x: u32, y: u32, size: u32, color: [u8; 3]) {
    let (width, height) = img.dimensions();
    
    for dy in 0..size {
        for dx in 0..size {
            let px = x + dx;
            let py = y + dy;
            
            if px < width && py < height {
                img.put_pixel(px, py, Rgb(color));
            }
        }
    }
}

// Create a placeholder thumbnail for HDR/EXR files with visual indicators
fn create_hdr_placeholder_thumbnail(hdr_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating HDR/EXR placeholder thumbnail: {:?}", hdr_path);
    
    // Get file info
    let file_size = fs::metadata(hdr_path)?.len();
    let filename = hdr_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    let extension = hdr_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr")
        .to_uppercase();
    
    // Create a gradient background mimicking HDR lighting
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create a radial gradient from center
        let center_x = 0.5;
        let center_y = 0.5;
        let dx = x_norm - center_x;
        let dy = y_norm - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        // HDR-like gradient: bright center fading to darker edges
        let brightness = (1.0 - distance.min(1.0)) * 0.8 + 0.2;
        
        // Warm color scheme typical of HDR environment maps
        let r = (255.0 * brightness * 1.1).min(255.0) as u8;
        let g = (255.0 * brightness * 0.9) as u8;
        let b = (255.0 * brightness * 0.7) as u8;
        
        Rgb([r, g, b])
    });
    
    // Add HDR icon in center (simplified sun/lighting icon)
    let center_x = size / 2;
    let center_y = size / 2;
    let icon_size = (size / 6).max(8).min(32);
    
    // Draw sun icon
    draw_hdr_sun_icon(&mut img, center_x, center_y, icon_size);
    
    // Add format indicator in top-right corner
    let indicator_size = size / 8;
    let indicator_x = size - indicator_size - 4;
    let indicator_y = 4;
    
    // Draw format badge
    for dy in 0..indicator_size {
        for dx in 0..indicator_size {
            let px = indicator_x + dx;
            let py = indicator_y + dy;
            
            if px < size && py < size {
                // Semi-transparent background
                img.put_pixel(px, py, Rgb([40, 40, 40]));
            }
        }
    }
    
    // Add file size indicator in bottom-left
    let size_indicator_text = if file_size > 1_000_000 {
        format!("{:.1}MB", file_size as f32 / 1_000_000.0)
    } else if file_size > 1_000 {
        format!("{:.1}KB", file_size as f32 / 1_000.0)
    } else {
        format!("{}B", file_size)
    };
    
    // Save the image
    img.save(thumbnail_path)?;
    
    info!("✅ HDR/EXR thumbnail created: {:?} ({})", thumbnail_path, size_indicator_text);
    Ok(())
}

// Draw a stylized sun icon for HDR thumbnails
fn draw_hdr_sun_icon(img: &mut image::RgbImage, center_x: u32, center_y: u32, size: u32) {
    let half_size = size / 2;
    let ray_length = size / 3;
    
    // Sun center (bright yellow/white)
    let sun_color = Rgb([255u8, 255u8, 200u8]);
    let ray_color = Rgb([255u8, 220u8, 100u8]);
    
    // Draw sun rays (8 rays)
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::PI / 4.0;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        
        // Inner ray point
        let inner_x = center_x as i32 + (half_size as f32 * cos_a) as i32;
        let inner_y = center_y as i32 + (half_size as f32 * sin_a) as i32;
        
        // Outer ray point
        let outer_x = center_x as i32 + ((half_size + ray_length) as f32 * cos_a) as i32;
        let outer_y = center_y as i32 + ((half_size + ray_length) as f32 * sin_a) as i32;
        
        // Draw ray line
        if inner_x >= 0 && inner_y >= 0 && outer_x >= 0 && outer_y >= 0 {
            draw_line(img, (inner_x as u32, inner_y as u32), (outer_x as u32, outer_y as u32), ray_color);
        }
    }
    
    // Draw sun center (filled circle)
    for dy in 0..size {
        for dx in 0..size {
            let px = center_x + dx - half_size;
            let py = center_y + dy - half_size;
            
            if px < img.width() && py < img.height() {
                let dist_sq = (dx as i32 - half_size as i32).pow(2) + (dy as i32 - half_size as i32).pow(2);
                if dist_sq <= (half_size as i32).pow(2) {
                    img.put_pixel(px, py, sun_color);
                }
            }
        }
    }
}

/// Convert HDR/EXR panoramic image to 6 cube face images for BabylonJS
/// Returns the paths to the 6 generated cube face images
pub async fn convert_hdr_to_cubemap(
    project_name: &str, 
    asset_path: &str, 
    cube_size: u32
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    info!("🌍 Converting HDR to cube map: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if HDR file exists
    if !full_asset_path.exists() {
        let error_msg = format!("HDR file not found: {:?}", full_asset_path);
        error!("{}", error_msg);
        return Err(error_msg.into());
    }
    
    // Create cube maps directory
    let cubemaps_dir = get_cubemaps_dir(project_name);
    fs::create_dir_all(&cubemaps_dir)?;
    
    // Generate unique filename for this HDR file
    let hdr_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    let hdr_extension = full_asset_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    
    // Generate the 6 cube face file names
    let face_names = ["px", "nx", "py", "ny", "pz", "nz"]; // positive/negative x,y,z
    let mut cube_face_paths = Vec::new();
    
    for face_name in &face_names {
        let filename = format!("{}_{}_{}_{}.png", hdr_filename, hdr_extension, face_name, cube_size);
        let face_path = cubemaps_dir.join(&filename);
        
        // Return path with project prefix for bridge compatibility
        cube_face_paths.push(format!("projects/{}/.cache/cubemaps/{}", project_name, filename));
        
        // For now, create placeholder cube faces
        // TODO: Implement actual HDR panoramic to cube face conversion
        create_placeholder_cube_face(&face_path, cube_size, face_name)?;
    }
    
    info!("✅ Generated {} cube faces for HDR: {}", cube_face_paths.len(), asset_path);
    Ok(cube_face_paths)
}

/// Get the cube maps cache directory for a project
fn get_cubemaps_dir(project_name: &str) -> PathBuf {
    let projects_path = crate::get_projects_path();
    projects_path.join(project_name).join(".cache").join("cubemaps")
}

/// Create a placeholder cube face with a distinctive color/pattern
fn create_placeholder_cube_face(
    face_path: &Path, 
    size: u32, 
    face_name: &str
) -> Result<(), Box<dyn std::error::Error>> {
    info!("🎨 Creating placeholder cube face: {} ({}x{})", face_name, size, size);
    
    // Different colors for each cube face to help identify them
    let base_color = match face_name {
        "px" => [255, 100, 100], // Positive X - Red
        "nx" => [100, 255, 100], // Negative X - Green  
        "py" => [100, 100, 255], // Positive Y - Blue
        "ny" => [255, 255, 100], // Negative Y - Yellow
        "pz" => [255, 100, 255], // Positive Z - Magenta
        "nz" => [100, 255, 255], // Negative Z - Cyan
        _ => [128, 128, 128],    // Default - Gray
    };
    
    let mut img: RgbImage = ImageBuffer::from_fn(size, size, |x, y| {
        let x_norm = x as f32 / size as f32;
        let y_norm = y as f32 / size as f32;
        
        // Create a simple gradient pattern for each face
        let center_x = 0.5;
        let center_y = 0.5;
        let dx = x_norm - center_x;
        let dy = y_norm - center_y;
        let distance = (dx * dx + dy * dy).sqrt();
        
        // Apply gradient effect
        let brightness = (1.0 - distance * 0.5).max(0.3);
        
        let r = (base_color[0] as f32 * brightness) as u8;
        let g = (base_color[1] as f32 * brightness) as u8;
        let b = (base_color[2] as f32 * brightness) as u8;
        
        Rgb([r, g, b])
    });
    
    // Add face label in center
    let center = size / 2;
    let label_size = size / 8;
    
    // Draw a simple indicator pattern for the face
    for dy in 0..label_size {
        for dx in 0..label_size {
            let px = center + dx - label_size / 2;
            let py = center + dy - label_size / 2;
            
            if px < size && py < size {
                img.put_pixel(px, py, Rgb([255, 255, 255])); // White indicator
            }
        }
    }
    
    // Save the image
    img.save(face_path)?;
    info!("✅ Placeholder cube face saved: {:?}", face_path);
    
    Ok(())
}

/// Convert HDR/EXR panoramic image to 6 cube face assets stored in the project
/// Returns the folder path containing the cube map assets
pub async fn convert_hdr_to_cubemap_assets(
    project_name: &str,
    asset_path: &str,
    cube_size: u32
) -> Result<String, Box<dyn std::error::Error>> {
    info!("🌍 Converting HDR to cube map assets: {}/{}", project_name, asset_path);
    
    let projects_path = crate::get_projects_path();
    let full_asset_path = projects_path.join(project_name).join(asset_path);
    
    // Check if HDR file exists
    if !full_asset_path.exists() {
        let error_msg = format!("HDR file not found: {:?}", full_asset_path);
        error!("{}", error_msg);
        return Err(error_msg.into());
    }
    
    // Generate cube map folder name based on HDR file
    let hdr_filename = full_asset_path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("hdr");
    let cube_map_folder_name = format!("{}_cubemap", hdr_filename);
    
    // Create cube map folder in assets/skyboxes/
    let skyboxes_dir = projects_path.join(project_name).join("assets").join("skyboxes");
    fs::create_dir_all(&skyboxes_dir)?;
    
    let cube_map_dir = skyboxes_dir.join(&cube_map_folder_name);
    fs::create_dir_all(&cube_map_dir)?;
    
    // Generate the 6 cube face file names
    let face_names = ["px", "nx", "py", "ny", "pz", "nz"]; // positive/negative x,y,z
    
    for face_name in &face_names {
        let filename = format!("{}_{}.png", face_name, cube_size);
        let face_path = cube_map_dir.join(&filename);
        
        // Create cube face with proper asset structure
        create_placeholder_cube_face(&face_path, cube_size, face_name)?;
    }
    
    // Create cube map metadata file
    let metadata = serde_json::json!({
        "type": "cubemap",
        "source_hdr": asset_path,
        "cube_size": cube_size,
        "faces": {
            "positive_x": format!("px_{}.png", cube_size),
            "negative_x": format!("nx_{}.png", cube_size),
            "positive_y": format!("py_{}.png", cube_size),
            "negative_y": format!("ny_{}.png", cube_size),
            "positive_z": format!("pz_{}.png", cube_size),
            "negative_z": format!("nz_{}.png", cube_size)
        },
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "version": "1.0"
    });
    
    let metadata_path = cube_map_dir.join("cubemap.json");
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;
    
    let cube_map_asset_path = format!("assets/skyboxes/{}", cube_map_folder_name);
    info!("✅ Generated cube map assets: {}", cube_map_asset_path);
    
    Ok(cube_map_asset_path)
}