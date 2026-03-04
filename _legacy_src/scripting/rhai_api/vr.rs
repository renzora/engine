//! Rhai VR API functions
//!
//! Provides script access to VR state: head/controller tracking, input,
//! haptics, and VR-specific commands. All functions return safe defaults
//! when not in VR mode.

use rhai::{Dynamic, Engine, Map, ImmutableString};
use std::cell::RefCell;

use super::push_command;
use crate::scripting::rhai_commands::RhaiCommand;

// ============================================================================
// Thread-local VR command queue (drained by VR system after script execution)
// ============================================================================

thread_local! {
    /// Buffer for VR commands produced during script execution.
    /// Drained by the VR processing system each frame.
    static VR_COMMAND_BUFFER: RefCell<Vec<RhaiCommand>> = RefCell::new(Vec::new());
}

/// Push a VR command into the thread-local queue (called from runtime.rs)
pub fn push_vr_command(cmd: RhaiCommand) {
    VR_COMMAND_BUFFER.with(|buf| buf.borrow_mut().push(cmd));
}

/// Drain all buffered VR commands (called by the VR processing system)
pub fn drain_vr_commands() -> Vec<RhaiCommand> {
    VR_COMMAND_BUFFER.with(|buf| buf.borrow_mut().drain(..).collect())
}

// ============================================================================
// Thread-local VR state (populated per-frame before script execution)
// ============================================================================

thread_local! {
    /// Whether VR mode is currently active
    static VR_ACTIVE: RefCell<bool> = RefCell::new(false);

    /// Head position (x, y, z)
    static VR_HEAD_POSITION: RefCell<[f64; 3]> = RefCell::new([0.0; 3]);

    /// Head rotation quaternion (x, y, z, w)
    static VR_HEAD_ROTATION: RefCell<[f64; 4]> = RefCell::new([0.0, 0.0, 0.0, 1.0]);

    /// Left controller state
    static VR_LEFT_CONTROLLER: RefCell<ControllerSnapshot> = RefCell::new(ControllerSnapshot::default());

    /// Right controller state
    static VR_RIGHT_CONTROLLER: RefCell<ControllerSnapshot> = RefCell::new(ControllerSnapshot::default());

    /// Left hand tracking state
    static VR_LEFT_HAND: RefCell<HandSnapshot> = RefCell::new(HandSnapshot::default());

    /// Right hand tracking state
    static VR_RIGHT_HAND: RefCell<HandSnapshot> = RefCell::new(HandSnapshot::default());

    /// Current VR session status string
    static VR_SESSION_STATUS: RefCell<String> = RefCell::new(String::new());

    /// Connected headset name
    static VR_HEADSET_NAME: RefCell<String> = RefCell::new(String::new());
}

/// Snapshot of a single controller's state for script access
#[derive(Clone, Default)]
pub struct ControllerSnapshot {
    pub position: [f64; 3],
    pub rotation: [f64; 4],
    pub trigger: f64,
    pub trigger_pressed: bool,
    pub grip: f64,
    pub grip_pressed: bool,
    pub thumbstick_x: f64,
    pub thumbstick_y: f64,
    pub button_a: bool,
    pub button_b: bool,
    pub tracked: bool,
}

/// Snapshot of hand tracking state for script access
#[derive(Clone, Default)]
pub struct HandSnapshot {
    pub pinch_strength: f64,
    pub grab_strength: f64,
    pub tracked: bool,
}

// ============================================================================
// Population functions (called by the script runner before script execution)
// ============================================================================

pub fn set_vr_active(active: bool) {
    VR_ACTIVE.with(|v| *v.borrow_mut() = active);
}

pub fn set_vr_head_pose(pos: [f64; 3], rot: [f64; 4]) {
    VR_HEAD_POSITION.with(|v| *v.borrow_mut() = pos);
    VR_HEAD_ROTATION.with(|v| *v.borrow_mut() = rot);
}

pub fn set_vr_controller(hand: &str, snapshot: ControllerSnapshot) {
    match hand {
        "right" => VR_RIGHT_CONTROLLER.with(|v| *v.borrow_mut() = snapshot),
        _ => VR_LEFT_CONTROLLER.with(|v| *v.borrow_mut() = snapshot),
    }
}

pub fn set_vr_hand(hand: &str, snapshot: HandSnapshot) {
    match hand {
        "right" => VR_RIGHT_HAND.with(|v| *v.borrow_mut() = snapshot),
        _ => VR_LEFT_HAND.with(|v| *v.borrow_mut() = snapshot),
    }
}

pub fn set_vr_session_status(status: &str) {
    VR_SESSION_STATUS.with(|v| *v.borrow_mut() = status.to_string());
}

pub fn set_vr_headset_name(name: &str) {
    VR_HEADSET_NAME.with(|v| *v.borrow_mut() = name.to_string());
}

fn get_controller(hand: &str) -> ControllerSnapshot {
    match hand {
        "right" => VR_RIGHT_CONTROLLER.with(|v| v.borrow().clone()),
        _ => VR_LEFT_CONTROLLER.with(|v| v.borrow().clone()),
    }
}

fn get_hand(hand: &str) -> HandSnapshot {
    match hand {
        "right" => VR_RIGHT_HAND.with(|v| v.borrow().clone()),
        _ => VR_LEFT_HAND.with(|v| v.borrow().clone()),
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(engine: &mut Engine) {
    // ---- Status ----

    engine.register_fn("is_vr_active", || -> bool {
        VR_ACTIVE.with(|v| *v.borrow())
    });

    // ---- Head Tracking ----

    engine.register_fn("get_vr_head_position", || -> Map {
        let pos = VR_HEAD_POSITION.with(|v| *v.borrow());
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(pos[0]));
        m.insert("y".into(), Dynamic::from(pos[1]));
        m.insert("z".into(), Dynamic::from(pos[2]));
        m
    });

    engine.register_fn("get_vr_head_rotation", || -> Map {
        let rot = VR_HEAD_ROTATION.with(|v| *v.borrow());
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(rot[0]));
        m.insert("y".into(), Dynamic::from(rot[1]));
        m.insert("z".into(), Dynamic::from(rot[2]));
        m.insert("w".into(), Dynamic::from(rot[3]));
        m
    });

    // ---- Controller Input ----

    engine.register_fn("get_vr_controller_position", |hand: ImmutableString| -> Map {
        let ctrl = get_controller(hand.as_str());
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(ctrl.position[0]));
        m.insert("y".into(), Dynamic::from(ctrl.position[1]));
        m.insert("z".into(), Dynamic::from(ctrl.position[2]));
        m
    });

    engine.register_fn("get_vr_controller_rotation", |hand: ImmutableString| -> Map {
        let ctrl = get_controller(hand.as_str());
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(ctrl.rotation[0]));
        m.insert("y".into(), Dynamic::from(ctrl.rotation[1]));
        m.insert("z".into(), Dynamic::from(ctrl.rotation[2]));
        m.insert("w".into(), Dynamic::from(ctrl.rotation[3]));
        m
    });

    engine.register_fn("get_vr_trigger", |hand: ImmutableString| -> f64 {
        get_controller(hand.as_str()).trigger
    });

    engine.register_fn("is_vr_trigger_pressed", |hand: ImmutableString| -> bool {
        get_controller(hand.as_str()).trigger_pressed
    });

    engine.register_fn("get_vr_grip", |hand: ImmutableString| -> f64 {
        get_controller(hand.as_str()).grip
    });

    engine.register_fn("is_vr_grip_pressed", |hand: ImmutableString| -> bool {
        get_controller(hand.as_str()).grip_pressed
    });

    engine.register_fn("get_vr_thumbstick", |hand: ImmutableString| -> Map {
        let ctrl = get_controller(hand.as_str());
        let mut m = Map::new();
        m.insert("x".into(), Dynamic::from(ctrl.thumbstick_x));
        m.insert("y".into(), Dynamic::from(ctrl.thumbstick_y));
        m
    });

    engine.register_fn("is_vr_button_pressed", |hand: ImmutableString, button: ImmutableString| -> bool {
        let ctrl = get_controller(hand.as_str());
        match button.as_str() {
            "a" | "x" => ctrl.button_a,
            "b" | "y" => ctrl.button_b,
            "trigger" => ctrl.trigger_pressed,
            "grip" => ctrl.grip_pressed,
            _ => false,
        }
    });

    // ---- Hand Tracking ----

    engine.register_fn("get_hand_pinch_strength", |hand: ImmutableString| -> f64 {
        get_hand(hand.as_str()).pinch_strength
    });

    engine.register_fn("get_hand_grab_strength", |hand: ImmutableString| -> f64 {
        get_hand(hand.as_str()).grab_strength
    });

    // ---- VR Commands ----

    engine.register_fn("vr_haptic_pulse", |hand: ImmutableString, intensity: f64, duration: f64| {
        push_command(RhaiCommand::VrHapticPulse {
            hand: hand.to_string(),
            intensity: intensity as f32,
            duration: duration as f32,
        });
    });

    engine.register_fn("vr_teleport_to", |x: f64, y: f64, z: f64| {
        push_command(RhaiCommand::VrTeleportTo {
            x: x as f32,
            y: y as f32,
            z: z as f32,
        });
    });

    engine.register_fn("vr_recenter", || {
        push_command(RhaiCommand::VrRecenter);
    });

    // ---- Session Info ----

    engine.register_fn("get_vr_session_status", || -> ImmutableString {
        VR_SESSION_STATUS.with(|v| v.borrow().clone().into())
    });

    engine.register_fn("get_vr_headset_name", || -> ImmutableString {
        VR_HEADSET_NAME.with(|v| v.borrow().clone().into())
    });

    engine.register_fn("is_hand_tracked", |hand: ImmutableString| -> bool {
        get_hand(hand.as_str()).tracked
    });

    engine.register_fn("vr_set_passthrough", |enabled: bool| {
        push_command(RhaiCommand::VrSetPassthrough { enabled });
    });
}
