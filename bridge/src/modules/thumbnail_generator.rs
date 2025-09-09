use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use std::ffi::OsStr;
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};
use headless_chrome::{Browser, LaunchOptions};

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

// Generate 3D model thumbnail using headless Chrome and model-viewer (like screenshot-glb)
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

    // Use headless Chrome with model-viewer for rendering
    render_glb_with_chrome(&full_asset_path, &thumbnail_path, size).await?;
    
    // Return relative path to thumbnail
    Ok(format!(".cache/thumbnails/{}", thumbnail_filename))
}

async fn render_glb_with_chrome(model_path: &Path, thumbnail_path: &Path, size: u32) -> Result<(), Box<dyn std::error::Error>> {
    println!("🎯 Rendering GLB with headless Chrome: {:?}", model_path);
    
    // Launch headless Chrome with proper scaling and memory limits
    let browser = Browser::new(LaunchOptions {
        headless: true,
        sandbox: false,
        window_size: Some((size, size)),
        args: vec![
            OsStr::new(&format!("--window-size={},{}", size, size)),
            OsStr::new("--disable-web-security"),
            OsStr::new("--disable-dev-shm-usage"),
            OsStr::new("--no-first-run"),
            OsStr::new("--hide-scrollbars"),
            OsStr::new("--force-device-scale-factor=1"),
            OsStr::new("--max-old-space-size=2048"), // Limit Node.js memory to 2GB
            OsStr::new("--memory-pressure-off"), // Disable memory pressure handling
            OsStr::new("--disable-gpu-sandbox"),
            OsStr::new("--disable-software-rasterizer"),
            OsStr::new("--disable-background-timer-throttling"),
            OsStr::new("--disable-backgrounding-occluded-windows"),
            OsStr::new("--disable-renderer-backgrounding"),
            OsStr::new("--no-sandbox"),
            OsStr::new("--disable-extensions"),
            OsStr::new("--disable-plugins"),
            OsStr::new("--virtual-time-budget=5000"), // 5 second budget for rendering
        ],
        ..LaunchOptions::default()
    })?;
    
    let tab = browser.new_tab()?;
    
    // Instead of loading the entire file into memory, use file:// URL
    let model_url = format!("file:///{}", model_path.to_string_lossy().replace('\\', "/"));
    
    // Prepare temp HTML file path
    let temp_dir = std::env::temp_dir();
    let temp_html_path = temp_dir.join(format!("glb_viewer_{}.html", std::process::id()));
    
    // Create HTML page with model-viewer
    let html_content = format!(r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width={}, height={}, initial-scale=1">
    <title>GLB Screenshot</title>
    <script type="module" src="https://unpkg.com/@google/model-viewer@3.3.0/dist/model-viewer.min.js"></script>
    <style>
        * {{
            margin: 0;
            padding: 0;
            box-sizing: border-box;
        }}
        html, body {{
            width: {}px;
            height: {}px;
            overflow: hidden;
            background: transparent;
            background-color: rgba(240, 240, 240, 1);
        }}
        model-viewer {{
            width: {}px;
            height: {}px;
            background-color: rgba(240, 240, 240, 1);
            display: block;
            position: absolute;
            top: 0;
            left: 0;
        }}
    </style>
</head>
<body>
    <model-viewer 
        src="{}" 
        auto-rotate="false"
        camera-controls="false"
        exposure="1.0" 
        tone-mapping="neutral"
        environment-image="neutral"
        shadow-intensity="0.5"
        loading="eager"
        camera-orbit="45deg 75deg auto"
        field-of-view="30deg"
        min-camera-orbit="auto auto auto"
        max-camera-orbit="auto auto auto">
    </model-viewer>
    <script>
        window.modelReady = false;
        const modelViewer = document.querySelector('model-viewer');
        
        modelViewer.addEventListener('load', () => {{
            console.log('Model loaded successfully');
            // Auto-frame the model to fit in view
            modelViewer.cameraOrbit = 'auto auto auto';
            setTimeout(() => {{
                window.modelReady = true;
            }}, 200);
        }});
        
        modelViewer.addEventListener('error', (e) => {{
            console.error('Model loading error:', e);
            window.modelError = true;
        }});
        
        // Ensure model is properly framed after loading
        modelViewer.addEventListener('camera-change', () => {{
            if (window.modelReady) return;
            // Force reframe on first camera change
            modelViewer.cameraTarget = 'auto auto auto';
        }});
    </script>
</body>
</html>
    "#, size, size, size, size, size, size, model_url);
    
    // Create a temporary HTML file instead of using data URL to avoid memory issues
    fs::write(&temp_html_path, html_content.as_bytes())?;
    
    // Navigate to the temporary HTML file
    let file_url = format!("file:///{}", temp_html_path.to_string_lossy().replace('\\', "/"));
    tab.navigate_to(&file_url)?;
    
    // Wait for model to load
    tab.wait_for_element_with_custom_timeout("model-viewer", std::time::Duration::from_secs(30))?;
    
    // Wait for model to be ready
    let mut retries = 0;
    const MAX_RETRIES: u32 = 50;
    while retries < MAX_RETRIES {
        let ready_result = tab.evaluate("window.modelReady", false);
        let error_result = tab.evaluate("window.modelError", false);
        
        if let Ok(error_obj) = error_result {
            if let Some(error_val) = error_obj.value.as_ref().and_then(|v| v.as_bool()) {
                if error_val {
                    return Err("Model loading failed in browser".into());
                }
            }
        }
        
        if let Ok(ready_obj) = ready_result {
            if let Some(ready_val) = ready_obj.value.as_ref().and_then(|v| v.as_bool()) {
                if ready_val {
                    break;
                }
            }
        }
        
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        retries += 1;
    }
    
    if retries >= MAX_RETRIES {
        return Err("Timeout waiting for model to load".into());
    }
    
    // Additional delay to ensure model is fully rendered and framed
    tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
    
    // Set viewport size precisely and ensure proper bounds
    tab.set_bounds(headless_chrome::types::Bounds::Normal {
        left: Some(0),
        top: Some(0), 
        width: Some(size as f64),
        height: Some(size as f64),
    })?;
    
    // Set the viewport size to ensure consistent rendering
    // Note: set_viewport method may not be available in this version of headless_chrome
    // tab.set_viewport(headless_chrome::protocol::cdp::Page::Viewport {
    //     x: 0.0,
    //     y: 0.0,
    //     width: size as f64,
    //     height: size as f64,
    //     scale: 1.0,
    // })?;
    
    // Force a repaint to ensure everything is rendered
    tab.evaluate("document.body.style.visibility = 'visible'", false)?;
    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    
    // Take screenshot with exact dimensions and full page capture
    let screenshot_data = tab.capture_screenshot(
        headless_chrome::protocol::cdp::Page::CaptureScreenshotFormatOption::Png,
        Some(100), // High quality
        Some(headless_chrome::protocol::cdp::Page::Viewport {
            x: 0.0,
            y: 0.0,
            width: size as f64,
            height: size as f64,
            scale: 1.0,
        }),
        true, // From surface (captures full rendered content)
    )?;
    
    // Save PNG file to disk
    fs::write(thumbnail_path, &screenshot_data)?;
    println!("📸 Saved thumbnail: {:?}", thumbnail_path);
    
    // Clean up temporary HTML file
    if temp_html_path.exists() {
        let _ = fs::remove_file(&temp_html_path); // Ignore errors on cleanup
    }
    
    Ok(())
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
    
    // Generate new thumbnail
    match generate_model_thumbnail(&request.project_name, &request.asset_path, size).await {
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