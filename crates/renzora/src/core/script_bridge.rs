//! The inboxes scripting reads from, and the value types that travel in them.
//!
//! Scripting must not depend on the network, UI, animation or scene crates —
//! any of those would make the language plugin the centre of the dependency
//! graph. So each producer pushes plain data into a resource here and
//! `renzora_scripting` drains it once per frame. The same indirection, repeated:
//! [`ScriptRpcInbox`], [`ScriptUiInbox`], [`ScriptSceneInbox`],
//! [`ScriptAnimEventInbox`], [`ScriptNetLifecycleInbox`], [`ScriptDrawBuffer`].

use std::collections::HashMap;

use bevy::input::gamepad::{GamepadAxis, GamepadButton};
use bevy::prelude::*;

/// Generic script action event. Scripts call `action("name", { args })` and
/// domain crates observe this event to handle actions they recognize.
/// This decouples scripting from domain crates — no ScriptExtension imports needed.
#[derive(bevy::prelude::Event, Debug, Clone)]
pub struct ScriptAction {
    /// The action name (e.g. "apply_force", "play_sound", "gauge_damage").
    pub name: String,
    /// The entity that triggered the action (script's owning entity).
    pub entity: bevy::ecs::entity::Entity,
    /// Optional target entity (by name or ID).
    pub target_entity: Option<String>,
    /// Action arguments as key-value pairs.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
}

/// Value types for script action arguments.
#[derive(Debug, Clone)]
pub enum ScriptActionValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
}

/// Value types for property writes and reflection-based get/set.
#[derive(Clone, Debug)]
pub enum PropertyValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}

/// Lightweight network status bridge — updated by the network crate,
/// read by blueprint and other crates that need connection info without
/// depending on renzora_network.
#[derive(Resource, Default, Clone, Debug)]
pub struct NetworkBridge {
    /// Whether this instance is running as a server.
    pub is_server: bool,
    /// Whether the client is connected to a server (or server is running).
    pub is_connected: bool,
    /// Number of connected clients (server only).
    pub player_count: i32,
}

/// A single networked RPC delivered to this peer, awaiting dispatch to
/// scripts' `on_rpc(name, args)` hook.
#[derive(Clone, Debug)]
pub struct IncomingRpc {
    /// RPC name the sender used (the first arg to `rpc(name, args)`).
    pub name: String,
    /// Decoded argument table.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
    /// Sender's peer id (0 = server/local).
    pub from: u64,
}

/// Inbox bridge for networked RPCs received from the wire.
///
/// `renzora_network` pushes a [`IncomingRpc`] here for every `GameEvent` it
/// receives; `renzora_scripting` drains it each frame and invokes every
/// script's `on_rpc(name, args)` hook. Lives in core because scripting must
/// not depend on the network crate (same indirection as [`NetworkBridge`]).
#[derive(Resource, Default)]
pub struct ScriptRpcInbox {
    pub pending: Vec<IncomingRpc>,
}

/// A player join/leave event, awaiting dispatch to scripts'
/// `on_player_joined(id)` / `on_player_left(id)` hooks. Server-authoritative:
/// only the server/host observes connections, so only it produces these.
#[derive(Clone, Debug)]
pub struct NetPlayerEvent {
    /// Peer id that joined or left (same id space as [`IncomingRpc::from`]).
    pub id: u64,
    /// `true` = joined, `false` = left.
    pub joined: bool,
}

/// Inbox of player lifecycle events. `renzora_network` (server side) pushes a
/// [`NetPlayerEvent`] when a client connects/disconnects; `renzora_scripting`
/// drains it each frame and invokes every script's `on_player_joined(id)` /
/// `on_player_left(id)` hook. Lives in core for the same reason as
/// [`ScriptRpcInbox`] — scripting must not depend on the network crate.
#[derive(Resource, Default)]
pub struct ScriptNetLifecycleInbox {
    pub pending: Vec<NetPlayerEvent>,
}

/// A UI markup callback awaiting dispatch to scripts' `on_ui(name, args)` hook.
///
/// Produced by `renzora_hui` when a `bevy_hui` template node fires an event
/// (e.g. `on_press="start_game"`) that has no Rust-side `HtmlFunctions`
/// binding — the name then falls through to scripts instead.
#[derive(Clone, Debug)]
pub struct UiCallback {
    /// The markup callback name (the value of `on_press` / `on_change` / …).
    pub name: String,
    /// `tag:`-prefixed markup attributes on the node, decoded as args.
    pub args: std::collections::HashMap<String, ScriptActionValue>,
    /// The UI node entity that fired the event, as raw bits (`Entity::to_bits`).
    /// Scripts receive this so they can target the originating widget.
    pub entity_bits: u64,
}

/// Inbox bridge for UI markup callbacks. `renzora_hui` pushes a [`UiCallback`]
/// when a template event fires; `renzora_scripting` drains it each frame and
/// invokes every script's `on_ui(name, args)` hook (broadcast, same semantics
/// as [`ScriptRpcInbox`]). Lives in core so scripting depends on neither
/// `renzora_hui` nor `bevy_hui`.
#[derive(Resource, Default)]
pub struct ScriptUiInbox {
    pub pending: Vec<UiCallback>,
}

/// A broadcast game event: a name plus arguments, sent by anyone, heard by
/// anyone who cares.
///
/// The counterpart to addressing an entity by id. `set_on("music", …)` is right
/// when you know exactly what you're talking to; an event is right when the
/// sender shouldn't have to — "the boss died" may interest a quest tracker, an
/// achievement check and a save trigger, and the boss should not have to know
/// that any of them exist.
///
/// Triggered as an observer event, so Rust systems listen with
/// `app.add_observer(|t: On<GameEvent>| …)`. Scripts get the same events
/// through `on_event(name, args)`.
#[derive(bevy::prelude::Event, Clone, Debug)]
pub struct GameEvent {
    pub name: String,
    pub args: std::collections::HashMap<String, ScriptActionValue>,
    /// The entity that emitted it, when a script did. `None` for engine- or
    /// Rust-side emits.
    pub from: Option<bevy::ecs::entity::Entity>,
}

/// Queue of events awaiting dispatch, drained once per frame.
///
/// Emits are deferred by a frame rather than delivered inline, for two reasons:
/// a script emitting from inside a hook would otherwise re-enter the VM
/// mid-call, and an event handler that emits could otherwise recurse without
/// bound. The same reasoning as the `ScriptCommand` queue.
#[derive(Resource, Default)]
pub struct GameEventQueue {
    pub pending: Vec<GameEvent>,
}

/// A scene finishing (or failing) to load, awaiting dispatch to scripts'
/// `on_scene_loaded(path)` / `on_scene_load_failed(path, error)` hook.
#[derive(Clone, Debug)]
pub struct SceneEvent {
    /// The scene path, as the load was requested (project-relative).
    pub path: String,
    /// `None` on success; the failure reason otherwise.
    pub error: Option<String>,
}

/// Inbox bridge for scene-load completion.
///
/// `renzora_engine`'s scene streamer pushes a [`SceneEvent`] when the main
/// scene finishes or fails; `renzora_scripting` drains it each frame and
/// invokes the hook on every live script (broadcast, same semantics as
/// [`ScriptRpcInbox`]).
///
/// The point of the hook is that it reaches scripts the load did **not**
/// destroy: a script in the outgoing scene is despawned partway through, so
/// only a `Persistent` one (an autoload scene) is still alive to hear that the
/// new scene arrived. That is what makes a loading screen possible.
#[derive(Resource, Default)]
pub struct ScriptSceneInbox {
    pub pending: Vec<SceneEvent>,
}

/// An animation event fired when playback crosses a clip marker, awaiting
/// dispatch to scripts' `on_animation_event(name, entity)` hook.
#[derive(Clone, Debug)]
pub struct AnimEvent {
    /// The marker name.
    pub name: String,
    /// The animator entity that fired it (`Entity::to_bits`).
    pub entity_bits: u64,
}

/// Inbox bridge for animation events. The animation runtime pushes an
/// [`AnimEvent`] when playback crosses a clip marker; `renzora_scripting` drains
/// it each frame and invokes every script's `on_animation_event(name, entity)`
/// hook (broadcast, same semantics as [`ScriptUiInbox`]). Lives in core so
/// scripting doesn't depend on `renzora_animation`.
#[derive(Resource, Default)]
pub struct ScriptAnimEventInbox {
    pub pending: Vec<AnimEvent>,
}

/// One immediate-mode 2D draw command issued from a script's `on_draw(g)` hook
/// (a Godot `_draw()` / HTML-canvas-style drawing pass). Coordinates are in the
/// draw surface's local pixels — top-left origin, y-down, like CSS/UI. Colours are
/// sRGB `[r, g, b, a]` in 0..1. Each frame a script rebuilds its whole list.
#[derive(Clone, Debug)]
pub enum DrawCmd {
    /// Straight stroke between two points.
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: [f32; 4],
        thickness: f32,
    },
    /// Stroked circular arc, `start`/`end` in degrees (0 = +x, clockwise, y-down).
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        start: f32,
        end: f32,
        color: [f32; 4],
        thickness: f32,
    },
    /// Filled circle.
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        color: [f32; 4],
    },
    /// Filled axis-aligned rectangle.
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
    },
    /// Filled triangle from three points. `g.poly` fans a point list into these.
    Triangle {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        x3: f32,
        y3: f32,
        color: [f32; 4],
    },
    /// Text baseline-anchored at `(x, y)`, centred horizontally on `x`.
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: [f32; 4],
    },
}

/// Per-entity immediate-mode draw lists, rebuilt each frame from scripts'
/// `on_draw(g)` hooks. `renzora_scripting` fills it (keyed by the script's own
/// entity); the UI vector renderer in `renzora_ember`'s game_ui drains it into a
/// pooled set of shape entities under the entity's draw surface. Lives in core so
/// scripting depends on neither `renzora_ember` nor `bevy_hui`.
#[derive(Resource, Default)]
pub struct ScriptDrawBuffer {
    pub per_entity: std::collections::HashMap<Entity, Vec<DrawCmd>>,
}

/// Draw-surface sizes (px) published by the UI renderer, keyed by the *script*
/// entity that owns each `<canvas>` node. `renzora_scripting` reads this to size
/// the `g` context (`g.width`/`g.height`) before calling `on_draw`, and only calls
/// `on_draw` for entities that have a registered surface. The inverse of
/// [`ScriptDrawBuffer`]: game_ui writes sizes here, reads commands from there.
#[derive(Resource, Default)]
pub struct ScriptDrawSurfaces {
    pub per_entity: std::collections::HashMap<Entity, Vec2>,
}

/// Input state resource collected each frame for scripts and blueprints.
#[derive(Resource, Default, Clone)]
pub struct ScriptInput {
    pub keys_pressed: HashMap<KeyCode, bool>,
    pub keys_just_pressed: HashMap<KeyCode, bool>,
    pub keys_just_released: HashMap<KeyCode, bool>,
    pub mouse_pressed: HashMap<MouseButton, bool>,
    pub mouse_just_pressed: HashMap<MouseButton, bool>,
    pub mouse_position: Vec2,
    pub mouse_delta: Vec2,
    pub scroll_delta: Vec2,
    pub gamepad_axes: HashMap<u32, HashMap<GamepadAxis, f32>>,
    pub gamepad_buttons: HashMap<u32, HashMap<GamepadButton, bool>>,
    pub gamepad_buttons_just_pressed: HashMap<u32, HashMap<GamepadButton, bool>>,
    /// Slot ids of currently connected gamepads, sorted ascending. Slots are
    /// stable across the session: a pad keeps its id until it disconnects, and
    /// a newly connected pad takes the lowest free id — so unplugging pad 0
    /// doesn't shift pad 1 down.
    pub connected_gamepads: Vec<u32>,
}

impl ScriptInput {
    pub fn is_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn is_key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys_just_pressed.get(&key).copied().unwrap_or(false)
    }

    pub fn get_movement_vector(&self) -> Vec2 {
        let mut x = 0.0f32;
        let mut y = 0.0f32;
        if self.is_key_pressed(KeyCode::KeyA) || self.is_key_pressed(KeyCode::ArrowLeft) {
            x -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyD) || self.is_key_pressed(KeyCode::ArrowRight) {
            x += 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyS) || self.is_key_pressed(KeyCode::ArrowDown) {
            y -= 1.0;
        }
        if self.is_key_pressed(KeyCode::KeyW) || self.is_key_pressed(KeyCode::ArrowUp) {
            y += 1.0;
        }
        let v = Vec2::new(x, y);
        if v.length_squared() > 0.0 {
            v.normalize()
        } else {
            v
        }
    }

    pub fn get_gamepad_left_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return Vec2::ZERO,
        };
        Vec2::new(
            axes.get(&GamepadAxis::LeftStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::LeftStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_right_stick(&self, id: u32) -> Vec2 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return Vec2::ZERO,
        };
        Vec2::new(
            axes.get(&GamepadAxis::RightStickX).copied().unwrap_or(0.0),
            axes.get(&GamepadAxis::RightStickY).copied().unwrap_or(0.0),
        )
    }

    pub fn get_gamepad_trigger(&self, id: u32, left: bool) -> f32 {
        let axes = match self.gamepad_axes.get(&id) {
            Some(a) => a,
            None => return 0.0,
        };
        let axis = if left {
            GamepadAxis::LeftZ
        } else {
            GamepadAxis::RightZ
        };
        axes.get(&axis).copied().unwrap_or(0.0)
    }

    pub fn is_gamepad_button_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons
            .get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }

    pub fn is_gamepad_button_just_pressed(&self, id: u32, button: GamepadButton) -> bool {
        self.gamepad_buttons_just_pressed
            .get(&id)
            .and_then(|b| b.get(&button))
            .copied()
            .unwrap_or(false)
    }

    pub fn gamepad_count(&self) -> usize {
        self.connected_gamepads.len()
    }

    pub fn is_gamepad_connected(&self, id: u32) -> bool {
        self.connected_gamepads.contains(&id)
    }

    /// Lowest connected gamepad slot, if any. Used to back the legacy
    /// single-gamepad script globals.
    pub fn first_gamepad(&self) -> Option<u32> {
        self.connected_gamepads.first().copied()
    }
}
