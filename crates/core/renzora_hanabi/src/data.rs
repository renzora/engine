//! Particle effect data structures
//!
//! Defines the serializable data types for particle effects that can be
//! stored in scenes, edited in the UI, and converted to bevy_hanabi assets.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::node_graph::ParticleNodeGraph;

// ============================================================================
// Emission Shapes
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ShapeDimension {
    #[default]
    Volume,
    Surface,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum HanabiEmitShape {
    Point,
    Circle {
        radius: f32,
        dimension: ShapeDimension,
    },
    Sphere {
        radius: f32,
        dimension: ShapeDimension,
    },
    Cone {
        base_radius: f32,
        top_radius: f32,
        height: f32,
        dimension: ShapeDimension,
    },
    Rect {
        half_extents: [f32; 2],
        dimension: ShapeDimension,
    },
    Box {
        half_extents: [f32; 3],
    },
}

impl Default for HanabiEmitShape {
    fn default() -> Self {
        Self::Point
    }
}

// ============================================================================
// Spawn Mode
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SpawnMode {
    #[default]
    Rate,
    Burst,
    BurstRate,
}

// ============================================================================
// Velocity Mode
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum VelocityMode {
    #[default]
    Directional,
    Radial,
    Tangent,
    Random,
}

// ============================================================================
// Rendering Options
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Blend,
    Additive,
    Multiply,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum BillboardMode {
    #[default]
    FaceCamera,
    FaceCameraY,
    Velocity,
    Fixed,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleAlphaMode {
    #[default]
    Blend,
    Premultiply,
    Add,
    Multiply,
    Mask,
    Opaque,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleOrientMode {
    #[default]
    ParallelCameraDepthPlane,
    FaceCameraPosition,
    AlongVelocity,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum MotionIntegrationMode {
    #[default]
    PostUpdate,
    PreUpdate,
    None,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleColorBlendMode {
    #[default]
    Modulate,
    Overwrite,
    Add,
}

// ============================================================================
// Kill Zones
// ============================================================================

#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum KillZone {
    Sphere {
        center: [f32; 3],
        radius: f32,
        kill_inside: bool,
    },
    Aabb {
        center: [f32; 3],
        half_size: [f32; 3],
        kill_inside: bool,
    },
}

impl Default for KillZone {
    fn default() -> Self {
        Self::Sphere {
            center: [0.0, 0.0, 0.0],
            radius: 5.0,
            kill_inside: false,
        }
    }
}

// ============================================================================
// Conform to Sphere
// ============================================================================

#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct ConformToSphere {
    pub origin: [f32; 3],
    pub radius: f32,
    pub influence_dist: f32,
    pub attraction_accel: f32,
    pub max_attraction_speed: f32,
    pub shell_half_thickness: f32,
    pub sticky_factor: f32,
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct OrbitSettings {
    pub center: [f32; 3],
    pub axis: [f32; 3],
    pub speed: f32,
    pub radial_pull: f32,
    pub orbit_radius: f32,
}

impl Default for OrbitSettings {
    fn default() -> Self {
        Self {
            center: [0.0, 0.0, 0.0],
            axis: [0.0, 1.0, 0.0],
            speed: 1.0,
            radial_pull: 0.0,
            orbit_radius: 1.0,
        }
    }
}

impl Default for ConformToSphere {
    fn default() -> Self {
        Self {
            origin: [0.0, 0.0, 0.0],
            radius: 1.0,
            influence_dist: 3.0,
            attraction_accel: 5.0,
            max_attraction_speed: 2.0,
            shell_half_thickness: 0.1,
            sticky_factor: 0.5,
        }
    }
}

// ============================================================================
// Flipbook Animation
// ============================================================================

#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct FlipbookSettings {
    pub grid_columns: u32,
    pub grid_rows: u32,
}

impl Default for FlipbookSettings {
    fn default() -> Self {
        Self {
            grid_columns: 4,
            grid_rows: 4,
        }
    }
}

// ============================================================================
// Simulation Options
// ============================================================================

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SimulationSpace {
    #[default]
    Local,
    World,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SimulationCondition {
    #[default]
    Always,
    WhenVisible,
}

// ============================================================================
// Curves and Gradients
// ============================================================================

#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct CurvePoint {
    pub time: f32,
    pub value: f32,
}

impl Default for CurvePoint {
    fn default() -> Self {
        Self { time: 0.0, value: 1.0 }
    }
}

#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct GradientStop {
    pub position: f32,
    pub color: [f32; 4],
}

impl Default for GradientStop {
    fn default() -> Self {
        Self {
            position: 0.0,
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

// ============================================================================
// Custom Variables
// ============================================================================

#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum EffectVariable {
    Float { value: f32, min: f32, max: f32 },
    Color { value: [f32; 4] },
    Vec3 { value: [f32; 3] },
}

impl Default for EffectVariable {
    fn default() -> Self {
        Self::Float { value: 1.0, min: 0.0, max: 1.0 }
    }
}

// ============================================================================
// Effect Definition
// ============================================================================

#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct HanabiEffectDefinition {
    pub name: String,
    pub capacity: u32,

    // Spawning
    pub spawn_mode: SpawnMode,
    pub spawn_rate: f32,
    pub spawn_count: u32,
    #[serde(default)]
    pub spawn_duration: f32,
    #[serde(default)]
    pub spawn_cycle_count: u32,
    #[serde(default = "default_true")]
    pub spawn_starts_active: bool,

    // Lifetime
    pub lifetime_min: f32,
    pub lifetime_max: f32,

    // Emission Shape
    pub emit_shape: HanabiEmitShape,

    // Initial Velocity
    pub velocity_mode: VelocityMode,
    pub velocity_magnitude: f32,
    pub velocity_spread: f32,
    pub velocity_direction: [f32; 3],
    #[serde(default)]
    pub velocity_speed_min: f32,
    #[serde(default)]
    pub velocity_speed_max: f32,
    #[serde(default = "default_y_axis")]
    pub velocity_axis: [f32; 3],

    // Forces
    pub acceleration: [f32; 3],
    pub linear_drag: f32,
    pub radial_acceleration: f32,
    pub tangent_acceleration: f32,
    #[serde(default = "default_y_axis")]
    pub tangent_accel_axis: [f32; 3],
    #[serde(default)]
    pub conform_to_sphere: Option<ConformToSphere>,

    // Noise Turbulence
    #[serde(default)]
    pub noise_frequency: f32,
    #[serde(default)]
    pub noise_amplitude: f32,
    #[serde(default = "default_noise_octaves")]
    pub noise_octaves: u32,
    #[serde(default = "default_noise_lacunarity")]
    pub noise_lacunarity: f32,

    // Orbit
    #[serde(default)]
    pub orbit: Option<OrbitSettings>,

    // Velocity Limit
    #[serde(default)]
    pub velocity_limit: f32,

    // Size
    pub size_start: f32,
    pub size_end: f32,
    pub size_curve: Vec<CurvePoint>,
    #[serde(default)]
    pub size_start_min: f32,
    #[serde(default)]
    pub size_start_max: f32,
    #[serde(default)]
    pub size_non_uniform: bool,
    #[serde(default = "default_size")]
    pub size_start_x: f32,
    #[serde(default = "default_size")]
    pub size_start_y: f32,
    #[serde(default)]
    pub size_end_x: f32,
    #[serde(default)]
    pub size_end_y: f32,
    #[serde(default)]
    pub screen_space_size: bool,
    #[serde(default)]
    pub roundness: f32,

    // Color
    pub color_gradient: Vec<GradientStop>,
    #[serde(default)]
    pub use_flat_color: bool,
    #[serde(default = "default_white")]
    pub flat_color: [f32; 4],
    #[serde(default)]
    pub use_hdr_color: bool,
    #[serde(default = "default_one")]
    pub hdr_intensity: f32,
    #[serde(default)]
    pub color_blend_mode: ParticleColorBlendMode,

    // Rendering
    pub blend_mode: BlendMode,
    pub texture_path: Option<String>,
    pub billboard_mode: BillboardMode,
    pub render_layer: u8,
    #[serde(default)]
    pub alpha_mode: ParticleAlphaMode,
    #[serde(default = "default_half")]
    pub alpha_mask_threshold: f32,
    #[serde(default)]
    pub orient_mode: ParticleOrientMode,
    #[serde(default)]
    pub rotation_speed: f32,
    #[serde(default)]
    pub flipbook: Option<FlipbookSettings>,

    // Simulation
    pub simulation_space: SimulationSpace,
    pub simulation_condition: SimulationCondition,
    #[serde(default)]
    pub motion_integration: MotionIntegrationMode,

    // Kill Zones
    #[serde(default)]
    pub kill_zones: Vec<KillZone>,

    // Custom Variables
    #[reflect(ignore)]
    pub variables: HashMap<String, EffectVariable>,
}

fn default_noise_octaves() -> u32 { 3 }
fn default_noise_lacunarity() -> f32 { 2.0 }
fn default_true() -> bool { true }
fn default_y_axis() -> [f32; 3] { [0.0, 1.0, 0.0] }
fn default_size() -> f32 { 0.1 }
fn default_white() -> [f32; 4] { [1.0, 1.0, 1.0, 1.0] }
fn default_one() -> f32 { 1.0 }
fn default_half() -> f32 { 0.5 }

impl Default for HanabiEffectDefinition {
    fn default() -> Self {
        Self {
            name: "New Effect".to_string(),
            capacity: 1000,
            spawn_mode: SpawnMode::Rate,
            spawn_rate: 50.0,
            spawn_count: 10,
            spawn_duration: 0.0,
            spawn_cycle_count: 0,
            spawn_starts_active: true,
            lifetime_min: 1.0,
            lifetime_max: 2.0,
            emit_shape: HanabiEmitShape::Point,
            velocity_mode: VelocityMode::Directional,
            velocity_magnitude: 2.0,
            velocity_spread: 0.3,
            velocity_direction: [0.0, 1.0, 0.0],
            velocity_speed_min: 0.0,
            velocity_speed_max: 0.0,
            velocity_axis: [0.0, 1.0, 0.0],
            acceleration: [0.0, -2.0, 0.0],
            linear_drag: 0.0,
            radial_acceleration: 0.0,
            tangent_acceleration: 0.0,
            tangent_accel_axis: [0.0, 1.0, 0.0],
            conform_to_sphere: None,
            noise_frequency: 0.0,
            noise_amplitude: 0.0,
            noise_octaves: 3,
            noise_lacunarity: 2.0,
            orbit: None,
            velocity_limit: 0.0,
            size_start: 0.1,
            size_end: 0.0,
            size_curve: Vec::new(),
            size_start_min: 0.0,
            size_start_max: 0.0,
            size_non_uniform: false,
            size_start_x: 0.1,
            size_start_y: 0.1,
            size_end_x: 0.0,
            size_end_y: 0.0,
            screen_space_size: false,
            roundness: 0.0,
            color_gradient: vec![
                GradientStop { position: 0.0, color: [1.0, 1.0, 1.0, 1.0] },
                GradientStop { position: 1.0, color: [1.0, 1.0, 1.0, 0.0] },
            ],
            use_flat_color: false,
            flat_color: [1.0, 1.0, 1.0, 1.0],
            use_hdr_color: false,
            hdr_intensity: 1.0,
            color_blend_mode: ParticleColorBlendMode::Modulate,
            blend_mode: BlendMode::Blend,
            texture_path: None,
            billboard_mode: BillboardMode::FaceCamera,
            render_layer: 0,
            alpha_mode: ParticleAlphaMode::Blend,
            alpha_mask_threshold: 0.5,
            orient_mode: ParticleOrientMode::ParallelCameraDepthPlane,
            rotation_speed: 0.0,
            flipbook: None,
            simulation_space: SimulationSpace::Local,
            simulation_condition: SimulationCondition::Always,
            motion_integration: MotionIntegrationMode::PostUpdate,
            kill_zones: Vec::new(),
            variables: HashMap::new(),
        }
    }
}

// ============================================================================
// Effect Source
// ============================================================================

#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum EffectSource {
    Asset { path: String },
    Inline { definition: HanabiEffectDefinition },
}

impl Default for EffectSource {
    fn default() -> Self {
        Self::Inline {
            definition: HanabiEffectDefinition::default(),
        }
    }
}

// ============================================================================
// Component
// ============================================================================

#[derive(Component, Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Component, Serialize, Deserialize)]
pub struct HanabiEffectData {
    pub source: EffectSource,
    pub playing: bool,
    pub rate_multiplier: f32,
    pub scale_multiplier: f32,
    pub color_tint: [f32; 4],
    pub time_scale: f32,
    #[reflect(ignore)]
    pub variable_overrides: HashMap<String, EffectVariable>,
}

impl Default for HanabiEffectData {
    fn default() -> Self {
        Self {
            source: EffectSource::default(),
            playing: true,
            rate_multiplier: 1.0,
            scale_multiplier: 1.0,
            color_tint: [1.0, 1.0, 1.0, 1.0],
            time_scale: 1.0,
            variable_overrides: HashMap::new(),
        }
    }
}

// ============================================================================
// Editor Mode
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorMode {
    #[default]
    Simple,
    Graph,
}

// ============================================================================
// Editor State
// ============================================================================

#[derive(Resource, Default)]
pub struct ParticleEditorState {
    pub current_effect: Option<HanabiEffectDefinition>,
    pub current_file_path: Option<String>,
    pub is_modified: bool,
    pub selected_gradient_stop: Option<usize>,
    pub selected_curve_point: Option<usize>,
    pub preview_playing: bool,
    pub recently_saved_paths: Vec<String>,
    pub node_graph: Option<ParticleNodeGraph>,
    pub editor_mode: EditorMode,
    pub selected_node: Option<u64>,
}

#[derive(Resource, Default)]
pub struct ParticlePreviewState {
    pub preview_entity: Option<Entity>,
    pub camera_entity: Option<Entity>,
    pub is_active: bool,
}

// ============================================================================
// File I/O
// ============================================================================

/// Load a particle effect definition from a .particle file (RON format).
pub fn load_effect_from_file(path: &std::path::Path) -> Option<HanabiEffectDefinition> {
    match std::fs::read_to_string(path) {
        Ok(contents) => {
            match ron::from_str::<HanabiEffectDefinition>(&contents) {
                Ok(effect) => Some(effect),
                Err(e) => {
                    bevy::log::error!("Failed to parse particle effect {:?}: {}", path, e);
                    let mut effect = HanabiEffectDefinition::default();
                    effect.name = path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string();
                    Some(effect)
                }
            }
        }
        Err(e) => {
            bevy::log::error!("Failed to read particle effect {:?}: {}", path, e);
            None
        }
    }
}
