//! What a script can see: the world state a hook reads before it runs.
//!
//! ## Why this is split in two
//!
//! [`FrameContext`] is everything that is the same for every scripted entity —
//! time, input, the pressed-key set, connected gamepads, the named-entity
//! lookup. [`EntityContext`] is the handful of things that differ: the
//! transform, the parent, the children, collisions.
//!
//! The obvious design sends one combined blob per entity, and it is the wrong
//! one by roughly the ratio of the two structs. The frame half contains the
//! action tables and a map of *every named entity in the scene*; the entity half
//! is a few hundred bytes. Encoding the frame half once and pointing every call
//! at it turns "cost × entities" into "cost + entities", on both sides —
//! `frame_seq` lets the plugin skip re-decoding it too.
//!
//! Worth being precise about what this does and does not fix. It bounds the
//! cost the *boundary* adds. The engine separately still does
//! `ctx.found_entities = entities_by_name.clone()` inside its per-entity loop,
//! cloning that whole map for every scripted entity every frame, plus five more
//! clones for the key and action tables — a cost that predates any of this and
//! is unchanged by it. The split is what makes those removable, since there is
//! now a frame-shaped thing to build once and hand round; actually removing
//! them is a change to `execution.rs`, not to this file.
//!
//! ## Sparse, not dense
//!
//! The engine holds pressed keys as `HashMap<String, bool>`. On the wire they
//! are a list of the names that are actually down — usually zero to three
//! entries instead of a map with an entry per key ever touched. Same for
//! actions. A language plugin rebuilds whatever shape its runtime wants.

use super::value::ActionValue;
use super::wire::{Reader, WireError, Writer};

/// Clock values for the current frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScriptTime {
    pub elapsed: f64,
    pub delta: f32,
    pub fixed_delta: f32,
    pub frame_count: u64,
}

/// One connected gamepad.
///
/// Button arrays are in the engine's fixed order: South, East, West, North,
/// L1, R1, L2, R2, Select, Start, L3, R3, DPadUp, DPadDown, DPadLeft,
/// DPadRight. [`GAMEPAD_BUTTON_NAMES`] is that order as strings, so a plugin
/// can expose them by name without hard-coding the list.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct GamepadSnapshot {
    /// Stable slot id — a pad keeps it until it disconnects.
    pub id: u32,
    pub left_stick: [f32; 2],
    pub right_stick: [f32; 2],
    pub left_trigger: f32,
    pub right_trigger: f32,
    pub buttons: [bool; 16],
    pub buttons_just_pressed: [bool; 16],
}

/// Script-facing names for the 16 buttons, in [`GamepadSnapshot::buttons`] order.
pub const GAMEPAD_BUTTON_NAMES: [&str; 16] = [
    "south", "east", "west", "north", "l1", "r1", "l2", "r2", "select", "start", "l3", "r3",
    "dpad_up", "dpad_down", "dpad_left", "dpad_right",
];

impl GamepadSnapshot {
    fn encode(&self, w: &mut Writer) {
        w.u32(self.id);
        w.f32x2(self.left_stick);
        w.f32x2(self.right_stick);
        w.f32(self.left_trigger);
        w.f32(self.right_trigger);
        w.u16(pack16(&self.buttons));
        w.u16(pack16(&self.buttons_just_pressed));
    }

    fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            id: r.u32()?,
            left_stick: r.f32x2()?,
            right_stick: r.f32x2()?,
            left_trigger: r.f32()?,
            right_trigger: r.f32()?,
            buttons: unpack16(r.u16()?),
            buttons_just_pressed: unpack16(r.u16()?),
        })
    }
}

fn pack16(bits: &[bool; 16]) -> u16 {
    let mut out = 0u16;
    for (i, b) in bits.iter().enumerate() {
        if *b {
            out |= 1 << i;
        }
    }
    out
}

fn unpack16(bits: u16) -> [bool; 16] {
    let mut out = [false; 16];
    for (i, o) in out.iter_mut().enumerate() {
        *o = bits & (1 << i) != 0;
    }
    out
}

fn pack8(bits: &[bool; 5]) -> u8 {
    let mut out = 0u8;
    for (i, b) in bits.iter().enumerate() {
        if *b {
            out |= 1 << i;
        }
    }
    out
}

fn unpack8(bits: u8) -> [bool; 5] {
    let mut out = [false; 5];
    for (i, o) in out.iter_mut().enumerate() {
        *o = bits & (1 << i) != 0;
    }
    out
}

/// State shared by every script running this frame. Encoded once per frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FrameContext {
    pub time: ScriptTime,

    // Input.
    pub input_movement: [f32; 2],
    pub mouse_position: [f32; 2],
    pub mouse_delta: [f32; 2],
    pub mouse_scroll: f32,
    pub camera_yaw: f32,
    /// Names of the keys currently down. Sparse — see the module docs.
    pub keys_pressed: Vec<String>,
    pub keys_just_pressed: Vec<String>,
    pub keys_just_released: Vec<String>,
    pub mouse_buttons_pressed: [bool; 5],
    pub mouse_buttons_just_pressed: [bool; 5],

    /// Live scene EV-100 from the auto-exposure readback. 0.0 when auto-exposure
    /// is inactive or has not read back yet.
    pub camera_ev: f32,

    /// Configured game resolution in world units (`ProjectConfig.viewport`).
    /// Falls back to 1920×1080 with no project loaded.
    pub project_width: f32,
    pub project_height: f32,

    // Networking. All false/0 when networking is not active.
    pub net_is_server: bool,
    pub net_is_connected: bool,
    pub net_player_count: i32,

    /// Every connected pad. The legacy single-pad globals are the first entry —
    /// a plugin derives them rather than the engine sending them twice.
    pub gamepads: Vec<GamepadSnapshot>,

    // Unified action input (keyboard + mouse + gamepad through `InputMap`).
    pub actions_pressed: Vec<String>,
    pub actions_just_pressed: Vec<String>,
    pub actions_just_released: Vec<String>,
    pub action_axis_1d: Vec<(String, f32)>,
    pub action_axis_2d: Vec<(String, [f32; 2])>,

    /// Every named entity in the scene, for `find_entity`.
    pub named_entities: Vec<(String, u64)>,
    /// Timers that finished this frame.
    pub timers_just_finished: Vec<String>,
}

impl FrameContext {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.time.elapsed.to_bits());
        w.f32(self.time.delta);
        w.f32(self.time.fixed_delta);
        w.u64(self.time.frame_count);

        w.f32x2(self.input_movement);
        w.f32x2(self.mouse_position);
        w.f32x2(self.mouse_delta);
        w.f32(self.mouse_scroll);
        w.f32(self.camera_yaw);
        encode_strs(w, &self.keys_pressed);
        encode_strs(w, &self.keys_just_pressed);
        encode_strs(w, &self.keys_just_released);
        w.u8(pack8(&self.mouse_buttons_pressed));
        w.u8(pack8(&self.mouse_buttons_just_pressed));

        w.f32(self.camera_ev);
        w.f32(self.project_width);
        w.f32(self.project_height);

        w.bool(self.net_is_server);
        w.bool(self.net_is_connected);
        w.u32(self.net_player_count as u32);

        w.count(self.gamepads.len());
        for g in &self.gamepads {
            g.encode(w);
        }

        encode_strs(w, &self.actions_pressed);
        encode_strs(w, &self.actions_just_pressed);
        encode_strs(w, &self.actions_just_released);
        w.count(self.action_axis_1d.len());
        for (k, v) in &self.action_axis_1d {
            w.str(k);
            w.f32(*v);
        }
        w.count(self.action_axis_2d.len());
        for (k, v) in &self.action_axis_2d {
            w.str(k);
            w.f32x2(*v);
        }

        w.count(self.named_entities.len());
        for (k, v) in &self.named_entities {
            w.str(k);
            w.u64(*v);
        }
        encode_strs(w, &self.timers_just_finished);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            time: ScriptTime {
                elapsed: f64::from_bits(r.u64()?),
                delta: r.f32()?,
                fixed_delta: r.f32()?,
                frame_count: r.u64()?,
            },
            input_movement: r.f32x2()?,
            mouse_position: r.f32x2()?,
            mouse_delta: r.f32x2()?,
            mouse_scroll: r.f32()?,
            camera_yaw: r.f32()?,
            keys_pressed: r.list(|r| r.string())?,
            keys_just_pressed: r.list(|r| r.string())?,
            keys_just_released: r.list(|r| r.string())?,
            mouse_buttons_pressed: unpack8(r.u8()?),
            mouse_buttons_just_pressed: unpack8(r.u8()?),
            camera_ev: r.f32()?,
            project_width: r.f32()?,
            project_height: r.f32()?,
            net_is_server: r.bool()?,
            net_is_connected: r.bool()?,
            net_player_count: r.u32()? as i32,
            gamepads: r.list(GamepadSnapshot::decode)?,
            actions_pressed: r.list(|r| r.string())?,
            actions_just_pressed: r.list(|r| r.string())?,
            actions_just_released: r.list(|r| r.string())?,
            action_axis_1d: r.list(|r| Ok((r.string()?, r.f32()?)))?,
            action_axis_2d: r.list(|r| Ok((r.string()?, r.f32x2()?)))?,
            named_entities: r.list(|r| Ok((r.string()?, r.u64()?)))?,
            timers_just_finished: r.list(|r| r.string())?,
        })
    }
}

fn encode_strs(w: &mut Writer, v: &[String]) {
    w.count(v.len());
    for s in v {
        w.str(s);
    }
}

/// A child of the scripted entity.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChildNode {
    pub entity_id: u64,
    pub name: String,
    pub position: [f32; 3],
    /// Euler degrees, YXZ — the order the engine's inspector uses.
    pub rotation: [f32; 3],
    pub scale: [f32; 3],
}

/// The outcome of a `raycast(...)` issued on a previous frame.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RaycastHit {
    pub hit: bool,
    pub entity_id: Option<u64>,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    pub distance: f32,
}

/// State specific to the entity a hook is running for.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EntityContext {
    pub entity_id: u64,
    pub name: String,

    pub position: [f32; 3],
    /// Quaternion `[x, y, z, w]`, for deriving `forward`/`right`/`up`.
    pub rotation: [f32; 4],
    /// The same rotation as euler degrees, YXZ — the order the inspector uses.
    ///
    /// Sent alongside the quaternion rather than left for the plugin to derive,
    /// and the redundancy is deliberate. Quaternion-to-YXZ has a gimbal case and
    /// an easily-inverted sign, and getting it wrong would silently change what
    /// `rotation_y` means to every script that turns a character. The engine
    /// already has glam and has always done this conversion; making each
    /// language plugin reimplement it is three lines saved on the wire against
    /// one subtle bug per language.
    pub rotation_euler: [f32; 3],
    pub scale: [f32; 3],

    pub has_parent: bool,
    pub parent_entity: Option<u64>,
    pub parent_position: [f32; 3],
    /// Euler degrees, YXZ.
    pub parent_rotation: [f32; 3],
    pub parent_scale: [f32; 3],
    pub children: Vec<ChildNode>,

    pub collisions_entered: Vec<u64>,
    pub collisions_exited: Vec<u64>,
    pub active_collisions: Vec<u64>,

    pub raycast_results: Vec<(String, RaycastHit)>,

    pub health: f32,
    pub max_health: f32,
    pub health_percent: f32,
    pub is_invincible: bool,
    pub light_intensity: f32,
    pub light_color: [f32; 3],
    pub material_color: [f32; 4],
}

impl EntityContext {
    pub fn encode(&self, w: &mut Writer) {
        w.u64(self.entity_id);
        w.str(&self.name);
        w.f32x3(self.position);
        w.f32x4(self.rotation);
        w.f32x3(self.rotation_euler);
        w.f32x3(self.scale);

        w.bool(self.has_parent);
        w.opt_u64(self.parent_entity);
        w.f32x3(self.parent_position);
        w.f32x3(self.parent_rotation);
        w.f32x3(self.parent_scale);

        w.count(self.children.len());
        for c in &self.children {
            w.u64(c.entity_id);
            w.str(&c.name);
            w.f32x3(c.position);
            w.f32x3(c.rotation);
            w.f32x3(c.scale);
        }

        encode_u64s(w, &self.collisions_entered);
        encode_u64s(w, &self.collisions_exited);
        encode_u64s(w, &self.active_collisions);

        w.count(self.raycast_results.len());
        for (k, h) in &self.raycast_results {
            w.str(k);
            w.bool(h.hit);
            w.opt_u64(h.entity_id);
            w.f32x3(h.point);
            w.f32x3(h.normal);
            w.f32(h.distance);
        }

        w.f32(self.health);
        w.f32(self.max_health);
        w.f32(self.health_percent);
        w.bool(self.is_invincible);
        w.f32(self.light_intensity);
        w.f32x3(self.light_color);
        w.f32x4(self.material_color);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            entity_id: r.u64()?,
            name: r.string()?,
            position: r.f32x3()?,
            rotation: r.f32x4()?,
            rotation_euler: r.f32x3()?,
            scale: r.f32x3()?,
            has_parent: r.bool()?,
            parent_entity: r.opt_u64()?,
            parent_position: r.f32x3()?,
            parent_rotation: r.f32x3()?,
            parent_scale: r.f32x3()?,
            children: r.list(|r| {
                Ok(ChildNode {
                    entity_id: r.u64()?,
                    name: r.string()?,
                    position: r.f32x3()?,
                    rotation: r.f32x3()?,
                    scale: r.f32x3()?,
                })
            })?,
            collisions_entered: r.list(|r| r.u64())?,
            collisions_exited: r.list(|r| r.u64())?,
            active_collisions: r.list(|r| r.u64())?,
            raycast_results: r.list(|r| {
                Ok((
                    r.string()?,
                    RaycastHit {
                        hit: r.bool()?,
                        entity_id: r.opt_u64()?,
                        point: r.f32x3()?,
                        normal: r.f32x3()?,
                        distance: r.f32()?,
                    },
                ))
            })?,
            health: r.f32()?,
            max_health: r.f32()?,
            health_percent: r.f32()?,
            is_invincible: r.bool()?,
            light_intensity: r.f32()?,
            light_color: r.f32x3()?,
            material_color: r.f32x4()?,
        })
    }
}

fn encode_u64s(w: &mut Writer, v: &[u64]) {
    w.count(v.len());
    for x in v {
        w.u64(*x);
    }
}

/// Arguments specific to the hook being invoked.
///
/// The op on the call already says which hook it is; this carries what that
/// hook needs beyond the context. `None` covers `on_ready`, `on_update` and
/// prop parsing, which take nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum HookArgs {
    None,
    /// `on_rpc(name, args, from)`.
    Rpc {
        name: String,
        from: u64,
        args: Vec<(String, ActionValue)>,
    },
    /// `on_ui(name, args, entity)` — a UI markup callback with no Rust binding.
    Ui {
        name: String,
        entity_bits: u64,
        args: Vec<(String, ActionValue)>,
    },
    /// `on_draw(g)` — `width`/`height` are the draw surface size in pixels.
    Draw { width: f32, height: f32 },
    /// `on_animation_event(name, entity)`.
    AnimationEvent { name: String, entity_bits: u64 },
    /// `on_http(callback, status, body)`.
    Http {
        callback: String,
        status: u16,
        body: String,
    },
    /// `on_player_joined(id)` when `joined`, else `on_player_left(id)`.
    PlayerEvent { id: u64, joined: bool },
    /// An expression to evaluate, for the console REPL.
    Eval { expr: String },
}

impl HookArgs {
    pub fn encode(&self, w: &mut Writer) {
        match self {
            Self::None => w.u16(0),
            Self::Rpc { name, from, args } => {
                w.u16(1);
                w.str(name);
                w.u64(*from);
                encode_args(w, args);
            }
            Self::Ui {
                name,
                entity_bits,
                args,
            } => {
                w.u16(2);
                w.str(name);
                w.u64(*entity_bits);
                encode_args(w, args);
            }
            Self::Draw { width, height } => {
                w.u16(3);
                w.f32(*width);
                w.f32(*height);
            }
            Self::AnimationEvent { name, entity_bits } => {
                w.u16(4);
                w.str(name);
                w.u64(*entity_bits);
            }
            Self::Http {
                callback,
                status,
                body,
            } => {
                w.u16(5);
                w.str(callback);
                w.u16(*status);
                w.str(body);
            }
            Self::PlayerEvent { id, joined } => {
                w.u16(6);
                w.u64(*id);
                w.bool(*joined);
            }
            Self::Eval { expr } => {
                w.u16(7);
                w.str(expr);
            }
        }
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let tag = r.u16()?;
        Ok(match tag {
            0 => Self::None,
            1 => Self::Rpc {
                name: r.string()?,
                from: r.u64()?,
                args: decode_args(r)?,
            },
            2 => Self::Ui {
                name: r.string()?,
                entity_bits: r.u64()?,
                args: decode_args(r)?,
            },
            3 => Self::Draw {
                width: r.f32()?,
                height: r.f32()?,
            },
            4 => Self::AnimationEvent {
                name: r.string()?,
                entity_bits: r.u64()?,
            },
            5 => Self::Http {
                callback: r.string()?,
                status: r.u16()?,
                body: r.string()?,
            },
            6 => Self::PlayerEvent {
                id: r.u64()?,
                joined: r.bool()?,
            },
            7 => Self::Eval { expr: r.string()? },
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

fn encode_args(w: &mut Writer, args: &[(String, ActionValue)]) {
    w.count(args.len());
    for (k, v) in args {
        w.str(k);
        v.encode(w);
    }
}

fn decode_args(r: &mut Reader) -> Result<Vec<(String, ActionValue)>, WireError> {
    r.list(|r| Ok((r.string()?, ActionValue::decode(r)?)))
}

/// Asset-load progress, for a script's loading screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AssetProgress {
    /// `"idle"`, `"loading"` or `"done"`.
    pub state: String,
    pub total_files: u32,
    pub loaded_files: u32,
    pub total_bytes: u64,
    pub loaded_bytes: u64,
    pub current_path: Option<String>,
    pub elapsed_secs: f32,
    /// Best-effort `[0.0, 1.0]`.
    pub fraction: f32,
}

impl AssetProgress {
    pub fn encode(&self, w: &mut Writer) {
        w.str(&self.state);
        w.u32(self.total_files);
        w.u32(self.loaded_files);
        w.u64(self.total_bytes);
        w.u64(self.loaded_bytes);
        w.opt_str(self.current_path.as_deref());
        w.f32(self.elapsed_secs);
        w.f32(self.fraction);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        Ok(Self {
            state: r.string()?,
            total_files: r.u32()?,
            loaded_files: r.u32()?,
            total_bytes: r.u64()?,
            loaded_bytes: r.u64()?,
            current_path: r.opt_string()?,
            elapsed_secs: r.f32()?,
            fraction: r.f32()?,
        })
    }
}

/// The type of one parameter of a declared binding.
///
/// [`ParamKind::Vec3`] consumes **three** numbers from the script call and
/// produces one [`ActionValue::Vec3`] argument. That is not a convenience: both
/// shapes exist in the engine today — `apply_force(x, y, z)` sends three
/// separate float args named `x`, `y`, `z`, while `nav_set_destination(x, y, z)`
/// sends one Vec3 named `target` — and a binding system that could only express
/// one of them would not replace the hand-written functions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Float,
    Int,
    Bool,
    Str,
    Vec3,
}

impl ParamKind {
    fn tag(self) -> u16 {
        match self {
            Self::Float => 0,
            Self::Int => 1,
            Self::Bool => 2,
            Self::Str => 3,
            Self::Vec3 => 4,
        }
    }

    fn from_tag(t: u16) -> Result<Self, WireError> {
        Ok(match t {
            0 => Self::Float,
            1 => Self::Int,
            2 => Self::Bool,
            3 => Self::Str,
            4 => Self::Vec3,
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }

    /// How many script-level arguments this parameter consumes.
    pub fn arity(self) -> usize {
        match self {
            Self::Vec3 => 3,
            _ => 1,
        }
    }
}

/// One parameter of a declared binding.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    /// The key this becomes in the action's argument list.
    pub name: String,
    pub kind: ParamKind,
}

/// What a declared binding does when called.
#[derive(Debug, Clone, PartialEq)]
pub enum BindingKind {
    /// Pack the arguments and emit [`ScriptCommand::Action`]. Returns nothing.
    ///
    /// [`ScriptCommand::Action`]: super::ScriptCommand::Action
    Action { action: String },
    /// Read a reflected field and return it.
    ///
    /// `component` and `field` may contain `{0}`, `{1}` … placeholders, which
    /// the plugin substitutes with the call's arguments. That is what lets
    /// `get_animation_length(name)` be declared rather than written: it is
    /// `Read { component: "AnimatorReadState", field: "clip_lengths.{0}" }`.
    Read { component: String, field: String },
    /// Look the argument up in the localization table and return the result.
    Translate,
}

/// A script function a domain crate declares rather than writes.
///
/// This is what replaced `ScriptExtension::register_lua_functions`. Every one
/// of the engine's five extensions turned out to be sugar of exactly this
/// shape — pack the arguments, fire an action — so declaring it means the
/// domain crate stops linking a Lua interpreter and *every* language backend
/// gets the function for free. A Wren plugin picks these up without
/// `renzora_physics` knowing Wren exists.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    /// The name scripts call, e.g. `"apply_force"`.
    pub name: String,
    pub kind: BindingKind,
    /// Parameters in call order.
    pub params: Vec<Param>,
    /// One-line description, for editor autocomplete. May be empty.
    pub doc: String,
}

impl Binding {
    pub fn encode(&self, w: &mut Writer) {
        w.str(&self.name);
        match &self.kind {
            BindingKind::Action { action } => {
                w.u16(0);
                w.str(action);
            }
            BindingKind::Read { component, field } => {
                w.u16(1);
                w.str(component);
                w.str(field);
            }
            BindingKind::Translate => w.u16(2),
        }
        w.count(self.params.len());
        for p in &self.params {
            w.str(&p.name);
            w.u16(p.kind.tag());
        }
        w.str(&self.doc);
    }

    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        let name = r.string()?;
        let kind = match r.u16()? {
            0 => BindingKind::Action {
                action: r.string()?,
            },
            1 => BindingKind::Read {
                component: r.string()?,
                field: r.string()?,
            },
            2 => BindingKind::Translate,
            t => return Err(WireError::UnknownTag(t as u32)),
        };
        let params = r.list(|r| {
            Ok(Param {
                name: r.string()?,
                kind: ParamKind::from_tag(r.u16()?)?,
            })
        })?;
        Ok(Self {
            name,
            kind,
            params,
            doc: r.string()?,
        })
    }
}

/// Substitute `{0}`, `{1}` … in a [`BindingKind::Read`] path with the call's
/// arguments.
///
/// Lives here rather than in either backend so every language resolves a path
/// the same way — a Lua plugin and a Wren plugin disagreeing about what
/// `clip_lengths.{0}` means would be a genuinely miserable bug to find.
///
/// A placeholder with no matching argument is left as written, deliberately:
/// a path that visibly fails to resolve beats one that silently reads a
/// different field.
pub fn substitute(template: &str, args: &[String]) -> String {
    // Almost every template has no placeholder at all, so do not build a new
    // string for the common case.
    if !template.contains('{') {
        return template.to_string();
    }
    let mut out = template.to_string();
    for (i, a) in args.iter().enumerate() {
        out = out.replace(&format!("{{{i}}}"), a);
    }
    out
}

pub fn encode_bindings(w: &mut Writer, bindings: &[Binding]) {
    w.count(bindings.len());
    for b in bindings {
        b.encode(w);
    }
}

pub fn decode_bindings(r: &mut Reader) -> Result<Vec<Binding>, WireError> {
    r.list(Binding::decode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::script::value::PropValue;

    #[test]
    fn frame_context_round_trips() {
        let f = FrameContext {
            time: ScriptTime {
                elapsed: 12.5,
                delta: 0.016,
                fixed_delta: 0.02,
                frame_count: 900,
            },
            input_movement: [1.0, -1.0],
            mouse_position: [100.0, 200.0],
            mouse_delta: [1.0, 2.0],
            mouse_scroll: -1.0,
            camera_yaw: 90.0,
            keys_pressed: vec!["w".into(), "shift".into()],
            keys_just_pressed: vec!["space".into()],
            keys_just_released: vec![],
            mouse_buttons_pressed: [true, false, true, false, false],
            mouse_buttons_just_pressed: [false; 5],
            camera_ev: 9.5,
            project_width: 1920.0,
            project_height: 1080.0,
            net_is_server: true,
            net_is_connected: true,
            net_player_count: 4,
            gamepads: vec![GamepadSnapshot {
                id: 1,
                left_stick: [0.5, -0.5],
                right_stick: [0.0, 0.0],
                left_trigger: 1.0,
                right_trigger: 0.0,
                buttons: unpack16(0b1010_0101_1010_0101),
                buttons_just_pressed: unpack16(1),
            }],
            actions_pressed: vec!["jump".into()],
            actions_just_pressed: vec![],
            actions_just_released: vec!["fire".into()],
            action_axis_1d: vec![("throttle".into(), 0.5)],
            action_axis_2d: vec![("move".into(), [1.0, 0.0])],
            named_entities: vec![("Player".into(), 42), ("Sun".into(), 7)],
            timers_just_finished: vec!["reload".into()],
        };

        let mut w = Writer::new();
        f.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(FrameContext::decode(&mut r).unwrap(), f);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn default_frame_context_round_trips() {
        let f = FrameContext::default();
        let mut w = Writer::new();
        f.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(FrameContext::decode(&mut r).unwrap(), f);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn entity_context_round_trips() {
        let e = EntityContext {
            entity_id: 42,
            name: "Player".into(),
            position: [1.0, 2.0, 3.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            rotation_euler: [0.0, 45.0, 0.0],
            scale: [1.0, 1.0, 1.0],
            has_parent: true,
            parent_entity: Some(7),
            parent_position: [0.0, 1.0, 0.0],
            parent_rotation: [0.0, 90.0, 0.0],
            parent_scale: [1.0, 1.0, 1.0],
            children: vec![ChildNode {
                entity_id: 43,
                name: "Weapon".into(),
                position: [0.0, 0.0, 1.0],
                rotation: [0.0; 3],
                scale: [1.0; 3],
            }],
            collisions_entered: vec![1, 2],
            collisions_exited: vec![3],
            active_collisions: vec![1],
            raycast_results: vec![(
                "ground".into(),
                RaycastHit {
                    hit: true,
                    entity_id: Some(9),
                    point: [0.0, -1.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    distance: 1.0,
                },
            )],
            health: 50.0,
            max_health: 100.0,
            health_percent: 0.5,
            is_invincible: false,
            light_intensity: 800.0,
            light_color: [1.0, 1.0, 1.0],
            material_color: [1.0, 0.0, 0.0, 1.0],
        };

        let mut w = Writer::new();
        e.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(EntityContext::decode(&mut r).unwrap(), e);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn every_hook_args_variant_round_trips() {
        for a in [
            HookArgs::None,
            HookArgs::Rpc {
                name: "shoot".into(),
                from: 3,
                args: vec![("power".into(), ActionValue::Float(1.0))],
            },
            HookArgs::Ui {
                name: "click".into(),
                entity_bits: 9,
                args: vec![],
            },
            HookArgs::Draw {
                width: 800.0,
                height: 600.0,
            },
            HookArgs::AnimationEvent {
                name: "footstep".into(),
                entity_bits: 9,
            },
            HookArgs::Http {
                callback: "on_done".into(),
                status: 200,
                body: "{}".into(),
            },
            HookArgs::PlayerEvent { id: 1, joined: true },
            HookArgs::Eval {
                expr: "1 + 1".into(),
            },
        ] {
            let mut w = Writer::new();
            a.encode(&mut w);
            let bytes = w.into_bytes();
            let mut r = Reader::new(&bytes);
            assert_eq!(HookArgs::decode(&mut r).unwrap(), a);
            assert_eq!(r.remaining(), 0);
        }
    }

    #[test]
    fn bindings_round_trip() {
        let bs = vec![
            Binding {
                name: "apply_force".into(),
                kind: BindingKind::Action {
                    action: "apply_force".into(),
                },
                params: vec![
                    Param { name: "x".into(), kind: ParamKind::Float },
                    Param { name: "y".into(), kind: ParamKind::Float },
                    Param { name: "z".into(), kind: ParamKind::Float },
                ],
                doc: "Apply a force in world space.".into(),
            },
            Binding {
                name: "nav_set_destination".into(),
                kind: BindingKind::Action {
                    action: "nav_set_destination".into(),
                },
                params: vec![Param {
                    name: "target".into(),
                    kind: ParamKind::Vec3,
                }],
                doc: String::new(),
            },
            Binding {
                name: "get_animation_length".into(),
                kind: BindingKind::Read {
                    component: "AnimatorReadState".into(),
                    field: "clip_lengths.{0}".into(),
                },
                params: vec![Param { name: "name".into(), kind: ParamKind::Str }],
                doc: String::new(),
            },
            Binding {
                name: "tr".into(),
                kind: BindingKind::Translate,
                params: vec![Param { name: "key".into(), kind: ParamKind::Str }],
                doc: String::new(),
            },
        ];

        let mut w = Writer::new();
        encode_bindings(&mut w, &bs);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(decode_bindings(&mut r).unwrap(), bs);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_vec3_param_consumes_three_script_arguments() {
        assert_eq!(ParamKind::Vec3.arity(), 3);
        assert_eq!(ParamKind::Float.arity(), 1);
        assert_eq!(ParamKind::Str.arity(), 1);
    }

    #[test]
    fn asset_progress_round_trips() {
        let p = AssetProgress {
            state: "loading".into(),
            total_files: 100,
            loaded_files: 40,
            total_bytes: 1_000_000,
            loaded_bytes: 400_000,
            current_path: Some("models/x.glb".into()),
            elapsed_secs: 2.5,
            fraction: 0.4,
        };
        let mut w = Writer::new();
        p.encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(AssetProgress::decode(&mut r).unwrap(), p);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn bit_packing_round_trips_across_the_whole_range() {
        for i in 0..16 {
            let mut bits = [false; 16];
            bits[i] = true;
            assert_eq!(unpack16(pack16(&bits)), bits);
        }
        assert_eq!(unpack16(pack16(&[true; 16])), [true; 16]);
        for i in 0..5 {
            let mut bits = [false; 5];
            bits[i] = true;
            assert_eq!(unpack8(pack8(&bits)), bits);
        }
    }

    /// `PropValue` is used by the host-call replies rather than by the context
    /// itself, but it travels the same buffers — a smoke test that the two
    /// modules agree.
    #[test]
    fn prop_values_travel_in_context_buffers() {
        let mut w = Writer::new();
        PropValue::Color([1.0, 2.0, 3.0, 4.0]).encode(&mut w);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(
            PropValue::decode(&mut r).unwrap(),
            PropValue::Color([1.0, 2.0, 3.0, 4.0])
        );
    }
}
