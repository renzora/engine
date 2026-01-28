//! Animation nodes
//!
//! Nodes for skeletal animation, sprite animation, and tweening.

use super::{NodeTypeDefinition, Pin, PinType, PinValue};

// =============================================================================
// SKELETAL ANIMATION
// =============================================================================

/// Play animation
pub static PLAY_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/play",
    display_name: "Play Animation",
    category: "Animation",
    description: "Play a skeletal animation clip",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("animation", "Animation", PinType::String),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("loop", "Loop", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Play animation once
pub static PLAY_ANIMATION_ONCE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/play_once",
    display_name: "Play Animation Once",
    category: "Animation",
    description: "Play an animation once (no loop)",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("animation", "Animation", PinType::String),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_finish", "On Finish", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Stop animation
pub static STOP_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/stop",
    display_name: "Stop Animation",
    category: "Animation",
    description: "Stop the current animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Pause animation
pub static PAUSE_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/pause",
    display_name: "Pause Animation",
    category: "Animation",
    description: "Pause the current animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Resume animation
pub static RESUME_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/resume",
    display_name: "Resume Animation",
    category: "Animation",
    description: "Resume a paused animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Set animation speed
pub static SET_ANIMATION_SPEED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/set_speed",
    display_name: "Set Animation Speed",
    category: "Animation",
    description: "Set the playback speed of animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("speed", "Speed", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Set animation time
pub static SET_ANIMATION_TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/set_time",
    display_name: "Set Animation Time",
    category: "Animation",
    description: "Set the current time of animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("time", "Time", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Get animation time
pub static GET_ANIMATION_TIME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/get_time",
    display_name: "Get Animation Time",
    category: "Animation",
    description: "Get the current animation time and progress",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("time", "Time", PinType::Float),
        Pin::output("duration", "Duration", PinType::Float),
        Pin::output("progress", "Progress", PinType::Float),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Is animation playing
pub static IS_ANIMATION_PLAYING: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/is_playing",
    display_name: "Is Animation Playing",
    category: "Animation",
    description: "Check if an animation is currently playing",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("playing", "Is Playing", PinType::Bool),
        Pin::output("name", "Current Anim", PinType::String),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ANIMATION BLENDING
// =============================================================================

/// Crossfade animation
pub static CROSSFADE_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/crossfade",
    display_name: "Crossfade Animation",
    category: "Animation",
    description: "Smoothly transition to a new animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("animation", "Animation", PinType::String),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(0.2)),
        Pin::input("loop", "Loop", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Set animation weight (for blending)
pub static SET_ANIMATION_WEIGHT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/set_weight",
    display_name: "Set Animation Weight",
    category: "Animation",
    description: "Set the blend weight of an animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("animation", "Animation", PinType::String),
        Pin::input("weight", "Weight", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// ANIMATION EVENTS
// =============================================================================

/// On animation finished
pub static ON_ANIMATION_FINISHED: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/on_finished",
    display_name: "On Animation Finished",
    category: "Animation Events",
    description: "Triggered when an animation finishes",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("name", "Animation Name", PinType::String),
    ],
    color: [100, 180, 160],
    is_event: true,
    is_comment: false,
};

/// On animation loop
pub static ON_ANIMATION_LOOP: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/on_loop",
    display_name: "On Animation Loop",
    category: "Animation Events",
    description: "Triggered when an animation loops",
    create_pins: || vec![
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("name", "Animation Name", PinType::String),
        Pin::output("count", "Loop Count", PinType::Int),
    ],
    color: [100, 180, 160],
    is_event: true,
    is_comment: false,
};

// =============================================================================
// TWEENING
// =============================================================================

/// Tween position
pub static TWEEN_POSITION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/tween_position",
    display_name: "Tween Position",
    category: "Animation",
    description: "Smoothly animate position over time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("easing", "Easing", PinType::String).with_default(PinValue::String("linear".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_complete", "On Complete", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Tween rotation
pub static TWEEN_ROTATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/tween_rotation",
    display_name: "Tween Rotation",
    category: "Animation",
    description: "Smoothly animate rotation over time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("easing", "Easing", PinType::String).with_default(PinValue::String("linear".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_complete", "On Complete", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Tween scale
pub static TWEEN_SCALE: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/tween_scale",
    display_name: "Tween Scale",
    category: "Animation",
    description: "Smoothly animate scale over time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target", PinType::Vec3).with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("easing", "Easing", PinType::String).with_default(PinValue::String("linear".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_complete", "On Complete", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Tween float value
pub static TWEEN_FLOAT: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/tween_float",
    display_name: "Tween Float",
    category: "Animation",
    description: "Smoothly interpolate a float value over time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("from", "From", PinType::Float).with_default(PinValue::Float(0.0)),
        Pin::input("to", "To", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("easing", "Easing", PinType::String).with_default(PinValue::String("linear".into())),
        Pin::output("update", "Update", PinType::Execution),
        Pin::output("value", "Value", PinType::Float),
        Pin::output("on_complete", "On Complete", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Tween color
pub static TWEEN_COLOR: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/tween_color",
    display_name: "Tween Color",
    category: "Animation",
    description: "Smoothly interpolate a color over time",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("target", "Target Color", PinType::Color).with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        Pin::input("duration", "Duration", PinType::Float).with_default(PinValue::Float(1.0)),
        Pin::input("easing", "Easing", PinType::String).with_default(PinValue::String("linear".into())),
        Pin::output("exec", "Exec", PinType::Execution),
        Pin::output("on_complete", "On Complete", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Cancel tween
pub static CANCEL_TWEEN: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/cancel_tween",
    display_name: "Cancel Tween",
    category: "Animation",
    description: "Cancel all tweens on an entity",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

// =============================================================================
// SPRITE ANIMATION
// =============================================================================

/// Play sprite animation
pub static PLAY_SPRITE_ANIMATION: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/play_sprite",
    display_name: "Play Sprite Animation",
    category: "Animation",
    description: "Play a sprite sheet animation",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("animation", "Animation", PinType::String),
        Pin::input("fps", "FPS", PinType::Float).with_default(PinValue::Float(12.0)),
        Pin::input("loop", "Loop", PinType::Bool).with_default(PinValue::Bool(true)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Set sprite frame
pub static SET_SPRITE_FRAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/set_sprite_frame",
    display_name: "Set Sprite Frame",
    category: "Animation",
    description: "Set the current frame of a sprite sheet",
    create_pins: || vec![
        Pin::input("exec", "Exec", PinType::Execution),
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::input("frame", "Frame", PinType::Int).with_default(PinValue::Int(0)),
        Pin::output("exec", "Exec", PinType::Execution),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};

/// Get sprite frame
pub static GET_SPRITE_FRAME: NodeTypeDefinition = NodeTypeDefinition {
    type_id: "animation/get_sprite_frame",
    display_name: "Get Sprite Frame",
    category: "Animation",
    description: "Get the current frame of a sprite sheet",
    create_pins: || vec![
        Pin::input("entity", "Entity", PinType::Entity),
        Pin::output("frame", "Frame", PinType::Int),
        Pin::output("total", "Total Frames", PinType::Int),
    ],
    color: [100, 180, 160],
    is_event: false,
    is_comment: false,
};
