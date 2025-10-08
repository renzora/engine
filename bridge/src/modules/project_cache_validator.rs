use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use sha2::{Sha256, Digest};
use log::{info, debug};
use crate::modules::memory_cache::{MemoryCache, ProjectManifest, FileMetadata, CacheValidationResult, ChangeSummary};

const CACHE_VERSION: &str = "1.0";

pub struct ProjectCacheValidator {
    project_name: String,
    project_path: PathBuf,
    memory_cache: Option<std::sync::Arc<tokio::sync::Mutex<MemoryCache>>>,
}

#[derive(Debug)]
pub struct ProcessingPlan {
    pub new_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub deleted_files: Vec<String>,
    pub moved_files: Vec<(String, PathBuf)>,
}

impl ProcessingPlan {
    pub fn new() -> Self {
        Self {
            new_files: Vec::new(),
            modified_files: Vec::new(),
            deleted_files: Vec::new(),
            moved_files: Vec::new(),
        }
    }

    pub fn add_new_file(&mut self, path: PathBuf) {
        self.new_files.push(path);
    }

    pub fn add_modified_file(&mut self, path: PathBuf) {
        self.modified_files.push(path);
    }

    pub fn add_deleted_file(&mut self, path: String) {
        self.deleted_files.push(path);
    }

    pub fn add_moved_file(&mut self, old_path: String, new_path: PathBuf) {
        self.moved_files.push((old_path, new_path));
    }

    pub fn total_changes(&self) -> usize {
        self.new_files.len() + self.modified_files.len() + self.deleted_files.len() + self.moved_files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.total_changes() == 0
    }

    pub fn estimate_processing_time(&self) -> u64 {
        // Rough time estimates in seconds
        let _image_time = 2;  // seconds per image
        let _model_time = 10; // seconds per model
        let _audio_time = 3;  // seconds per audio file
        let _other_time = 1;  // seconds per other file

        let mut total_time = 0;
        
        for file in &self.new_files {
            total_time += estimate_file_processing_time(file);
        }
        
        for file in &self.modified_files {
            total_time += estimate_file_processing_time(file);
        }

        // Add some overhead for cache operations
        total_time + (self.total_changes() as u64 / 10)
    }
}

fn estimate_file_processing_time(file_path: &Path) -> u64 {
    if let Some(extension) = file_path.extension() {
        let ext = extension.to_string_lossy().to_lowercase();
        match ext.as_str() {
            // Images and textures
            "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tga" | "tiff" | "hdr" | "exr" => 2,
            // 3D Models
            "glb" | "gltf" | "obj" | "fbx" | "dae" | "3ds" | "blend" | "stl" => 10,
            // Audio
            "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => 3,
            // Video
            "mp4" | "avi" | "mov" | "mkv" | "webm" => 8,
            // Scripts and text
            "js" | "ts" | "json" | "txt" | "md" | "ren" => 1,
            // Other
            _ => 1,
        }
    } else {
        1
    }
}

impl ProjectCacheValidator {
    pub fn new(project_name: String, memory_cache: Option<std::sync::Arc<tokio::sync::Mutex<MemoryCache>>>) -> Self {
        let projects_path = crate::get_projects_path();
        let project_path = projects_path.join(&project_name);
        
        Self {
            project_name,
            project_path,
            memory_cache,
        }
    }

    pub async fn validate_cache(&self) -> Result<CacheValidationResult, Box<dyn std::error::Error + Send + Sync>> {
        info!("🔍 Validating cache for project: {}", self.project_name);
        
        // Get cached manifest
        let cached_manifest = if let Some(redis) = &self.memory_cache {
            let redis_guard = redis.lock().await;
            redis_guard.get_project_manifest(&self.project_name).await
        } else {
            None
        };

        if cached_manifest.is_some() {
            info!("📋 Found cached manifest for project: {}", self.project_name);
        } else {
            info!("⚠️ No cached manifest found for project: {}", self.project_name);
        }

        // Scan current project files
        let current_files = self.scan_project_files()?;
        let current_file_count = current_files.len();
        let current_checksum = self.calculate_project_checksum(&current_files)?;

        // Quick validation first
        if let Some(manifest) = &cached_manifest {
            info!("🔍 Comparing: cached files={}, current files={}", manifest.file_count, current_file_count);
            info!("🔍 Comparing: cached checksum={}, current checksum={}", &manifest.checksum[..8], &current_checksum[..8]);
            
            if manifest.file_count == current_file_count && manifest.checksum == current_checksum {
                info!("✅ Cache is valid for project: {}", self.project_name);
                return Ok(CacheValidationResult {
                    cache_status: "valid".to_string(),
                    changes_detected: 0,
                    estimated_processing_time: 0,
                    change_summary: ChangeSummary {
                        new_files: 0,
                        modified_files: 0,
                        deleted_files: 0,
                        moved_files: 0,
                    },
                });
            }
        }

        // If we get here, we need a detailed comparison
        let processing_plan = self.create_processing_plan(&current_files).await?;

        let cache_status = if cached_manifest.is_none() {
            "missing"
        } else if processing_plan.total_changes() > current_file_count / 2 {
            "needs_full_rebuild"
        } else {
            "needs_update"
        };

        info!("🔄 Cache status: {} ({} changes detected)", cache_status, processing_plan.total_changes());

        Ok(CacheValidationResult {
            cache_status: cache_status.to_string(),
            changes_detected: processing_plan.total_changes(),
            estimated_processing_time: processing_plan.estimate_processing_time(),
            change_summary: ChangeSummary {
                new_files: processing_plan.new_files.len(),
                modified_files: processing_plan.modified_files.len(),
                deleted_files: processing_plan.deleted_files.len(),
                moved_files: processing_plan.moved_files.len(),
            },
        })
    }

    pub async fn create_processing_plan(&self, current_files: &[PathBuf]) -> Result<ProcessingPlan, Box<dyn std::error::Error + Send + Sync>> {
        let mut plan = ProcessingPlan::new();

        // Get cached file metadata
        let cached_files = if let Some(redis) = &self.memory_cache {
            let redis_guard = redis.lock().await;
            redis_guard.get_all_file_metadata(&self.project_name).await
        } else {
            Vec::new()
        };

        // Create lookup map for cached files
        let mut cached_lookup: HashMap<String, FileMetadata> = HashMap::new();
        for metadata in cached_files {
            cached_lookup.insert(metadata.path.clone(), metadata);
        }

        // Check current files
        for file_path in current_files {
            let relative_path = self.get_relative_path(file_path)?;
            
            if let Some(cached_metadata) = cached_lookup.get(&relative_path) {
                // File exists in cache, check if it needs updating
                if self.file_needs_reprocessing(file_path, cached_metadata)? {
                    plan.add_modified_file(file_path.clone());
                }
                // Remove from cached_lookup to track remaining files
                cached_lookup.remove(&relative_path);
            } else {
                // New file not in cache
                plan.add_new_file(file_path.clone());
            }
        }

        // Remaining files in cached_lookup are deleted
        for (path, _) in cached_lookup {
            plan.add_deleted_file(path);
        }

        debug!("📊 Processing plan: {} new, {} modified, {} deleted files", 
               plan.new_files.len(), plan.modified_files.len(), plan.deleted_files.len());

        Ok(plan)
    }

    pub fn scan_project_files(&self) -> Result<Vec<PathBuf>, Box<dyn std::error::Error + Send + Sync>> {
        let mut files = Vec::new();
        self.scan_directory_recursive(&self.project_path, &mut files)?;
        
        // Filter out cache directories, system files, and configuration files, but KEEP .cache/thumbnails
        files.retain(|path| {
            let path_str = path.to_string_lossy();
            let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let is_thumbnail_cache = path_str.contains(".cache/thumbnails");
            let is_other_cache = path_str.contains(".cache") && !is_thumbnail_cache;
            
            // Exclude system and cache files (except thumbnail cache)
            if is_other_cache || 
               path_str.contains(".git") || 
               path_str.starts_with('.') ||
               !path.is_file() {
                return false;
            }
            
            // Exclude configuration files that don't affect asset processing
            if file_name == "project.json" ||
               file_name == "package.json" ||
               file_name == "tsconfig.json" ||
               file_name == "webpack.config.js" ||
               ((path_str.contains("scenes/") || path_str.contains("scenes\\")) && file_name.ends_with(".json")) {
                return false;
            }
            
            true
        });

        debug!("📁 Scanned {} files in project: {}", files.len(), self.project_name);
        Ok(files)
    }

    fn scan_directory_recursive(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if dir.is_dir() {
            for entry in fs::read_dir(dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    // Skip hidden and cache directories
                    if let Some(dir_name) = path.file_name() {
                        let dir_str = dir_name.to_string_lossy();
                        if !dir_str.starts_with('.') {
                            self.scan_directory_recursive(&path, files)?;
                        }
                    }
                } else {
                    files.push(path);
                }
            }
        }
        Ok(())
    }

    fn calculate_project_checksum(&self, files: &[PathBuf]) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        info!("🔍 Starting checksum calculation for {} files...", files.len());
        let mut hasher = Sha256::new();
        
        // Sort files for consistent hashing
        info!("📋 Sorting files for checksum calculation...");
        let mut sorted_files = files.to_vec();
        sorted_files.sort();
        info!("📋 File sorting complete");
        
        info!("📋 Reading file metadata for checksum...");
        for (index, file_path) in sorted_files.iter().enumerate() {
            if index % 10 == 0 {
                info!("📋 Processing file {}/{} for checksum", index + 1, sorted_files.len());
            }
            
            // Add file path to hash
            hasher.update(file_path.to_string_lossy().as_bytes());
            
            // Add file metadata to hash
            if let Ok(metadata) = fs::metadata(&file_path) {
                if let Ok(modified) = metadata.modified() {
                    if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                        hasher.update(duration.as_secs().to_be_bytes());
                    }
                }
                hasher.update(metadata.len().to_be_bytes());
            }
        }
        
        info!("📋 Finalizing checksum calculation...");
        let checksum = format!("{:x}", hasher.finalize());
        info!("📋 Checksum calculation complete: {}", &checksum[..16]);
        Ok(checksum)
    }

    fn file_needs_reprocessing(&self, file_path: &Path, cached_metadata: &FileMetadata) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
        let metadata = fs::metadata(file_path)?;
        
        // Check file size
        if metadata.len() != cached_metadata.file_size {
            return Ok(true);
        }
        
        // Check modification time
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                let current_modified = duration.as_secs();
                if current_modified != cached_metadata.last_modified {
                    return Ok(true);
                }
            }
        }
        
        // Check cache version
        if cached_metadata.processed_at == 0 {
            return Ok(true);
        }
        
        Ok(false)
    }

    pub fn get_relative_path(&self, file_path: &Path) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let relative = file_path.strip_prefix(&self.project_path)?;
        Ok(relative.to_string_lossy().replace('\\', "/"))
    }

    pub async fn update_project_manifest(&self, current_files: &[PathBuf]) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(redis) = &self.memory_cache {
            let checksum = self.calculate_project_checksum(current_files)?;
            let current_time = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
            
            let manifest = ProjectManifest {
                project_name: self.project_name.clone(),
                last_scan: current_time,
                file_count: current_files.len(),
                checksum,
                cache_version: CACHE_VERSION.to_string(),
            };
            
            let redis_guard = redis.lock().await;
            if redis_guard.cache_project_manifest(&manifest).await {
                info!("📋 Cached project manifest to Redis: {} ({} files, checksum: {})", 
                      self.project_name, manifest.file_count, &manifest.checksum[..8]);
            } else {
                info!("💾 Updated project manifest for: {}", self.project_name);
            }
        }
        
        Ok(())
    }

    pub fn get_file_type(&self, file_path: &Path) -> String {
        if let Some(extension) = file_path.extension() {
            let ext = extension.to_string_lossy().to_lowercase();
            match ext.as_str() {
                "jpg" | "jpeg" | "png" | "webp" | "bmp" | "tga" | "tiff" | "ico" | "svg" => "image",
                "hdr" | "exr" => "hdr_image",
                "glb" | "gltf" | "obj" | "fbx" | "dae" | "3ds" | "blend" | "stl" | "ply" => "model",
                "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => "audio",
                "mp4" | "avi" | "mov" | "mkv" | "webm" | "wmv" => "video",
                "js" | "ts" | "jsx" | "tsx" => "script",
                "json" | "xml" | "yaml" | "yml" => "data",
                "txt" | "md" | "rst" => "document",
                "ren" => "renscript",
                _ => "other",
            }
        } else {
            "other"
        }.to_string()
    }
}