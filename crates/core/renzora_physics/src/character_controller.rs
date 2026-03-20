use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Serializable character controller configuration.
///
/// When added to an entity, this component provides kinematic character movement
/// with ground detection, jumping, slope handling, and gravity — driven by
/// `CharacterControllerInput`.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct CharacterControllerData {
    /// When true, the controller reads input actions directly from the InputMap.
    /// No blueprint wiring needed — add the component and it moves.
    /// Set to false if you want to drive input from blueprints/scripts instead.
    #[serde(default = "default_true")]
    pub auto_input: bool,
    /// Input action name for movement (default: "move"). Must be an Axis2D action.
    #[serde(default = "default_move_action")]
    pub move_action: String,
    /// Input action name for jumping (default: "jump"). Must be a Button action.
    #[serde(default = "default_jump_action")]
    pub jump_action: String,
    /// Input action name for sprinting (default: "sprint"). Must be a Button action.
    #[serde(default = "default_sprint_action")]
    pub sprint_action: String,
    /// Movement speed in units/second.
    #[serde(default = "default_move_speed")]
    pub move_speed: f32,
    /// Sprint speed multiplier.
    #[serde(default = "default_sprint_multiplier")]
    pub sprint_multiplier: f32,
    /// Upward impulse applied on jump.
    #[serde(default = "default_jump_force")]
    pub jump_force: f32,
    /// Multiplier on world gravity (1.0 = normal gravity).
    #[serde(default = "default_one")]
    pub gravity_scale: f32,
    /// Maximum walkable slope angle in degrees.
    #[serde(default = "default_max_slope")]
    pub max_slope_angle: f32,
    /// How far below the collider to check for ground.
    #[serde(default = "default_ground_distance")]
    pub ground_distance: f32,
    /// Movement influence while airborne (0.0–1.0).
    #[serde(default = "default_air_control")]
    pub air_control: f32,
    /// Time after leaving ground where jump is still allowed.
    #[serde(default = "default_coyote_time")]
    pub coyote_time: f32,
    /// Time before landing where a jump input is buffered.
    #[serde(default = "default_jump_buffer")]
    pub jump_buffer_time: f32,
}

fn default_true() -> bool { true }
fn default_move_action() -> String { "move".into() }
fn default_jump_action() -> String { "jump".into() }
fn default_sprint_action() -> String { "sprint".into() }
fn default_move_speed() -> f32 { 6.0 }
fn default_sprint_multiplier() -> f32 { 1.5 }
fn default_jump_force() -> f32 { 7.0 }
fn default_one() -> f32 { 1.0 }
fn default_max_slope() -> f32 { 45.0 }
fn default_ground_distance() -> f32 { 0.08 }
fn default_air_control() -> f32 { 0.3 }
fn default_coyote_time() -> f32 { 0.15 }
fn default_jump_buffer() -> f32 { 0.1 }

impl Default for CharacterControllerData {
    fn default() -> Self {
        Self {
            auto_input: true,
            move_action: "move".into(),
            jump_action: "jump".into(),
            sprint_action: "sprint".into(),
            move_speed: 6.0,
            sprint_multiplier: 1.5,
            jump_force: 7.0,
            gravity_scale: 1.0,
            max_slope_angle: 45.0,
            ground_distance: 0.08,
            air_control: 0.3,
            coyote_time: 0.15,
            jump_buffer_time: 0.1,
        }
    }
}

/// Per-entity input that drives the character controller.
///
/// Written by blueprints, scripts, or AI systems each frame.
/// If no system writes this, the controller stands still.
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct CharacterControllerInput {
    /// Movement direction on the XZ plane (normalized or zero).
    pub movement: Vec2,
    /// True on the frame jump is requested.
    pub jump: bool,
    /// Whether sprint is held.
    pub sprint: bool,
}

/// Runtime state for the character controller (not serialized).
#[derive(Component, Clone, Debug)]
pub struct CharacterControllerState {
    /// Current velocity (managed by the controller, not raw physics).
    pub velocity: Vec3,
    /// Whether the character is on the ground.
    pub is_grounded: bool,
    /// Normal of the ground surface (if grounded).
    pub ground_normal: Vec3,
    /// Time since the character last left the ground (for coyote time).
    pub airborne_timer: f32,
    /// Time since jump was last requested (for jump buffering).
    pub jump_buffer_timer: f32,
    /// Whether the character was grounded last frame.
    pub was_grounded: bool,
}

impl Default for CharacterControllerState {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            is_grounded: false,
            ground_normal: Vec3::Y,
            airborne_timer: 999.0,
            jump_buffer_timer: 999.0,
            was_grounded: false,
        }
    }
}
