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
// Rendering Options
// ============================================================================

/// Blend mode for particle rendering
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

/// Billboard orientation mode
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

    // ---- Forces ----
    /// Constant acceleration (e.g., gravity: [0, -9.8, 0])
    pub acceleration: [f32; 3],
    /// Linear drag coefficient (velocity reduction per second)
    pub linear_drag: f32,
    /// Radial acceleration (positive = outward, negative = inward)
    pub radial_acceleration: f32,
    /// Tangent acceleration (creates swirling effects)
    pub tangent_acceleration: f32,

    // ---- Size ----
    /// Initial particle size
    pub size_start: f32,
    /// Final particle size (at end of lifetime)
    pub size_end: f32,
    /// Optional curve for size over lifetime
    pub size_curve: Vec<CurvePoint>,

    // ---- Color ----
    /// Color gradient over particle lifetime
    pub color_gradient: Vec<GradientStop>,

    // ---- Rendering ----
    /// Blend mode for particles
    pub blend_mode: BlendMode,
    /// Optional texture path for particles
    pub texture_path: Option<String>,
    /// Billboard orientation mode
    pub billboard_mode: BillboardMode,
    /// Render layer (0-31)
    pub render_layer: u8,

    // ---- Simulation ----
    /// Coordinate space for simulation
    pub simulation_space: SimulationSpace,
    /// When to simulate
    pub simulation_condition: SimulationCondition,

    // ---- Custom Variables ----
    /// User-defined variables exposed to scripts
    #[reflect(ignore)]
    pub variables: HashMap<String, EffectVariable>,
}

impl Default for HanabiEffectDefinition {
    fn default() -> Self {
        Self {
            name: "New Effect".to_string(),
            capacity: 1000,

            spawn_mode: SpawnMode::Rate,
            spawn_rate: 50.0,
            spawn_count: 10,

            lifetime_min: 1.0,
            lifetime_max: 2.0,

            emit_shape: HanabiEmitShape::Point,

            velocity_mode: VelocityMode::Directional,
            velocity_magnitude: 2.0,
            velocity_spread: 0.3,
            velocity_direction: [0.0, 1.0, 0.0],

            acceleration: [0.0, -2.0, 0.0],
            linear_drag: 0.0,
            radial_acceleration: 0.0,
            tangent_acceleration: 0.0,

            size_start: 0.1,
            size_end: 0.0,
            size_curve: Vec::new(),

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

            blend_mode: BlendMode::Blend,
            texture_path: None,
            billboard_mode: BillboardMode::FaceCamera,
            render_layer: 0,

            simulation_space: SimulationSpace::Local,
            simulation_condition: SimulationCondition::Always,

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
