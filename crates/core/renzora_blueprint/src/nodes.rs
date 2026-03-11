//! Blueprint node type definitions and registry.
//!
//! Each node type declares its pins, category, and display info.
//! The interpreter uses node_type strings to dispatch execution.

use crate::graph::{BlueprintNodeDef, PinTemplate, PinType, PinValue};

// ── Category constants ──────────────────────────────────────────────────────

pub const CAT_EVENT: &str = "Event";
pub const CAT_FLOW: &str = "Flow";
pub const CAT_MATH: &str = "Math";
pub const CAT_TRANSFORM: &str = "Transform";
pub const CAT_INPUT: &str = "Input";
pub const CAT_ENTITY: &str = "Entity";
pub const CAT_COMPONENT: &str = "Component";
pub const CAT_PHYSICS: &str = "Physics";
pub const CAT_AUDIO: &str = "Audio";
pub const CAT_UI: &str = "UI";
pub const CAT_SCENE: &str = "Scene";
pub const CAT_DEBUG: &str = "Debug";
pub const CAT_VARIABLE: &str = "Variable";
pub const CAT_RENDERING: &str = "Rendering";
pub const CAT_ANIMATION: &str = "Animation";

// ── Color constants for categories ──────────────────────────────────────────

const CLR_EVENT: [u8; 3] = [200, 60, 60];
const CLR_FLOW: [u8; 3] = [140, 140, 160];
const CLR_MATH: [u8; 3] = [120, 120, 120];
const CLR_TRANSFORM: [u8; 3] = [100, 150, 220];
const CLR_INPUT: [u8; 3] = [127, 204, 25];
const CLR_ENTITY: [u8; 3] = [80, 200, 180];
const CLR_COMPONENT: [u8; 3] = [160, 100, 200];
const CLR_PHYSICS: [u8; 3] = [220, 170, 80];
const CLR_AUDIO: [u8; 3] = [200, 100, 150];
const CLR_UI: [u8; 3] = [100, 180, 220];
const CLR_SCENE: [u8; 3] = [180, 140, 100];
const CLR_DEBUG: [u8; 3] = [180, 180, 80];
const CLR_VARIABLE: [u8; 3] = [60, 180, 120];
const CLR_RENDERING: [u8; 3] = [200, 150, 120];
const CLR_ANIMATION: [u8; 3] = [80, 200, 180];

// =============================================================================
// EVENT NODES — entry points, no exec input
// =============================================================================

pub static ON_READY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_ready",
    display_name: "On Ready",
    category: CAT_EVENT,
    description: "Fires once when the entity is first initialized",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
    ],
    color: CLR_EVENT,
};

pub static ON_UPDATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_update",
    display_name: "On Update",
    category: CAT_EVENT,
    description: "Fires every frame",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
        PinTemplate::output("delta", "Delta Time", PinType::Float),
        PinTemplate::output("elapsed", "Elapsed", PinType::Float),
    ],
    color: CLR_EVENT,
};

pub static ON_COLLISION_ENTER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_collision_enter",
    display_name: "On Collision Enter",
    category: CAT_EVENT,
    description: "Fires when this entity starts colliding with another",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
        PinTemplate::output("other", "Other Entity", PinType::Entity),
    ],
    color: CLR_EVENT,
};

pub static ON_COLLISION_EXIT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_collision_exit",
    display_name: "On Collision Exit",
    category: CAT_EVENT,
    description: "Fires when a collision ends",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
        PinTemplate::output("other", "Other Entity", PinType::Entity),
    ],
    color: CLR_EVENT,
};

pub static ON_TIMER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_timer",
    display_name: "On Timer",
    category: CAT_EVENT,
    description: "Fires when a named timer completes",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
        PinTemplate::input("timer_name", "Timer Name", PinType::String)
            .with_default(PinValue::String("my_timer".into())),
    ],
    color: CLR_EVENT,
};

pub static ON_MESSAGE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "event/on_message",
    display_name: "On Message",
    category: CAT_EVENT,
    description: "Fires when a message is received (from UI, scripts, or other blueprints)",
    pins: || vec![
        PinTemplate::exec_out("exec", ""),
        PinTemplate::input("message", "Message", PinType::String)
            .with_default(PinValue::String("my_message".into())),
    ],
    color: CLR_EVENT,
};

// =============================================================================
// FLOW CONTROL NODES
// =============================================================================

pub static BRANCH: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/branch",
    display_name: "Branch",
    category: CAT_FLOW,
    description: "If/else — routes execution based on a condition",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("condition", "Condition", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("true", "True"),
        PinTemplate::exec_out("false", "False"),
    ],
    color: CLR_FLOW,
};

pub static SEQUENCE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/sequence",
    display_name: "Sequence",
    category: CAT_FLOW,
    description: "Executes outputs in order (Then 0, Then 1, ...)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_out("then_0", "Then 0"),
        PinTemplate::exec_out("then_1", "Then 1"),
        PinTemplate::exec_out("then_2", "Then 2"),
    ],
    color: CLR_FLOW,
};

pub static DO_ONCE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/do_once",
    display_name: "Do Once",
    category: CAT_FLOW,
    description: "Executes only the first time, ignores subsequent triggers",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_in("reset", "Reset"),
        PinTemplate::exec_out("completed", "Completed"),
    ],
    color: CLR_FLOW,
};

pub static FLIP_FLOP: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/flip_flop",
    display_name: "Flip Flop",
    category: CAT_FLOW,
    description: "Alternates between A and B each time triggered",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_out("a", "A"),
        PinTemplate::exec_out("b", "B"),
        PinTemplate::output("is_a", "Is A", PinType::Bool),
    ],
    color: CLR_FLOW,
};

pub static GATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/gate",
    display_name: "Gate",
    category: CAT_FLOW,
    description: "Only passes execution when the gate is open",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::exec_in("open", "Open"),
        PinTemplate::exec_in("close", "Close"),
        PinTemplate::exec_in("toggle", "Toggle"),
        PinTemplate::input("start_open", "Start Open", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("exit", "Exit"),
    ],
    color: CLR_FLOW,
};

pub static DELAY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/delay",
    display_name: "Delay",
    category: CAT_FLOW,
    description: "Waits for a duration before continuing execution",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("duration", "Duration", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::exec_out("completed", "Completed"),
    ],
    color: CLR_FLOW,
};

// =============================================================================
// MATH NODES — pure data, no exec pins
// =============================================================================

pub static ADD: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/add",
    display_name: "Add",
    category: CAT_MATH,
    description: "A + B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SUBTRACT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/subtract",
    display_name: "Subtract",
    category: CAT_MATH,
    description: "A - B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static MULTIPLY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/multiply",
    display_name: "Multiply",
    category: CAT_MATH,
    description: "A * B",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static DIVIDE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/divide",
    display_name: "Divide",
    category: CAT_MATH,
    description: "A / B (safe — returns 0 if B is 0)",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static NEGATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/negate",
    display_name: "Negate",
    category: CAT_MATH,
    description: "-Value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static ABS: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/abs",
    display_name: "Abs",
    category: CAT_MATH,
    description: "Absolute value",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static CLAMP: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/clamp",
    display_name: "Clamp",
    category: CAT_MATH,
    description: "Clamp value between min and max",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static LERP: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/lerp",
    display_name: "Lerp",
    category: CAT_MATH,
    description: "Linear interpolation: mix(A, B, T)",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::input("t", "T", PinType::Float).with_default(PinValue::Float(0.5)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static RANDOM_RANGE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/random_range",
    display_name: "Random Range",
    category: CAT_MATH,
    description: "Random float between min and max",
    pins: || vec![
        PinTemplate::input("min", "Min", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("max", "Max", PinType::Float).with_default(PinValue::Float(1.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static SIN: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/sin",
    display_name: "Sin",
    category: CAT_MATH,
    description: "Sine function",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static COS: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/cos",
    display_name: "Cos",
    category: CAT_MATH,
    description: "Cosine function",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Float),
    ],
    color: CLR_MATH,
};

pub static COMPARE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/compare",
    display_name: "Compare",
    category: CAT_MATH,
    description: "A > B, A < B, A == B comparisons",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("b", "B", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("greater", "A > B", PinType::Bool),
        PinTemplate::output("less", "A < B", PinType::Bool),
        PinTemplate::output("equal", "A == B", PinType::Bool),
    ],
    color: CLR_MATH,
};

pub static AND: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/and",
    display_name: "AND",
    category: CAT_MATH,
    description: "Logical AND",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

pub static OR: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/or",
    display_name: "OR",
    category: CAT_MATH,
    description: "Logical OR",
    pins: || vec![
        PinTemplate::input("a", "A", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::input("b", "B", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

pub static NOT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/not",
    display_name: "NOT",
    category: CAT_MATH,
    description: "Logical NOT",
    pins: || vec![
        PinTemplate::input("value", "Value", PinType::Bool).with_default(PinValue::Bool(false)),
        PinTemplate::output("result", "Result", PinType::Bool),
    ],
    color: CLR_MATH,
};

pub static COMBINE_VEC3: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/combine_vec3",
    display_name: "Combine Vec3",
    category: CAT_MATH,
    description: "Create Vec3 from X, Y, Z",
    pins: || vec![
        PinTemplate::input("x", "X", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("y", "Y", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::input("z", "Z", PinType::Float).with_default(PinValue::Float(0.0)),
        PinTemplate::output("result", "Result", PinType::Vec3),
    ],
    color: CLR_MATH,
};

pub static SPLIT_VEC3: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "math/split_vec3",
    display_name: "Split Vec3",
    category: CAT_MATH,
    description: "Extract X, Y, Z from Vec3",
    pins: || vec![
        PinTemplate::input("vector", "Vector", PinType::Vec3).with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_MATH,
};

// =============================================================================
// TRANSFORM NODES
// =============================================================================

pub static GET_POSITION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/get_position",
    display_name: "Get Position",
    category: CAT_TRANSFORM,
    description: "Get this entity's world position",
    pins: || vec![
        PinTemplate::output("position", "Position", PinType::Vec3),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_TRANSFORM,
};

pub static SET_POSITION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/set_position",
    display_name: "Set Position",
    category: CAT_TRANSFORM,
    description: "Set this entity's position",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("position", "Position", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static TRANSLATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/translate",
    display_name: "Translate",
    category: CAT_TRANSFORM,
    description: "Move this entity by an offset",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("offset", "Offset", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static GET_ROTATION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/get_rotation",
    display_name: "Get Rotation",
    category: CAT_TRANSFORM,
    description: "Get this entity's rotation (euler degrees)",
    pins: || vec![
        PinTemplate::output("rotation", "Rotation", PinType::Vec3),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
        PinTemplate::output("z", "Z", PinType::Float),
    ],
    color: CLR_TRANSFORM,
};

pub static SET_ROTATION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/set_rotation",
    display_name: "Set Rotation",
    category: CAT_TRANSFORM,
    description: "Set this entity's rotation (euler degrees)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("rotation", "Rotation", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static ROTATE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/rotate",
    display_name: "Rotate",
    category: CAT_TRANSFORM,
    description: "Rotate this entity by degrees",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("degrees", "Degrees", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static LOOK_AT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/look_at",
    display_name: "Look At",
    category: CAT_TRANSFORM,
    description: "Rotate to face a target position",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("target", "Target", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static SET_SCALE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/set_scale",
    display_name: "Set Scale",
    category: CAT_TRANSFORM,
    description: "Set this entity's scale",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("scale", "Scale", PinType::Vec3)
            .with_default(PinValue::Vec3([1.0, 1.0, 1.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_TRANSFORM,
};

pub static GET_FORWARD: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "transform/get_forward",
    display_name: "Get Forward",
    category: CAT_TRANSFORM,
    description: "Get the entity's forward direction vector",
    pins: || vec![
        PinTemplate::output("forward", "Forward", PinType::Vec3),
        PinTemplate::output("right", "Right", PinType::Vec3),
        PinTemplate::output("up", "Up", PinType::Vec3),
    ],
    color: CLR_TRANSFORM,
};

// =============================================================================
// INPUT NODES
// =============================================================================

pub static GET_MOVEMENT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/get_movement",
    display_name: "Get Movement",
    category: CAT_INPUT,
    description: "WASD/Arrow movement vector (normalized)",
    pins: || vec![
        PinTemplate::output("movement", "Movement", PinType::Vec2),
        PinTemplate::output("x", "X", PinType::Float),
        PinTemplate::output("y", "Y", PinType::Float),
    ],
    color: CLR_INPUT,
};

pub static IS_KEY_PRESSED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/is_key_pressed",
    display_name: "Is Key Pressed",
    category: CAT_INPUT,
    description: "Check if a key is currently held down",
    pins: || vec![
        PinTemplate::input("key", "Key", PinType::String)
            .with_default(PinValue::String("Space".into())),
        PinTemplate::output("pressed", "Pressed", PinType::Bool),
    ],
    color: CLR_INPUT,
};

pub static IS_KEY_JUST_PRESSED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/is_key_just_pressed",
    display_name: "Is Key Just Pressed",
    category: CAT_INPUT,
    description: "Check if a key was pressed this frame",
    pins: || vec![
        PinTemplate::input("key", "Key", PinType::String)
            .with_default(PinValue::String("Space".into())),
        PinTemplate::output("pressed", "Pressed", PinType::Bool),
    ],
    color: CLR_INPUT,
};

pub static GET_MOUSE_POSITION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/get_mouse_position",
    display_name: "Get Mouse Position",
    category: CAT_INPUT,
    description: "Current mouse cursor position",
    pins: || vec![
        PinTemplate::output("position", "Position", PinType::Vec2),
        PinTemplate::output("delta", "Delta", PinType::Vec2),
    ],
    color: CLR_INPUT,
};

pub static IS_MOUSE_PRESSED: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/is_mouse_pressed",
    display_name: "Is Mouse Pressed",
    category: CAT_INPUT,
    description: "Check if a mouse button is pressed (0=left, 1=right, 2=middle)",
    pins: || vec![
        PinTemplate::input("button", "Button", PinType::Int)
            .with_default(PinValue::Int(0)),
        PinTemplate::output("pressed", "Pressed", PinType::Bool),
    ],
    color: CLR_INPUT,
};

pub static GET_GAMEPAD: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "input/get_gamepad",
    display_name: "Get Gamepad",
    category: CAT_INPUT,
    description: "Gamepad stick and trigger values",
    pins: || vec![
        PinTemplate::output("left_stick", "Left Stick", PinType::Vec2),
        PinTemplate::output("right_stick", "Right Stick", PinType::Vec2),
        PinTemplate::output("left_trigger", "Left Trigger", PinType::Float),
        PinTemplate::output("right_trigger", "Right Trigger", PinType::Float),
    ],
    color: CLR_INPUT,
};

// =============================================================================
// ENTITY NODES
// =============================================================================

pub static GET_SELF: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "entity/get_self",
    display_name: "Get Self",
    category: CAT_ENTITY,
    description: "Reference to this entity",
    pins: || vec![
        PinTemplate::output("entity", "Self", PinType::Entity),
        PinTemplate::output("name", "Name", PinType::String),
    ],
    color: CLR_ENTITY,
};

pub static GET_ENTITY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "entity/get_entity",
    display_name: "Get Entity",
    category: CAT_ENTITY,
    description: "Find an entity by name or tag",
    pins: || vec![
        PinTemplate::input("name", "Name", PinType::String)
            .with_default(PinValue::String(String::new())),
        PinTemplate::output("entity", "Entity", PinType::Entity),
        PinTemplate::output("found", "Found", PinType::Bool),
    ],
    color: CLR_ENTITY,
};

pub static SPAWN_ENTITY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "entity/spawn",
    display_name: "Spawn Entity",
    category: CAT_ENTITY,
    description: "Spawn a new entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("name", "Name", PinType::String)
            .with_default(PinValue::String("New Entity".into())),
        PinTemplate::exec_out("then", ""),
        PinTemplate::output("entity", "Entity", PinType::Entity),
    ],
    color: CLR_ENTITY,
};

pub static DESPAWN_ENTITY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "entity/despawn",
    display_name: "Despawn Entity",
    category: CAT_ENTITY,
    description: "Destroy an entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("entity", "Entity", PinType::Entity),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_ENTITY,
};

pub static DESPAWN_SELF: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "entity/despawn_self",
    display_name: "Despawn Self",
    category: CAT_ENTITY,
    description: "Destroy this entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
    ],
    color: CLR_ENTITY,
};

// =============================================================================
// COMPONENT NODES — reflection-based, work with any registered component
// =============================================================================

pub static GET_COMPONENT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "component/get_field",
    display_name: "Get Component Field",
    category: CAT_COMPONENT,
    description: "Read a field from any component (via reflection)",
    pins: || vec![
        PinTemplate::input("entity", "Entity", PinType::Entity),
        PinTemplate::input("component", "Component", PinType::String)
            .with_default(PinValue::String("Transform".into())),
        PinTemplate::input("field", "Field", PinType::String)
            .with_default(PinValue::String("translation".into())),
        PinTemplate::output("value", "Value", PinType::Any),
    ],
    color: CLR_COMPONENT,
};

pub static SET_COMPONENT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "component/set_field",
    display_name: "Set Component Field",
    category: CAT_COMPONENT,
    description: "Write a field on any component (via reflection)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("entity", "Entity", PinType::Entity),
        PinTemplate::input("component", "Component", PinType::String)
            .with_default(PinValue::String("Transform".into())),
        PinTemplate::input("field", "Field", PinType::String)
            .with_default(PinValue::String("translation".into())),
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_COMPONENT,
};

// =============================================================================
// PHYSICS NODES
// =============================================================================

pub static APPLY_FORCE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "physics/apply_force",
    display_name: "Apply Force",
    category: CAT_PHYSICS,
    description: "Apply a force to this entity's rigidbody",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("force", "Force", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_PHYSICS,
};

pub static APPLY_IMPULSE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "physics/apply_impulse",
    display_name: "Apply Impulse",
    category: CAT_PHYSICS,
    description: "Apply an instant impulse to this entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("impulse", "Impulse", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 10.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_PHYSICS,
};

pub static SET_VELOCITY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "physics/set_velocity",
    display_name: "Set Velocity",
    category: CAT_PHYSICS,
    description: "Set this entity's linear velocity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("velocity", "Velocity", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, 0.0, 0.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_PHYSICS,
};

pub static RAYCAST: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "physics/raycast",
    display_name: "Raycast",
    category: CAT_PHYSICS,
    description: "Cast a ray and check for hits",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("origin", "Origin", PinType::Vec3),
        PinTemplate::input("direction", "Direction", PinType::Vec3)
            .with_default(PinValue::Vec3([0.0, -1.0, 0.0])),
        PinTemplate::input("max_distance", "Max Distance", PinType::Float)
            .with_default(PinValue::Float(100.0)),
        PinTemplate::exec_out("hit", "Hit"),
        PinTemplate::exec_out("miss", "Miss"),
        PinTemplate::output("point", "Hit Point", PinType::Vec3),
        PinTemplate::output("normal", "Hit Normal", PinType::Vec3),
        PinTemplate::output("entity", "Hit Entity", PinType::Entity),
        PinTemplate::output("distance", "Distance", PinType::Float),
    ],
    color: CLR_PHYSICS,
};

// =============================================================================
// AUDIO NODES
// =============================================================================

pub static PLAY_SOUND: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "audio/play_sound",
    display_name: "Play Sound",
    category: CAT_AUDIO,
    description: "Play a sound effect",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("path", "Path", PinType::String)
            .with_default(PinValue::String("sounds/click.ogg".into())),
        PinTemplate::input("volume", "Volume", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::input("looping", "Loop", PinType::Bool)
            .with_default(PinValue::Bool(false)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_AUDIO,
};

pub static PLAY_MUSIC: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "audio/play_music",
    display_name: "Play Music",
    category: CAT_AUDIO,
    description: "Play background music (with crossfade)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("path", "Path", PinType::String)
            .with_default(PinValue::String("music/theme.ogg".into())),
        PinTemplate::input("volume", "Volume", PinType::Float)
            .with_default(PinValue::Float(0.8)),
        PinTemplate::input("fade_in", "Fade In", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_AUDIO,
};

pub static STOP_MUSIC: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "audio/stop_music",
    display_name: "Stop Music",
    category: CAT_AUDIO,
    description: "Stop background music",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("fade_out", "Fade Out", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_AUDIO,
};

// =============================================================================
// UI NODES
// =============================================================================

pub static SHOW_UI: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/show",
    display_name: "Show UI",
    category: CAT_UI,
    description: "Load and display a UI scene",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("path", "Path", PinType::String)
            .with_default(PinValue::String("ui/main_menu.ui".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static HIDE_UI: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/hide",
    display_name: "Hide UI",
    category: CAT_UI,
    description: "Remove a UI scene from the screen",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("path", "Path", PinType::String)
            .with_default(PinValue::String("ui/main_menu.ui".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_TEXT: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_text",
    display_name: "Set UI Text",
    category: CAT_UI,
    description: "Update text content of a named UI element",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("text", "Text", PinType::String),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_PROGRESS: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_progress",
    display_name: "Set UI Progress",
    category: CAT_UI,
    description: "Update a progress bar (0.0 to 1.0)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("value", "Value", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_HEALTH: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_health",
    display_name: "Set UI Health",
    category: CAT_UI,
    description: "Update a health bar's current and max values",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("current", "Current", PinType::Float)
            .with_default(PinValue::Float(75.0)),
        PinTemplate::input("max", "Max", PinType::Float)
            .with_default(PinValue::Float(100.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_SLIDER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_slider",
    display_name: "Set UI Slider",
    category: CAT_UI,
    description: "Set a slider's value",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("value", "Value", PinType::Float)
            .with_default(PinValue::Float(0.5)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_CHECKBOX: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_checkbox",
    display_name: "Set UI Checkbox",
    category: CAT_UI,
    description: "Set a checkbox's checked state",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("checked", "Checked", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_TOGGLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_toggle",
    display_name: "Set UI Toggle",
    category: CAT_UI,
    description: "Set a toggle switch's on/off state",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("on", "On", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_VISIBLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_visible",
    display_name: "Set UI Visible",
    category: CAT_UI,
    description: "Show or hide a named UI widget",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("visible", "Visible", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static TOGGLE_UI: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/toggle",
    display_name: "Toggle UI",
    category: CAT_UI,
    description: "Toggle a UI canvas visibility on/off",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("name", "Canvas Name", PinType::String),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_THEME: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_theme",
    display_name: "Set UI Theme",
    category: CAT_UI,
    description: "Switch the active UI theme (dark, light, high_contrast)",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("theme", "Theme", PinType::String)
            .with_default(PinValue::String("dark".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

pub static SET_UI_COLOR: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "ui/set_color",
    display_name: "Set UI Color",
    category: CAT_UI,
    description: "Set the background color of a named UI widget",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("element", "Element Name", PinType::String),
        PinTemplate::input("color", "Color", PinType::Color)
            .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_UI,
};

// =============================================================================
// SCENE NODES
// =============================================================================

pub static LOAD_SCENE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "scene/load",
    display_name: "Load Scene",
    category: CAT_SCENE,
    description: "Load a new 3D scene",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("path", "Path", PinType::String)
            .with_default(PinValue::String("scenes/main.ron".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_SCENE,
};

// =============================================================================
// DEBUG NODES
// =============================================================================

pub static LOG: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "debug/log",
    display_name: "Log",
    category: CAT_DEBUG,
    description: "Print a message to the console",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("message", "Message", PinType::String)
            .with_default(PinValue::String("Hello!".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_DEBUG,
};

pub static DRAW_LINE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "debug/draw_line",
    display_name: "Draw Line",
    category: CAT_DEBUG,
    description: "Draw a debug line in the viewport",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("start", "Start", PinType::Vec3),
        PinTemplate::input("end", "End", PinType::Vec3),
        PinTemplate::input("color", "Color", PinType::Color)
            .with_default(PinValue::Color([1.0, 0.0, 0.0, 1.0])),
        PinTemplate::input("duration", "Duration", PinType::Float)
            .with_default(PinValue::Float(0.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_DEBUG,
};

// =============================================================================
// VARIABLE NODES
// =============================================================================

pub static GET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "variable/get",
    display_name: "Get Variable",
    category: CAT_VARIABLE,
    description: "Read a script variable from this entity",
    pins: || vec![
        PinTemplate::input("name", "Name", PinType::String)
            .with_default(PinValue::String("my_var".into())),
        PinTemplate::output("value", "Value", PinType::Any),
    ],
    color: CLR_VARIABLE,
};

pub static SET_VARIABLE: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "variable/set",
    display_name: "Set Variable",
    category: CAT_VARIABLE,
    description: "Write a script variable on this entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("name", "Name", PinType::String)
            .with_default(PinValue::String("my_var".into())),
        PinTemplate::input("value", "Value", PinType::Any),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_VARIABLE,
};

// =============================================================================
// RENDERING NODES
// =============================================================================

pub static SET_VISIBILITY: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "rendering/set_visibility",
    display_name: "Set Visibility",
    category: CAT_RENDERING,
    description: "Show or hide an entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("visible", "Visible", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_RENDERING,
};

pub static SET_MATERIAL_COLOR: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "rendering/set_material_color",
    display_name: "Set Material Color",
    category: CAT_RENDERING,
    description: "Change this entity's material base color",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("color", "Color", PinType::Color)
            .with_default(PinValue::Color([1.0, 1.0, 1.0, 1.0])),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_RENDERING,
};

// =============================================================================
// ANIMATION NODES
// =============================================================================

pub static PLAY_ANIMATION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "animation/play",
    display_name: "Play Animation",
    category: CAT_ANIMATION,
    description: "Play an animation on this entity",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("name", "Name", PinType::String),
        PinTemplate::input("looping", "Loop", PinType::Bool)
            .with_default(PinValue::Bool(true)),
        PinTemplate::input("speed", "Speed", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_ANIMATION,
};

pub static TWEEN_POSITION: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "animation/tween_position",
    display_name: "Tween Position",
    category: CAT_ANIMATION,
    description: "Smoothly move entity to a target position",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("target", "Target", PinType::Vec3),
        PinTemplate::input("duration", "Duration", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::input("easing", "Easing", PinType::String)
            .with_default(PinValue::String("ease_in_out".into())),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_ANIMATION,
};

// =============================================================================
// TIMER NODES
// =============================================================================

pub static START_TIMER: BlueprintNodeDef = BlueprintNodeDef {
    node_type: "flow/start_timer",
    display_name: "Start Timer",
    category: CAT_FLOW,
    description: "Start a named timer",
    pins: || vec![
        PinTemplate::exec_in("exec", ""),
        PinTemplate::input("name", "Name", PinType::String)
            .with_default(PinValue::String("my_timer".into())),
        PinTemplate::input("duration", "Duration", PinType::Float)
            .with_default(PinValue::Float(1.0)),
        PinTemplate::input("repeat", "Repeat", PinType::Bool)
            .with_default(PinValue::Bool(false)),
        PinTemplate::exec_out("then", ""),
    ],
    color: CLR_FLOW,
};

// =============================================================================
// REGISTRY
// =============================================================================

/// All available blueprint node types.
pub static ALL_NODES: &[&BlueprintNodeDef] = &[
    // Event
    &ON_READY, &ON_UPDATE, &ON_COLLISION_ENTER, &ON_COLLISION_EXIT,
    &ON_TIMER, &ON_MESSAGE,
    // Flow
    &BRANCH, &SEQUENCE, &DO_ONCE, &FLIP_FLOP, &GATE, &DELAY, &START_TIMER,
    // Math
    &ADD, &SUBTRACT, &MULTIPLY, &DIVIDE, &NEGATE, &ABS, &CLAMP, &LERP,
    &RANDOM_RANGE, &SIN, &COS, &COMPARE, &AND, &OR, &NOT,
    &COMBINE_VEC3, &SPLIT_VEC3,
    // Transform
    &GET_POSITION, &SET_POSITION, &TRANSLATE,
    &GET_ROTATION, &SET_ROTATION, &ROTATE, &LOOK_AT,
    &SET_SCALE, &GET_FORWARD,
    // Input
    &GET_MOVEMENT, &IS_KEY_PRESSED, &IS_KEY_JUST_PRESSED,
    &GET_MOUSE_POSITION, &IS_MOUSE_PRESSED, &GET_GAMEPAD,
    // Entity
    &GET_SELF, &GET_ENTITY, &SPAWN_ENTITY, &DESPAWN_ENTITY, &DESPAWN_SELF,
    // Component
    &GET_COMPONENT, &SET_COMPONENT,
    // Physics
    &APPLY_FORCE, &APPLY_IMPULSE, &SET_VELOCITY, &RAYCAST,
    // Audio
    &PLAY_SOUND, &PLAY_MUSIC, &STOP_MUSIC,
    // UI
    &SHOW_UI, &HIDE_UI, &TOGGLE_UI, &SET_UI_TEXT, &SET_UI_PROGRESS,
    &SET_UI_HEALTH, &SET_UI_SLIDER, &SET_UI_CHECKBOX, &SET_UI_TOGGLE,
    &SET_UI_VISIBLE, &SET_UI_THEME, &SET_UI_COLOR,
    // Scene
    &LOAD_SCENE,
    // Debug
    &LOG, &DRAW_LINE,
    // Variable
    &GET_VARIABLE, &SET_VARIABLE,
    // Rendering
    &SET_VISIBILITY, &SET_MATERIAL_COLOR,
    // Animation
    &PLAY_ANIMATION, &TWEEN_POSITION,
];

/// Get all unique categories in display order.
pub fn categories() -> Vec<&'static str> {
    vec![
        CAT_EVENT, CAT_FLOW, CAT_MATH, CAT_TRANSFORM, CAT_INPUT,
        CAT_ENTITY, CAT_COMPONENT, CAT_PHYSICS, CAT_AUDIO, CAT_UI,
        CAT_SCENE, CAT_VARIABLE, CAT_RENDERING, CAT_ANIMATION, CAT_DEBUG,
    ]
}

/// Get all node definitions in a category.
pub fn nodes_in_category(category: &str) -> Vec<&'static BlueprintNodeDef> {
    ALL_NODES
        .iter()
        .copied()
        .filter(|n| n.category == category)
        .collect()
}

/// Look up a node definition by type string.
pub fn node_def(node_type: &str) -> Option<&'static BlueprintNodeDef> {
    ALL_NODES.iter().copied().find(|n| n.node_type == node_type)
}
