//! Particle effect data structures
//!
//! Defines the serializable data types for particle effects that can be
//! stored in scenes, edited in the UI, and converted to bevy_hanabi assets.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Emission Shapes
// ============================================================================

/// Dimension mode for emission shapes
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ShapeDimension {
    /// Emit from the entire volume
    #[default]
    Volume,
    /// Emit only from the surface
    Surface,
}

/// Emission shape types matching bevy_hanabi
#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum HanabiEmitShape {
    /// Emit from a single point
    Point,
    /// Emit from a circle
    Circle {
        radius: f32,
        dimension: ShapeDimension,
    },
    /// Emit from a sphere
    Sphere {
        radius: f32,
        dimension: ShapeDimension,
    },
    /// Emit from a cone
    Cone {
        base_radius: f32,
        top_radius: f32,
        height: f32,
        dimension: ShapeDimension,
    },
    /// Emit from a rectangle
    Rect {
        half_extents: [f32; 2],
        dimension: ShapeDimension,
    },
    /// Emit from a box volume
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

/// How particles are spawned
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SpawnMode {
    /// Continuous emission at a fixed rate (particles per second)
    #[default]
    Rate,
    /// Single burst of particles
    Burst,
    /// Repeated bursts at a fixed rate
    BurstRate,
}

// ============================================================================
// Velocity Mode
// ============================================================================

/// How initial particle velocity is determined
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum VelocityMode {
    /// Particles move in a specified direction with optional spread
    #[default]
    Directional,
    /// Particles move outward from the emission center
    Radial,
    /// Particles move tangent to the radial direction
    Tangent,
    /// Random velocity in a sphere
    Random,
}

// ============================================================================
// Rendering Options (Legacy - kept for backward compat)
// ============================================================================

/// Blend mode for particle rendering (legacy, kept for backward compat)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum BlendMode {
    /// Standard alpha blending
    #[default]
    Blend,
    /// Additive blending (for fire, glow effects)
    Additive,
    /// Multiplicative blending
    Multiply,
}

/// Billboard orientation mode (legacy, kept for backward compat)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum BillboardMode {
    /// Always face the camera
    #[default]
    FaceCamera,
    /// Face camera but lock Y axis (vertical billboards)
    FaceCameraY,
    /// Align to velocity direction
    Velocity,
    /// No billboarding, use world rotation
    Fixed,
}

// ============================================================================
// New Rendering Enums
// ============================================================================

/// Alpha mode for particle rendering (maps to bevy_hanabi AlphaMode)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleAlphaMode {
    /// Standard alpha blending
    #[default]
    Blend,
    /// Premultiplied alpha
    Premultiply,
    /// Additive blending
    Add,
    /// Multiplicative blending
    Multiply,
    /// Alpha mask with threshold
    Mask,
    /// Fully opaque
    Opaque,
}

/// Orientation mode for particles (maps to bevy_hanabi OrientMode)
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleOrientMode {
    /// Face camera's depth plane (cheaper, default)
    #[default]
    ParallelCameraDepthPlane,
    /// Point directly at camera position (more expensive)
    FaceCameraPosition,
    /// Align with velocity direction
    AlongVelocity,
}

/// Motion integration mode
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum MotionIntegrationMode {
    /// Apply velocity integration after update modifiers (default)
    #[default]
    PostUpdate,
    /// Apply velocity integration before update modifiers
    PreUpdate,
    /// No automatic velocity integration
    None,
}

/// Color blend mode for particle color modifiers
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum ParticleColorBlendMode {
    /// Multiply with destination (default)
    #[default]
    Modulate,
    /// Replace destination color
    Overwrite,
    /// Add to destination color
    Add,
}

// ============================================================================
// Kill Zones
// ============================================================================

/// A kill zone that removes particles entering/exiting a region
#[derive(Clone, Serialize, Deserialize, PartialEq, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum KillZone {
    /// Spherical kill zone
    Sphere {
        center: [f32; 3],
        radius: f32,
        kill_inside: bool,
    },
    /// Axis-aligned bounding box kill zone
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

/// Settings for the conform-to-sphere attractor modifier
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

/// Settings for flipbook sprite sheet animation
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

/// Coordinate space for particle simulation
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SimulationSpace {
    /// Particles move with the emitter
    #[default]
    Local,
    /// Particles stay in world space when emitter moves
    World,
}

/// When to simulate particles
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Default, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum SimulationCondition {
    /// Always simulate
    #[default]
    Always,
    /// Only simulate when visible
    WhenVisible,
}

// ============================================================================
// Curves and Gradients
// ============================================================================

/// A point on a curve (time 0.0-1.0, value)
#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct CurvePoint {
    /// Time along the curve (0.0 to 1.0, representing particle lifetime)
    pub time: f32,
    /// Value at this point
    pub value: f32,
}

impl Default for CurvePoint {
    fn default() -> Self {
        Self {
            time: 0.0,
            value: 1.0,
        }
    }
}

/// A color stop in a gradient
#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct GradientStop {
    /// Position in the gradient (0.0 to 1.0)
    pub position: f32,
    /// RGBA color at this position
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
// Custom Variables (for scripting)
// ============================================================================

/// Custom variable types that can be exposed to scripts
#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum EffectVariable {
    /// Floating point value with optional range
    Float {
        value: f32,
        min: f32,
        max: f32,
    },
    /// RGBA color value
    Color {
        value: [f32; 4],
    },
    /// 3D vector value
    Vec3 {
        value: [f32; 3],
    },
}

impl Default for EffectVariable {
    fn default() -> Self {
        Self::Float {
            value: 1.0,
            min: 0.0,
            max: 1.0,
        }
    }
}

// ============================================================================
// Effect Definition
// ============================================================================

/// Complete definition of a particle effect
///
/// This can be stored in .effect asset files or embedded inline in components.
#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub struct HanabiEffectDefinition {
    /// Display name for the effect
    pub name: String,

    /// Maximum number of particles that can exist simultaneously
    pub capacity: u32,

    // ---- Spawning ----
    /// How particles are spawned
    pub spawn_mode: SpawnMode,
    /// Particles per second (for Rate mode) or bursts per second (for BurstRate)
    pub spawn_rate: f32,
    /// Number of particles per burst (for Burst and BurstRate modes)
    pub spawn_count: u32,
    /// Duration of spawning in seconds (0 = infinite)
    #[serde(default)]
    pub spawn_duration: f32,
    /// Number of spawn cycles (0 = infinite)
    #[serde(default)]
    pub spawn_cycle_count: u32,
    /// Whether spawner starts active
    #[serde(default = "default_true")]
    pub spawn_starts_active: bool,

    // ---- Lifetime ----
    /// Minimum particle lifetime in seconds
    pub lifetime_min: f32,
    /// Maximum particle lifetime in seconds
    pub lifetime_max: f32,

    // ---- Emission Shape ----
    /// Shape from which particles are emitted
    pub emit_shape: HanabiEmitShape,

    // ---- Initial Velocity ----
    /// How velocity is determined
    pub velocity_mode: VelocityMode,
    /// Base velocity magnitude
    pub velocity_magnitude: f32,
    /// Random spread angle in radians (for directional mode)
    pub velocity_spread: f32,
    /// Direction vector (for directional mode)
    pub velocity_direction: [f32; 3],
    /// Minimum speed (for randomized speed range)
    #[serde(default)]
    pub velocity_speed_min: f32,
    /// Maximum speed (for randomized speed range, 0 = use velocity_magnitude)
    #[serde(default)]
    pub velocity_speed_max: f32,
    /// Axis for tangent velocity mode
    #[serde(default = "default_y_axis")]
    pub velocity_axis: [f32; 3],

    // ---- Forces ----
    /// Constant acceleration (e.g., gravity: [0, -9.8, 0])
    pub acceleration: [f32; 3],
    /// Linear drag coefficient (velocity reduction per second)
    pub linear_drag: f32,
    /// Radial acceleration (positive = outward, negative = inward)
    pub radial_acceleration: f32,
    /// Tangent acceleration (creates swirling effects)
    pub tangent_acceleration: f32,
    /// Axis for tangent acceleration
    #[serde(default = "default_y_axis")]
    pub tangent_accel_axis: [f32; 3],
    /// Conform-to-sphere attractor
    #[serde(default)]
    pub conform_to_sphere: Option<ConformToSphere>,

    // ---- Size ----
    /// Initial particle size
    pub size_start: f32,
    /// Final particle size (at end of lifetime)
    pub size_end: f32,
    /// Optional curve for size over lifetime
    pub size_curve: Vec<CurvePoint>,
    /// Minimum start size (for randomization, 0 = use size_start)
    #[serde(default)]
    pub size_start_min: f32,
    /// Maximum start size (for randomization, 0 = use size_start)
    #[serde(default)]
    pub size_start_max: f32,
    /// Whether to use non-uniform (X/Y) sizing
    #[serde(default)]
    pub size_non_uniform: bool,
    /// Non-uniform start X size
    #[serde(default = "default_size")]
    pub size_start_x: f32,
    /// Non-uniform start Y size
    #[serde(default = "default_size")]
    pub size_start_y: f32,
    /// Non-uniform end X size
    #[serde(default)]
    pub size_end_x: f32,
    /// Non-uniform end Y size
    #[serde(default)]
    pub size_end_y: f32,
    /// Whether particle size is in screen-space pixels
    #[serde(default)]
    pub screen_space_size: bool,
    /// Particle roundness (0.0 = square, 1.0 = circle)
    #[serde(default)]
    pub roundness: f32,

    // ---- Color ----
    /// Color gradient over particle lifetime
    pub color_gradient: Vec<GradientStop>,
    /// Whether to use a flat color instead of gradient
    #[serde(default)]
    pub use_flat_color: bool,
    /// Flat color RGBA (used when use_flat_color is true)
    #[serde(default = "default_white")]
    pub flat_color: [f32; 4],
    /// Whether to use HDR color values
    #[serde(default)]
    pub use_hdr_color: bool,
    /// HDR intensity multiplier
    #[serde(default = "default_one")]
    pub hdr_intensity: f32,
    /// Color blend mode for color modifiers
    #[serde(default)]
    pub color_blend_mode: ParticleColorBlendMode,

    // ---- Rendering ----
    /// Blend mode for particles (legacy, kept for backward compat)
    pub blend_mode: BlendMode,
    /// Optional texture path for particles
    pub texture_path: Option<String>,
    /// Billboard orientation mode (legacy, kept for backward compat)
    pub billboard_mode: BillboardMode,
    /// Render layer (0-31)
    pub render_layer: u8,
    /// Alpha mode (replaces blend_mode for actual rendering)
    #[serde(default)]
    pub alpha_mode: ParticleAlphaMode,
    /// Alpha mask threshold (used when alpha_mode is Mask)
    #[serde(default = "default_half")]
    pub alpha_mask_threshold: f32,
    /// Orient mode (replaces billboard_mode for actual rendering)
    #[serde(default)]
    pub orient_mode: ParticleOrientMode,
    /// Rotation speed in radians/sec (applied via orient modifier)
    #[serde(default)]
    pub rotation_speed: f32,
    /// Flipbook animation settings
    #[serde(default)]
    pub flipbook: Option<FlipbookSettings>,

    // ---- Simulation ----
    /// Coordinate space for simulation
    pub simulation_space: SimulationSpace,
    /// When to simulate
    pub simulation_condition: SimulationCondition,
    /// Motion integration mode
    #[serde(default)]
    pub motion_integration: MotionIntegrationMode,

    // ---- Kill Zones ----
    /// Kill zones that remove particles
    #[serde(default)]
    pub kill_zones: Vec<KillZone>,

    // ---- Custom Variables ----
    /// User-defined variables exposed to scripts
    #[reflect(ignore)]
    pub variables: HashMap<String, EffectVariable>,
}

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
                GradientStop {
                    position: 0.0,
                    color: [1.0, 1.0, 1.0, 1.0],
                },
                GradientStop {
                    position: 1.0,
                    color: [1.0, 1.0, 1.0, 0.0],
                },
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

/// Where the effect definition comes from
#[derive(Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Serialize, Deserialize)]
pub enum EffectSource {
    /// Load from an external .effect asset file
    Asset { path: String },
    /// Embedded inline definition
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

/// Component for entities with Hanabi particle effects
///
/// This component stores the effect definition and runtime state.
/// The actual bevy_hanabi components (ParticleEffect, etc.) are
/// created/updated by the sync system.
#[derive(Component, Clone, Serialize, Deserialize, Reflect, Debug)]
#[reflect(Component, Serialize, Deserialize)]
pub struct HanabiEffectData {
    /// Where the effect definition comes from
    pub source: EffectSource,

    // ---- Runtime State (scriptable) ----
    /// Whether the effect is currently playing
    pub playing: bool,
    /// Multiplier for spawn rate (1.0 = normal)
    pub rate_multiplier: f32,
    /// Multiplier for particle size (1.0 = normal)
    pub scale_multiplier: f32,
    /// Color tint applied to all particles (RGBA, 1.0 = no tint)
    pub color_tint: [f32; 4],
    /// Time scale for effect (1.0 = normal speed)
    pub time_scale: f32,

    /// Runtime overrides for custom variables
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
// Editor State
// ============================================================================

/// State for the particle editor panel
#[derive(Resource, Default)]
pub struct ParticleEditorState {
    /// Currently edited effect definition (working copy)
    pub current_effect: Option<HanabiEffectDefinition>,
    /// Path to the current .effect file (if editing an asset)
    pub current_file_path: Option<String>,
    /// Whether the current effect has unsaved changes
    pub is_modified: bool,
    /// Currently selected gradient stop index
    pub selected_gradient_stop: Option<usize>,
    /// Currently selected curve point index
    pub selected_curve_point: Option<usize>,
    /// Whether the preview is playing
    pub preview_playing: bool,
}

/// State for the particle preview system
#[derive(Resource, Default)]
pub struct ParticlePreviewState {
    /// Entity used for preview
    pub preview_entity: Option<Entity>,
    /// Preview camera entity
    pub camera_entity: Option<Entity>,
    /// Whether preview is active
    pub is_active: bool,
}
