//! Rendering-related component data types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for mesh nodes - stores the mesh type so it can be serialized
#[derive(Component, Clone, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MeshNodeData {
    pub mesh_type: MeshPrimitiveType,
}

/// Types of mesh primitives supported
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum MeshPrimitiveType {
    #[default]
    Cube,
    Sphere,
    Cylinder,
    Plane,
}

#[allow(dead_code)]
impl MeshPrimitiveType {
    /// Get the type_id string for this mesh type
    pub fn type_id(&self) -> &'static str {
        match self {
            MeshPrimitiveType::Cube => "mesh.cube",
            MeshPrimitiveType::Sphere => "mesh.sphere",
            MeshPrimitiveType::Cylinder => "mesh.cylinder",
            MeshPrimitiveType::Plane => "mesh.plane",
        }
    }

    /// Convert from type_id string
    pub fn from_type_id(type_id: &str) -> Option<Self> {
        match type_id {
            "mesh.cube" => Some(MeshPrimitiveType::Cube),
            "mesh.sphere" => Some(MeshPrimitiveType::Sphere),
            "mesh.cylinder" => Some(MeshPrimitiveType::Cylinder),
            "mesh.plane" => Some(MeshPrimitiveType::Plane),
            _ => None,
        }
    }
}

/// Data component for 2D sprite nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Sprite2DData {
    /// Path to the texture file (relative to assets folder)
    pub texture_path: String,
    /// Sprite color/tint (RGBA)
    pub color: Vec4,
    /// Whether to flip the sprite horizontally
    pub flip_x: bool,
    /// Whether to flip the sprite vertically
    pub flip_y: bool,
    /// Anchor point (0.5, 0.5 = center)
    pub anchor: Vec2,
}

impl Default for Sprite2DData {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            color: Vec4::ONE,
            flip_x: false,
            flip_y: false,
            anchor: Vec2::new(0.5, 0.5),
        }
    }
}

/// A single sprite sheet animation definition
#[derive(Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct SpriteAnimation {
    /// Name of the animation (e.g., "idle", "walk", "attack")
    pub name: String,
    /// First frame index in the sprite sheet
    pub first_frame: usize,
    /// Last frame index in the sprite sheet
    pub last_frame: usize,
    /// Duration of each frame in seconds
    pub frame_duration: f32,
    /// Whether the animation loops
    pub looping: bool,
}

impl Default for SpriteAnimation {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            first_frame: 0,
            last_frame: 0,
            frame_duration: 0.1,
            looping: true,
        }
    }
}

/// Data component for sprite sheet animations
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SpriteSheetData {
    /// Number of columns in the sprite sheet
    pub columns: usize,
    /// Number of rows in the sprite sheet
    pub rows: usize,
    /// Total number of frames (sprites) in the sheet
    pub frame_count: usize,
    /// Tile/frame width in pixels
    pub tile_width: f32,
    /// Tile/frame height in pixels
    pub tile_height: f32,
    /// Named animations
    pub animations: Vec<SpriteAnimation>,
    /// Default animation to play on start (if any)
    pub default_animation: Option<String>,
}

impl Default for SpriteSheetData {
    fn default() -> Self {
        Self {
            columns: 1,
            rows: 1,
            frame_count: 1,
            tile_width: 32.0,
            tile_height: 32.0,
            animations: Vec::new(),
            default_animation: None,
        }
    }
}

impl SpriteSheetData {
    /// Find an animation by name
    pub fn get_animation(&self, name: &str) -> Option<&SpriteAnimation> {
        self.animations.iter().find(|a| a.name == name)
    }
}

// =============================================================================
// Material Data Components
// =============================================================================

/// Data component for material blueprints - stores the path to a .material_bp file
/// When attached to an entity with a mesh, the material will be compiled and applied
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize, Default)]
#[reflect(Component)]
pub struct MaterialData {
    /// Path to the material blueprint file (.material_bp)
    /// Can be relative to project or absolute
    pub material_path: Option<String>,
}

// =============================================================================
// Light Data Components
// =============================================================================

/// Data component for point lights - serializable version of Bevy's PointLight
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct PointLightData {
    /// Light color (RGB, 0-1 range)
    pub color: Vec3,
    /// Light intensity in lumens
    pub intensity: f32,
    /// Maximum distance the light affects
    pub range: f32,
    /// Light radius for soft shadows
    pub radius: f32,
    /// Whether this light casts shadows
    pub shadows_enabled: bool,
}

impl Default for PointLightData {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1000.0,
            range: 20.0,
            radius: 0.0,
            shadows_enabled: true,
        }
    }
}

/// Data component for directional lights - serializable version of Bevy's DirectionalLight
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct DirectionalLightData {
    /// Light color (RGB, 0-1 range)
    pub color: Vec3,
    /// Illuminance in lux
    pub illuminance: f32,
    /// Whether this light casts shadows
    pub shadows_enabled: bool,
}

impl Default for DirectionalLightData {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            illuminance: 10000.0,
            shadows_enabled: true,
        }
    }
}

/// Data component for spot lights - serializable version of Bevy's SpotLight
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SpotLightData {
    /// Light color (RGB, 0-1 range)
    pub color: Vec3,
    /// Light intensity in lumens
    pub intensity: f32,
    /// Maximum distance the light affects
    pub range: f32,
    /// Light radius for soft shadows
    pub radius: f32,
    /// Inner cone angle in radians (full intensity)
    pub inner_angle: f32,
    /// Outer cone angle in radians (light falloff)
    pub outer_angle: f32,
    /// Whether this light casts shadows
    pub shadows_enabled: bool,
}

impl Default for SpotLightData {
    fn default() -> Self {
        Self {
            color: Vec3::ONE,
            intensity: 1000.0,
            range: 20.0,
            radius: 0.0,
            inner_angle: 0.3,
            outer_angle: 0.5,
            shadows_enabled: true,
        }
    }
}

// =============================================================================
// Sun Data
// =============================================================================

/// Data component for a sun light — a directional light positioned by azimuth/elevation angles.
/// Automatically orients a DirectionalLight based on the angular position.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SunData {
    /// Azimuth angle in degrees (0-360, compass direction of the sun)
    pub azimuth: f32,
    /// Elevation angle in degrees (-90 to 90, height above horizon)
    pub elevation: f32,
    /// Light color (RGB, 0-1 range)
    pub color: Vec3,
    /// Illuminance in lux
    pub illuminance: f32,
    /// Whether this light casts shadows
    pub shadows_enabled: bool,
    /// Angular diameter of the sun disk in degrees (default ~0.53 for Earth's sun)
    pub angular_diameter: f32,
}

impl Default for SunData {
    fn default() -> Self {
        Self {
            azimuth: 0.0,
            elevation: 50.0,
            color: Vec3::new(1.0, 0.96, 0.90),
            illuminance: 100_000.0,
            shadows_enabled: true,
            angular_diameter: 0.53,
        }
    }
}

impl SunData {
    /// Compute the sun's direction vector from azimuth and elevation.
    /// Returns the direction the light travels (pointing away from the sun toward the scene).
    pub fn direction(&self) -> Vec3 {
        let az = self.azimuth.to_radians();
        let el = self.elevation.to_radians();
        // Direction FROM the sun (light travels this way)
        Vec3::new(
            -el.cos() * az.sin(),
            -el.sin(),
            -el.cos() * az.cos(),
        )
    }
}

// =============================================================================
// Solari Raytracing Settings
// =============================================================================

/// DLSS quality/performance mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Serialize, Deserialize, Default)]
pub enum DlssQualityMode {
    /// Let DLSS choose the best mode
    #[default]
    Auto,
    /// DLAA - Deep Learning Anti-Aliasing (native resolution)
    Dlaa,
    /// Highest quality upscaling
    Quality,
    /// Balanced quality/performance
    Balanced,
    /// Higher performance, lower quality
    Performance,
    /// Maximum performance (lowest quality)
    UltraPerformance,
}

/// Data component for Solari raytraced lighting settings
/// Add this to an entity to enable raytracing in the scene
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct SolariLightingData {
    /// Enable Solari raytraced lighting
    pub enabled: bool,
    /// Enable DLSS Ray Reconstruction for denoising (NVIDIA GPUs only)
    pub dlss_enabled: bool,
    /// DLSS quality mode
    pub dlss_quality: DlssQualityMode,
}

impl Default for SolariLightingData {
    fn default() -> Self {
        Self {
            enabled: true, // Enabled when component is added
            dlss_enabled: false,
            dlss_quality: DlssQualityMode::Auto,
        }
    }
}

// =============================================================================
// Meshlet/Virtual Geometry Settings
// =============================================================================

/// Data component for meshlet (virtual geometry) mesh rendering
/// Entities with this component use GPU-driven meshlet rendering instead of standard mesh rendering.
/// This is beneficial for high-polygon meshes as it provides:
/// - Automatic LOD through meshlet clustering
/// - Visibility buffer rendering
/// - Occlusion culling
///
/// Note: Only supports opaque, non-deforming meshes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MeshletMeshData {
    /// Path to the preprocessed .meshlet asset file
    pub meshlet_path: String,
    /// Whether meshlet rendering is enabled for this entity
    pub enabled: bool,
    /// LOD bias for this mesh (-1.0 = prefer higher LOD, 1.0 = prefer lower LOD)
    pub lod_bias: f32,
}

impl Default for MeshletMeshData {
    fn default() -> Self {
        Self {
            meshlet_path: String::new(),
            enabled: true,
            lod_bias: 0.0,
        }
    }
}
