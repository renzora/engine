//! Rendering-related component data types

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Data component for mesh nodes - stores the mesh type so it can be serialized
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
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
