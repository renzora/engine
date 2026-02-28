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
    Cone,
    Torus,
    Capsule,
    Wedge,
    Stairs,
    Arch,
    HalfCylinder,
    QuarterPipe,
    Corner,
    Prism,
    Pyramid,
    Pipe,
    Ring,
    Wall,
    Ramp,
    Hemisphere,
    CurvedWall,
    Doorway,
    WindowWall,
    LShape,
    TShape,
    CrossShape,
    Funnel,
    Gutter,
    SpiralStairs,
    Pillar,
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
            MeshPrimitiveType::Cone => "mesh.cone",
            MeshPrimitiveType::Torus => "mesh.torus",
            MeshPrimitiveType::Capsule => "mesh.capsule",
            MeshPrimitiveType::Wedge => "mesh.wedge",
            MeshPrimitiveType::Stairs => "mesh.stairs",
            MeshPrimitiveType::Arch => "mesh.arch",
            MeshPrimitiveType::HalfCylinder => "mesh.half_cylinder",
            MeshPrimitiveType::QuarterPipe => "mesh.quarter_pipe",
            MeshPrimitiveType::Corner => "mesh.corner",
            MeshPrimitiveType::Prism => "mesh.prism",
            MeshPrimitiveType::Pyramid => "mesh.pyramid",
            MeshPrimitiveType::Pipe => "mesh.pipe",
            MeshPrimitiveType::Ring => "mesh.ring",
            MeshPrimitiveType::Wall => "mesh.wall",
            MeshPrimitiveType::Ramp => "mesh.ramp",
            MeshPrimitiveType::Hemisphere => "mesh.hemisphere",
            MeshPrimitiveType::CurvedWall => "mesh.curved_wall",
            MeshPrimitiveType::Doorway => "mesh.doorway",
            MeshPrimitiveType::WindowWall => "mesh.window_wall",
            MeshPrimitiveType::LShape => "mesh.l_shape",
            MeshPrimitiveType::TShape => "mesh.t_shape",
            MeshPrimitiveType::CrossShape => "mesh.cross_shape",
            MeshPrimitiveType::Funnel => "mesh.funnel",
            MeshPrimitiveType::Gutter => "mesh.gutter",
            MeshPrimitiveType::SpiralStairs => "mesh.spiral_stairs",
            MeshPrimitiveType::Pillar => "mesh.pillar",
        }
    }

    /// Convert from type_id string
    pub fn from_type_id(type_id: &str) -> Option<Self> {
        match type_id {
            "mesh.cube" => Some(MeshPrimitiveType::Cube),
            "mesh.sphere" => Some(MeshPrimitiveType::Sphere),
            "mesh.cylinder" => Some(MeshPrimitiveType::Cylinder),
            "mesh.plane" => Some(MeshPrimitiveType::Plane),
            "mesh.cone" => Some(MeshPrimitiveType::Cone),
            "mesh.torus" => Some(MeshPrimitiveType::Torus),
            "mesh.capsule" => Some(MeshPrimitiveType::Capsule),
            "mesh.wedge" => Some(MeshPrimitiveType::Wedge),
            "mesh.stairs" => Some(MeshPrimitiveType::Stairs),
            "mesh.arch" => Some(MeshPrimitiveType::Arch),
            "mesh.half_cylinder" => Some(MeshPrimitiveType::HalfCylinder),
            "mesh.quarter_pipe" => Some(MeshPrimitiveType::QuarterPipe),
            "mesh.corner" => Some(MeshPrimitiveType::Corner),
            "mesh.prism" => Some(MeshPrimitiveType::Prism),
            "mesh.pyramid" => Some(MeshPrimitiveType::Pyramid),
            "mesh.pipe" => Some(MeshPrimitiveType::Pipe),
            "mesh.ring" => Some(MeshPrimitiveType::Ring),
            "mesh.wall" => Some(MeshPrimitiveType::Wall),
            "mesh.ramp" => Some(MeshPrimitiveType::Ramp),
            "mesh.hemisphere" => Some(MeshPrimitiveType::Hemisphere),
            "mesh.curved_wall" => Some(MeshPrimitiveType::CurvedWall),
            "mesh.doorway" => Some(MeshPrimitiveType::Doorway),
            "mesh.window_wall" => Some(MeshPrimitiveType::WindowWall),
            "mesh.l_shape" => Some(MeshPrimitiveType::LShape),
            "mesh.t_shape" => Some(MeshPrimitiveType::TShape),
            "mesh.cross_shape" => Some(MeshPrimitiveType::CrossShape),
            "mesh.funnel" => Some(MeshPrimitiveType::Funnel),
            "mesh.gutter" => Some(MeshPrimitiveType::Gutter),
            "mesh.spiral_stairs" => Some(MeshPrimitiveType::SpiralStairs),
            "mesh.pillar" => Some(MeshPrimitiveType::Pillar),
            _ => None,
        }
    }

    /// Display name for the mesh type
    pub fn display_name(&self) -> &'static str {
        match self {
            MeshPrimitiveType::Cube => "Cube",
            MeshPrimitiveType::Sphere => "Sphere",
            MeshPrimitiveType::Cylinder => "Cylinder",
            MeshPrimitiveType::Plane => "Plane",
            MeshPrimitiveType::Cone => "Cone",
            MeshPrimitiveType::Torus => "Torus",
            MeshPrimitiveType::Capsule => "Capsule",
            MeshPrimitiveType::Wedge => "Wedge",
            MeshPrimitiveType::Stairs => "Stairs",
            MeshPrimitiveType::Arch => "Arch",
            MeshPrimitiveType::HalfCylinder => "Half Cylinder",
            MeshPrimitiveType::QuarterPipe => "Quarter Pipe",
            MeshPrimitiveType::Corner => "Corner",
            MeshPrimitiveType::Prism => "Prism",
            MeshPrimitiveType::Pyramid => "Pyramid",
            MeshPrimitiveType::Pipe => "Pipe",
            MeshPrimitiveType::Ring => "Ring",
            MeshPrimitiveType::Wall => "Wall",
            MeshPrimitiveType::Ramp => "Ramp",
            MeshPrimitiveType::Hemisphere => "Hemisphere",
            MeshPrimitiveType::CurvedWall => "Curved Wall",
            MeshPrimitiveType::Doorway => "Doorway",
            MeshPrimitiveType::WindowWall => "Window Wall",
            MeshPrimitiveType::LShape => "L-Shape",
            MeshPrimitiveType::TShape => "T-Shape",
            MeshPrimitiveType::CrossShape => "Cross",
            MeshPrimitiveType::Funnel => "Funnel",
            MeshPrimitiveType::Gutter => "Gutter",
            MeshPrimitiveType::SpiralStairs => "Spiral Stairs",
            MeshPrimitiveType::Pillar => "Pillar",
        }
    }

    /// All mesh primitive types
    pub fn all() -> &'static [MeshPrimitiveType] {
        &[
            MeshPrimitiveType::Cube,
            MeshPrimitiveType::Sphere,
            MeshPrimitiveType::Cylinder,
            MeshPrimitiveType::Plane,
            MeshPrimitiveType::Cone,
            MeshPrimitiveType::Torus,
            MeshPrimitiveType::Capsule,
            MeshPrimitiveType::Wedge,
            MeshPrimitiveType::Stairs,
            MeshPrimitiveType::Arch,
            MeshPrimitiveType::HalfCylinder,
            MeshPrimitiveType::QuarterPipe,
            MeshPrimitiveType::Corner,
            MeshPrimitiveType::Prism,
            MeshPrimitiveType::Pyramid,
            MeshPrimitiveType::Pipe,
            MeshPrimitiveType::Ring,
            MeshPrimitiveType::Wall,
            MeshPrimitiveType::Ramp,
            MeshPrimitiveType::Hemisphere,
            MeshPrimitiveType::CurvedWall,
            MeshPrimitiveType::Doorway,
            MeshPrimitiveType::WindowWall,
            MeshPrimitiveType::LShape,
            MeshPrimitiveType::TShape,
            MeshPrimitiveType::CrossShape,
            MeshPrimitiveType::Funnel,
            MeshPrimitiveType::Gutter,
            MeshPrimitiveType::SpiralStairs,
            MeshPrimitiveType::Pillar,
        ]
    }

    /// Category for organizing in the shape library
    pub fn category(&self) -> ShapeCategory {
        match self {
            MeshPrimitiveType::Cube | MeshPrimitiveType::Sphere |
            MeshPrimitiveType::Cylinder | MeshPrimitiveType::Plane |
            MeshPrimitiveType::Cone | MeshPrimitiveType::Capsule |
            MeshPrimitiveType::Hemisphere => ShapeCategory::Basic,

            MeshPrimitiveType::Wedge | MeshPrimitiveType::Stairs |
            MeshPrimitiveType::Wall | MeshPrimitiveType::Corner |
            MeshPrimitiveType::Arch | MeshPrimitiveType::QuarterPipe |
            MeshPrimitiveType::HalfCylinder | MeshPrimitiveType::Ramp |
            MeshPrimitiveType::Doorway | MeshPrimitiveType::WindowWall |
            MeshPrimitiveType::LShape | MeshPrimitiveType::TShape |
            MeshPrimitiveType::CrossShape | MeshPrimitiveType::SpiralStairs |
            MeshPrimitiveType::CurvedWall | MeshPrimitiveType::Pillar => ShapeCategory::LevelBuilding,

            MeshPrimitiveType::Torus | MeshPrimitiveType::Pipe |
            MeshPrimitiveType::Ring | MeshPrimitiveType::Funnel |
            MeshPrimitiveType::Gutter => ShapeCategory::Curved,

            MeshPrimitiveType::Prism | MeshPrimitiveType::Pyramid => ShapeCategory::Advanced,
        }
    }
}

/// Categories for organizing shapes in the Shape Library panel
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ShapeCategory {
    #[default]
    All,
    Basic,
    LevelBuilding,
    Curved,
    Advanced,
}

impl ShapeCategory {
    pub fn display_name(&self) -> &'static str {
        match self {
            ShapeCategory::All => "All",
            ShapeCategory::Basic => "Basic",
            ShapeCategory::LevelBuilding => "Level",
            ShapeCategory::Curved => "Curved",
            ShapeCategory::Advanced => "Advanced",
        }
    }

    pub fn all_categories() -> &'static [ShapeCategory] {
        &[
            ShapeCategory::All,
            ShapeCategory::Basic,
            ShapeCategory::LevelBuilding,
            ShapeCategory::Curved,
            ShapeCategory::Advanced,
        ]
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
            intensity: 100_000.0,
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
            intensity: 100_000.0,
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
// 3D Text Settings
// =============================================================================

/// Data component for 3D world-space text nodes
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component)]
pub struct Text3DData {
    /// The text string to display
    pub text: String,
    /// Font size in world-space pixels
    pub font_size: f32,
    /// Text color (RGBA, 0-1 range)
    pub color: Vec4,
}

impl Default for Text3DData {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            // 64px in a 128px-tall render target fills ~50% of the quad height,
            // giving clearly readable text by default.
            font_size: 64.0,
            color: Vec4::ONE,
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
