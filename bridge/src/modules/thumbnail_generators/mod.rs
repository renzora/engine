pub mod image_types;
pub mod model_types;
pub mod game_engine_types;
pub mod real_glb_renderer;

use std::path::Path;
use serde::{Deserialize, Serialize};

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

/// Determine which thumbnail generator to use based on file extension
pub fn get_thumbnail_generator_for_file(file_path: &Path) -> ThumbnailGeneratorType {
    let extension = file_path.extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match extension.as_str() {
        // Regular image formats
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tiff" | "tif" | "webp" => {
            ThumbnailGeneratorType::Image
        }
        
        // Game engine HDR/environment formats
        "hdr" | "exr" | "pfm" => ThumbnailGeneratorType::GameEngineHDR,
        
        // Game engine texture formats
        "dds" | "ktx" | "ktx2" | "astc" | "pkm" | "pvr" | "etc1" | "etc2" => {
            ThumbnailGeneratorType::GameEngineTexture
        }
        
        // 3D model formats
        "glb" | "gltf" | "obj" | "fbx" | "dae" | "3ds" | "blend" | "max" | "ma" | "mb" => {
            ThumbnailGeneratorType::Model
        }
        
        // Material files
        "mat" | "material" | "json" => {
            // Could be material or other JSON - we'll let the caller decide
            ThumbnailGeneratorType::Material
        }
        
        // Default to generic
        _ => ThumbnailGeneratorType::Generic
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ThumbnailGeneratorType {
    Image,
    GameEngineHDR,
    GameEngineTexture,
    Model,
    Material,
    Generic,
}