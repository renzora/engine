//! Everything a script can ask the engine to do.
//!
//! This is *the* definition, not a mirror: `renzora_scripting` re-exports this
//! enum rather than keeping a parallel one. That is deliberate and it is the
//! difference between this migration working and rotting. A mirror would need
//! a hundred-odd conversion arms, and every command added afterwards would have
//! to be added in three places — the kind of duplication that stays correct for
//! about two months.
//!
//! Living here costs the engine the Bevy vocabulary in these fields: `Vec3`
//! becomes `[f32; 3]`, which is what it already was in memory. The engine
//! converts where it applies them, which is one place per command rather than
//! one per construction site.
//!
//! ## Tags are append-only
//!
//! Each variant's `u16` tag is its position in the list below and **must never
//! be renumbered**. A prebuilt language plugin encodes the tag it was compiled
//! with; renumbering turns its `PlaySound` into somebody else's `Raycast` with
//! the arguments read at the wrong offsets. Append at the end, bump
//! [`VARIANT_COUNT`], and older plugins keep working because they simply never
//! emit the new tag. A host that meets a tag it does not know refuses that one
//! command with a logged line — see [`ScriptCommand::decode`].

use super::value::{ActionValue, PropValue};
use super::wire::{Reader, WireError, Writer};

/// How many variants this build knows. A decoded tag at or above this came from
/// a plugin built against a newer engine.
pub const VARIANT_COUNT: u16 = 116;

/// Commands scripts issue, applied by the engine after the hook returns.
///
/// Language-agnostic by construction — a Lua binding, a Wren binding and the
/// blueprint compiler all produce these same values.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptCommand {
    // === Self-transform ===
    SetPosition { x: f32, y: f32, z: f32 },
    SetRotation { x: f32, y: f32, z: f32 },
    SetScale { x: f32, y: f32, z: f32 },
    Translate { x: f32, y: f32, z: f32 },
    Rotate { x: f32, y: f32, z: f32 },
    LookAt { x: f32, y: f32, z: f32 },
    /// Jump the script's own entity to a named `renzora::CameraPresets` angle.
    /// No-op (with a warning) if the entity has no presets or none match.
    GotoCameraPreset { name: String },

    // === Parent transform ===
    ParentSetPosition { x: f32, y: f32, z: f32 },
    ParentSetRotation { x: f32, y: f32, z: f32 },
    ParentTranslate { x: f32, y: f32, z: f32 },

    // === Child transform ===
    ChildSetPosition { name: String, x: f32, y: f32, z: f32 },
    ChildSetRotation { name: String, x: f32, y: f32, z: f32 },
    ChildTranslate { name: String, x: f32, y: f32, z: f32 },

    // === Environment ===
    SetSunAngles { azimuth: f32, elevation: f32 },
    SetAmbientBrightness { brightness: f32 },
    SetAmbientColor { r: f32, g: f32, b: f32 },
    SetSkyTopColor { r: f32, g: f32, b: f32 },
    SetSkyHorizonColor { r: f32, g: f32, b: f32 },
    SetFog { enabled: bool, start: f32, end: f32 },
    SetFogColor { r: f32, g: f32, b: f32 },
    SetEv100 { value: f32 },

    // === ECS ===
    SpawnEntity {
        name: String,
    },
    SpawnPrimitive {
        name: String,
        primitive_type: String,
        position: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
        /// Optional `MeshColor` so callers can vary the per-voxel tint without a
        /// separate `set` reflection call. `None` uses the shape's default.
        color: Option<[f32; 4]>,
    },
    DespawnEntity {
        entity_id: u64,
    },
    DespawnSelf,
    /// Despawn every entity whose `Name` starts with `prefix` — one world-walk,
    /// used by chunk-streaming scripts to evict a whole chunk in one call.
    DespawnByPrefix {
        prefix: String,
    },
    SetEntityName {
        entity_id: u64,
        name: String,
    },
    AddTag {
        entity_id: Option<u64>,
        tag: String,
    },
    RemoveTag {
        entity_id: Option<u64>,
        tag: String,
    },

    // === Audio ===
    PlaySound { path: String, volume: f32, looping: bool, bus: String },
    PlaySound3D { path: String, volume: f32, position: [f32; 3], bus: String },
    PlayMusic { path: String, volume: f32, fade_in: f32, bus: String },
    StopMusic { fade_out: f32 },
    StopAllSounds,
    SetMasterVolume { volume: f32 },
    PauseSound,
    PauseSoundEntity { entity_id: u64 },
    ResumeSound,
    ResumeSoundEntity { entity_id: u64 },
    SetSoundVolume { volume: f32, fade: f32 },
    SetSoundVolumeEntity { entity_id: u64, volume: f32, fade: f32 },
    SetSoundPitch { pitch: f32, fade: f32 },
    SetSoundPitchEntity { entity_id: u64, pitch: f32, fade: f32 },
    CrossfadeMusic { path: String, volume: f32, duration: f32, bus: String },

    // === Debug draw ===
    Log {
        level: String,
        message: String,
    },
    DrawLine { start: [f32; 3], end: [f32; 3], color: [f32; 4], duration: f32 },
    DrawRay {
        origin: [f32; 3],
        direction: [f32; 3],
        length: f32,
        color: [f32; 4],
        duration: f32,
    },
    DrawSphere { center: [f32; 3], radius: f32, color: [f32; 4], duration: f32 },
    DrawBox {
        center: [f32; 3],
        half_extents: [f32; 3],
        color: [f32; 4],
        duration: f32,
    },
    DrawPoint { position: [f32; 3], size: f32, color: [f32; 4], duration: f32 },

    // === Physics ===
    ApplyForce { entity_id: Option<u64>, force: [f32; 3] },
    ApplyImpulse { entity_id: Option<u64>, impulse: [f32; 3] },
    ApplyTorque { entity_id: Option<u64>, torque: [f32; 3] },
    SetVelocity { entity_id: Option<u64>, velocity: [f32; 3] },
    SetAngularVelocity { entity_id: Option<u64>, velocity: [f32; 3] },
    SetGravityScale { entity_id: Option<u64>, scale: f32 },
    Raycast {
        origin: [f32; 3],
        direction: [f32; 3],
        max_distance: f32,
        result_var: String,
    },

    // === Character controller ===
    CharacterMove { direction: [f32; 2] },
    CharacterJump,
    CharacterSprint { sprinting: bool },

    // === Timers ===
    StartTimer { name: String, duration: f32, repeat: bool },
    StopTimer { name: String },
    PauseTimer { name: String },
    ResumeTimer { name: String },

    // === Scene ===
    LoadScene { path: String },
    UnloadScene { handle_id: u64 },
    SpawnPrefab { path: String, position: [f32; 3], rotation: [f32; 3] },

    // === Animation ===
    PlayAnimation { entity_id: Option<u64>, name: String, looping: bool, speed: f32 },
    StopAnimation { entity_id: Option<u64> },
    PauseAnimation { entity_id: Option<u64> },
    ResumeAnimation { entity_id: Option<u64> },
    SetAnimationSpeed { entity_id: Option<u64>, speed: f32 },
    SeekAnimation { entity_id: Option<u64>, time: f32 },
    CrossfadeAnimation {
        entity_id: Option<u64>,
        name: String,
        duration: f32,
        looping: bool,
    },
    SetAnimationParam { entity_id: Option<u64>, name: String, value: f32 },
    SetAnimationBoolParam { entity_id: Option<u64>, name: String, value: bool },
    TriggerAnimation { entity_id: Option<u64>, name: String },
    SetAnimationLayerWeight {
        entity_id: Option<u64>,
        layer_name: String,
        weight: f32,
    },

    // === Sprite animation ===
    PlaySpriteAnimation { entity_id: Option<u64>, name: String, looping: bool },
    SetSpriteFrame { entity_id: Option<u64>, frame: i64 },

    // === Tweens ===
    Tween {
        entity_id: Option<u64>,
        property: String,
        target: f32,
        duration: f32,
        easing: String,
    },
    TweenPosition {
        entity_id: Option<u64>,
        target: [f32; 3],
        duration: f32,
        easing: String,
    },
    TweenRotation {
        entity_id: Option<u64>,
        target: [f32; 3],
        duration: f32,
        easing: String,
    },
    TweenScale {
        entity_id: Option<u64>,
        target: [f32; 3],
        duration: f32,
        easing: String,
    },

    // === Rendering ===
    SetVisibility { entity_id: Option<u64>, visible: bool },
    SetMaterialColor { entity_id: Option<u64>, color: [f32; 4] },
    SetLightIntensity { entity_id: Option<u64>, intensity: f32 },
    SetLightColor { entity_id: Option<u64>, color: [f32; 3] },

    // === Cursor ===
    LockCursor,
    UnlockCursor,

    // === Camera ===
    SetCameraTarget { position: [f32; 3] },
    SetCameraZoom { zoom: f32 },
    ScreenShake { intensity: f32, duration: f32 },
    CameraFollow { entity_id: u64, offset: [f32; 3], smoothing: f32 },
    StopCameraFollow,

    // === Health ===
    SetHealth { entity_id: Option<u64>, value: f32 },
    SetMaxHealth { entity_id: Option<u64>, value: f32 },
    Damage { entity_id: Option<u64>, amount: f32 },
    Heal { entity_id: Option<u64>, amount: f32 },
    SetInvincible { entity_id: Option<u64>, invincible: bool, duration: f32 },
    Kill { entity_id: Option<u64> },
    Revive { entity_id: Option<u64> },

    // === Particles ===
    ParticlePlay { entity_id: u64 },
    ParticlePause { entity_id: u64 },
    ParticleStop { entity_id: u64 },
    ParticleReset { entity_id: u64 },
    ParticleBurst { entity_id: u64, count: u32 },
    ParticleSetRate { entity_id: u64, multiplier: f32 },
    ParticleSetScale { entity_id: u64, multiplier: f32 },
    ParticleSetTimeScale { entity_id: u64, scale: f32 },
    ParticleSetTint { entity_id: u64, r: f32, g: f32, b: f32, a: f32 },

    // === Property (cross-entity) ===
    SetProperty {
        entity_id: u64,
        property: String,
        value: PropValue,
    },

    // === Generic reflection ===
    /// Set any reflected component field by path. `component_type` is the short
    /// type name (e.g. `"Sun"`); `field_path` is dot-separated (`"color.x"`).
    SetComponentField {
        entity_id: Option<u64>,
        entity_name: Option<String>,
        component_type: String,
        field_path: String,
        value: PropValue,
    },

    // === Generic script action ===
    /// Fires a `renzora::ScriptAction` event that domain crates observe. This is
    /// what the declarative bindings compile down to, and it is why a domain
    /// crate can add script functions without this enum growing.
    ///
    /// `args` is a list rather than a map: order is deterministic on the wire,
    /// the lists are two or three entries long, and the engine builds a map once
    /// where it fires the event.
    Action {
        name: String,
        target_entity: Option<String>,
        args: Vec<(String, ActionValue)>,
    },

    // === HTTP ===
    // === HTTP ===
    /// Fire an async request. The result arrives at the script's
    /// `on_http(callback, status, body)` hook.
    HttpRequest {
        method: String,
        url: String,
        body: Option<String>,
        callback: String,
    },

    // === Events ===
    /// Broadcast a game event. Every script's `on_event(name, args)` fires next
    /// frame, and Rust observers of `renzora::GameEvent` see it too.
    ///
    /// Distinct from [`Self::Action`], which it superficially resembles: an
    /// action names a *verb the engine performs*, consumed by whichever domain
    /// crate implements it, so an unclaimed name silently does nothing. An event
    /// names *something that happened*, goes to everyone, and having no
    /// listeners is a normal outcome rather than a misconfiguration.
    Emit {
        name: String,
        args: Vec<(String, ActionValue)>,
    },
}

impl ScriptCommand {
    /// This command's wire tag. Stable forever — see the module docs.
    pub fn tag(&self) -> u16 {
        use ScriptCommand as C;
        match self {
            C::SetPosition { .. } => 0,
            C::SetRotation { .. } => 1,
            C::SetScale { .. } => 2,
            C::Translate { .. } => 3,
            C::Rotate { .. } => 4,
            C::LookAt { .. } => 5,
            C::GotoCameraPreset { .. } => 6,
            C::ParentSetPosition { .. } => 7,
            C::ParentSetRotation { .. } => 8,
            C::ParentTranslate { .. } => 9,
            C::ChildSetPosition { .. } => 10,
            C::ChildSetRotation { .. } => 11,
            C::ChildTranslate { .. } => 12,
            C::SetSunAngles { .. } => 13,
            C::SetAmbientBrightness { .. } => 14,
            C::SetAmbientColor { .. } => 15,
            C::SetSkyTopColor { .. } => 16,
            C::SetSkyHorizonColor { .. } => 17,
            C::SetFog { .. } => 18,
            C::SetFogColor { .. } => 19,
            C::SetEv100 { .. } => 20,
            C::SpawnEntity { .. } => 21,
            C::SpawnPrimitive { .. } => 22,
            C::DespawnEntity { .. } => 23,
            C::DespawnSelf => 24,
            C::DespawnByPrefix { .. } => 25,
            C::SetEntityName { .. } => 26,
            C::AddTag { .. } => 27,
            C::RemoveTag { .. } => 28,
            C::PlaySound { .. } => 29,
            C::PlaySound3D { .. } => 30,
            C::PlayMusic { .. } => 31,
            C::StopMusic { .. } => 32,
            C::StopAllSounds => 33,
            C::SetMasterVolume { .. } => 34,
            C::PauseSound => 35,
            C::PauseSoundEntity { .. } => 36,
            C::ResumeSound => 37,
            C::ResumeSoundEntity { .. } => 38,
            C::SetSoundVolume { .. } => 39,
            C::SetSoundVolumeEntity { .. } => 40,
            C::SetSoundPitch { .. } => 41,
            C::SetSoundPitchEntity { .. } => 42,
            C::CrossfadeMusic { .. } => 43,
            C::Log { .. } => 44,
            C::DrawLine { .. } => 45,
            C::DrawRay { .. } => 46,
            C::DrawSphere { .. } => 47,
            C::DrawBox { .. } => 48,
            C::DrawPoint { .. } => 49,
            C::ApplyForce { .. } => 50,
            C::ApplyImpulse { .. } => 51,
            C::ApplyTorque { .. } => 52,
            C::SetVelocity { .. } => 53,
            C::SetAngularVelocity { .. } => 54,
            C::SetGravityScale { .. } => 55,
            C::Raycast { .. } => 56,
            C::CharacterMove { .. } => 57,
            C::CharacterJump => 58,
            C::CharacterSprint { .. } => 59,
            C::StartTimer { .. } => 60,
            C::StopTimer { .. } => 61,
            C::PauseTimer { .. } => 62,
            C::ResumeTimer { .. } => 63,
            C::LoadScene { .. } => 64,
            C::UnloadScene { .. } => 65,
            C::SpawnPrefab { .. } => 66,
            C::PlayAnimation { .. } => 67,
            C::StopAnimation { .. } => 68,
            C::PauseAnimation { .. } => 69,
            C::ResumeAnimation { .. } => 70,
            C::SetAnimationSpeed { .. } => 71,
            C::SeekAnimation { .. } => 72,
            C::CrossfadeAnimation { .. } => 73,
            C::SetAnimationParam { .. } => 74,
            C::SetAnimationBoolParam { .. } => 75,
            C::TriggerAnimation { .. } => 76,
            C::SetAnimationLayerWeight { .. } => 77,
            C::PlaySpriteAnimation { .. } => 78,
            C::SetSpriteFrame { .. } => 79,
            C::Tween { .. } => 80,
            C::TweenPosition { .. } => 81,
            C::TweenRotation { .. } => 82,
            C::TweenScale { .. } => 83,
            C::SetVisibility { .. } => 84,
            C::SetMaterialColor { .. } => 85,
            C::SetLightIntensity { .. } => 86,
            C::SetLightColor { .. } => 87,
            C::LockCursor => 88,
            C::UnlockCursor => 89,
            C::SetCameraTarget { .. } => 90,
            C::SetCameraZoom { .. } => 91,
            C::ScreenShake { .. } => 92,
            C::CameraFollow { .. } => 93,
            C::StopCameraFollow => 94,
            C::SetHealth { .. } => 95,
            C::SetMaxHealth { .. } => 96,
            C::Damage { .. } => 97,
            C::Heal { .. } => 98,
            C::SetInvincible { .. } => 99,
            C::Kill { .. } => 100,
            C::Revive { .. } => 101,
            C::ParticlePlay { .. } => 102,
            C::ParticlePause { .. } => 103,
            C::ParticleStop { .. } => 104,
            C::ParticleReset { .. } => 105,
            C::ParticleBurst { .. } => 106,
            C::ParticleSetRate { .. } => 107,
            C::ParticleSetScale { .. } => 108,
            C::ParticleSetTimeScale { .. } => 109,
            C::ParticleSetTint { .. } => 110,
            C::SetProperty { .. } => 111,
            C::SetComponentField { .. } => 112,
            C::Action { .. } => 113,
            C::HttpRequest { .. } => 114,
            C::Emit { .. } => 115,
        }
    }

    pub fn encode(&self, w: &mut Writer) {
        use ScriptCommand as C;
        w.u16(self.tag());
        match self {
            C::SetPosition { x, y, z }
            | C::SetRotation { x, y, z }
            | C::SetScale { x, y, z }
            | C::Translate { x, y, z }
            | C::Rotate { x, y, z }
            | C::LookAt { x, y, z }
            | C::ParentSetPosition { x, y, z }
            | C::ParentSetRotation { x, y, z }
            | C::ParentTranslate { x, y, z }
            | C::SetAmbientColor { r: x, g: y, b: z }
            | C::SetSkyTopColor { r: x, g: y, b: z }
            | C::SetSkyHorizonColor { r: x, g: y, b: z }
            | C::SetFogColor { r: x, g: y, b: z } => {
                w.f32(*x);
                w.f32(*y);
                w.f32(*z);
            }
            C::GotoCameraPreset { name }
            | C::SpawnEntity { name }
            | C::StopTimer { name }
            | C::PauseTimer { name }
            | C::ResumeTimer { name } => w.str(name),
            C::ChildSetPosition { name, x, y, z }
            | C::ChildSetRotation { name, x, y, z }
            | C::ChildTranslate { name, x, y, z } => {
                w.str(name);
                w.f32(*x);
                w.f32(*y);
                w.f32(*z);
            }
            C::SetSunAngles { azimuth, elevation } => {
                w.f32(*azimuth);
                w.f32(*elevation);
            }
            C::SetAmbientBrightness { brightness: v }
            | C::SetEv100 { value: v }
            | C::StopMusic { fade_out: v }
            | C::SetMasterVolume { volume: v }
            | C::SetCameraZoom { zoom: v } => w.f32(*v),
            C::SetFog { enabled, start, end } => {
                w.bool(*enabled);
                w.f32(*start);
                w.f32(*end);
            }
            C::SpawnPrimitive {
                name,
                primitive_type,
                position,
                scale,
                color,
            } => {
                w.str(name);
                w.str(primitive_type);
                encode_opt_f32x3(w, position);
                encode_opt_f32x3(w, scale);
                match color {
                    Some(c) => {
                        w.bool(true);
                        w.f32x4(*c);
                    }
                    None => w.bool(false),
                }
            }
            C::DespawnEntity { entity_id }
            | C::PauseSoundEntity { entity_id }
            | C::ResumeSoundEntity { entity_id }
            | C::UnloadScene { handle_id: entity_id }
            | C::ParticlePlay { entity_id }
            | C::ParticlePause { entity_id }
            | C::ParticleStop { entity_id }
            | C::ParticleReset { entity_id } => w.u64(*entity_id),
            C::DespawnSelf
            | C::StopAllSounds
            | C::PauseSound
            | C::ResumeSound
            | C::CharacterJump
            | C::LockCursor
            | C::UnlockCursor
            | C::StopCameraFollow => {}
            C::DespawnByPrefix { prefix } => w.str(prefix),
            C::SetEntityName { entity_id, name } => {
                w.u64(*entity_id);
                w.str(name);
            }
            C::AddTag { entity_id, tag } | C::RemoveTag { entity_id, tag } => {
                w.opt_u64(*entity_id);
                w.str(tag);
            }
            C::PlaySound {
                path,
                volume,
                looping,
                bus,
            } => {
                w.str(path);
                w.f32(*volume);
                w.bool(*looping);
                w.str(bus);
            }
            C::PlaySound3D {
                path,
                volume,
                position,
                bus,
            } => {
                w.str(path);
                w.f32(*volume);
                w.f32x3(*position);
                w.str(bus);
            }
            C::PlayMusic {
                path,
                volume,
                fade_in: second,
                bus,
            }
            | C::CrossfadeMusic {
                path,
                volume,
                duration: second,
                bus,
            } => {
                w.str(path);
                w.f32(*volume);
                w.f32(*second);
                w.str(bus);
            }
            C::SetSoundVolume {
                volume: a,
                fade: b,
            }
            | C::SetSoundPitch { pitch: a, fade: b }
            | C::ScreenShake {
                intensity: a,
                duration: b,
            } => {
                w.f32(*a);
                w.f32(*b);
            }
            C::SetSoundVolumeEntity {
                entity_id,
                volume: a,
                fade: b,
            }
            | C::SetSoundPitchEntity {
                entity_id,
                pitch: a,
                fade: b,
            } => {
                w.u64(*entity_id);
                w.f32(*a);
                w.f32(*b);
            }
            C::Log { level, message } => {
                w.str(level);
                w.str(message);
            }
            C::DrawLine {
                start,
                end,
                color,
                duration,
            } => {
                w.f32x3(*start);
                w.f32x3(*end);
                w.f32x4(*color);
                w.f32(*duration);
            }
            C::DrawRay {
                origin,
                direction,
                length,
                color,
                duration,
            } => {
                w.f32x3(*origin);
                w.f32x3(*direction);
                w.f32(*length);
                w.f32x4(*color);
                w.f32(*duration);
            }
            C::DrawSphere {
                center,
                radius,
                color,
                duration,
            } => {
                w.f32x3(*center);
                w.f32(*radius);
                w.f32x4(*color);
                w.f32(*duration);
            }
            C::DrawBox {
                center,
                half_extents,
                color,
                duration,
            } => {
                w.f32x3(*center);
                w.f32x3(*half_extents);
                w.f32x4(*color);
                w.f32(*duration);
            }
            C::DrawPoint {
                position,
                size,
                color,
                duration,
            } => {
                w.f32x3(*position);
                w.f32(*size);
                w.f32x4(*color);
                w.f32(*duration);
            }
            C::ApplyForce {
                entity_id,
                force: v,
            }
            | C::ApplyImpulse {
                entity_id,
                impulse: v,
            }
            | C::ApplyTorque {
                entity_id,
                torque: v,
            }
            | C::SetVelocity {
                entity_id,
                velocity: v,
            }
            | C::SetAngularVelocity {
                entity_id,
                velocity: v,
            }
            | C::SetLightColor {
                entity_id,
                color: v,
            } => {
                w.opt_u64(*entity_id);
                w.f32x3(*v);
            }
            C::SetGravityScale { entity_id, scale: v }
            | C::SetAnimationSpeed { entity_id, speed: v }
            | C::SeekAnimation { entity_id, time: v }
            | C::SetLightIntensity {
                entity_id,
                intensity: v,
            }
            | C::SetHealth { entity_id, value: v }
            | C::SetMaxHealth { entity_id, value: v }
            | C::Damage {
                entity_id,
                amount: v,
            }
            | C::Heal {
                entity_id,
                amount: v,
            } => {
                w.opt_u64(*entity_id);
                w.f32(*v);
            }
            C::Raycast {
                origin,
                direction,
                max_distance,
                result_var,
            } => {
                w.f32x3(*origin);
                w.f32x3(*direction);
                w.f32(*max_distance);
                w.str(result_var);
            }
            C::CharacterMove { direction } => w.f32x2(*direction),
            C::CharacterSprint { sprinting: b } => w.bool(*b),
            C::StartTimer {
                name,
                duration,
                repeat,
            } => {
                w.str(name);
                w.f32(*duration);
                w.bool(*repeat);
            }
            C::LoadScene { path } => w.str(path),
            C::SpawnPrefab {
                path,
                position,
                rotation,
            } => {
                w.str(path);
                w.f32x3(*position);
                w.f32x3(*rotation);
            }
            C::PlayAnimation {
                entity_id,
                name,
                looping,
                speed,
            } => {
                w.opt_u64(*entity_id);
                w.str(name);
                w.bool(*looping);
                w.f32(*speed);
            }
            C::StopAnimation { entity_id }
            | C::PauseAnimation { entity_id }
            | C::ResumeAnimation { entity_id }
            | C::Kill { entity_id }
            | C::Revive { entity_id } => w.opt_u64(*entity_id),
            C::CrossfadeAnimation {
                entity_id,
                name,
                duration,
                looping,
            } => {
                w.opt_u64(*entity_id);
                w.str(name);
                w.f32(*duration);
                w.bool(*looping);
            }
            C::SetAnimationParam {
                entity_id,
                name,
                value,
            }
            | C::SetAnimationLayerWeight {
                entity_id,
                layer_name: name,
                weight: value,
            } => {
                w.opt_u64(*entity_id);
                w.str(name);
                w.f32(*value);
            }
            C::SetAnimationBoolParam {
                entity_id,
                name,
                value,
            } => {
                w.opt_u64(*entity_id);
                w.str(name);
                w.bool(*value);
            }
            C::TriggerAnimation { entity_id, name } => {
                w.opt_u64(*entity_id);
                w.str(name);
            }
            C::PlaySpriteAnimation {
                entity_id,
                name,
                looping,
            } => {
                w.opt_u64(*entity_id);
                w.str(name);
                w.bool(*looping);
            }
            C::SetSpriteFrame { entity_id, frame } => {
                w.opt_u64(*entity_id);
                w.i64(*frame);
            }
            C::Tween {
                entity_id,
                property,
                target,
                duration,
                easing,
            } => {
                w.opt_u64(*entity_id);
                w.str(property);
                w.f32(*target);
                w.f32(*duration);
                w.str(easing);
            }
            C::TweenPosition {
                entity_id,
                target,
                duration,
                easing,
            }
            | C::TweenRotation {
                entity_id,
                target,
                duration,
                easing,
            }
            | C::TweenScale {
                entity_id,
                target,
                duration,
                easing,
            } => {
                w.opt_u64(*entity_id);
                w.f32x3(*target);
                w.f32(*duration);
                w.str(easing);
            }
            C::SetVisibility {
                entity_id,
                visible: b,
            } => {
                w.opt_u64(*entity_id);
                w.bool(*b);
            }
            C::SetMaterialColor { entity_id, color } => {
                w.opt_u64(*entity_id);
                w.f32x4(*color);
            }
            C::SetCameraTarget { position } => w.f32x3(*position),
            C::CameraFollow {
                entity_id,
                offset,
                smoothing,
            } => {
                w.u64(*entity_id);
                w.f32x3(*offset);
                w.f32(*smoothing);
            }
            C::SetInvincible {
                entity_id,
                invincible,
                duration,
            } => {
                w.opt_u64(*entity_id);
                w.bool(*invincible);
                w.f32(*duration);
            }
            C::ParticleBurst { entity_id, count } => {
                w.u64(*entity_id);
                w.u32(*count);
            }
            C::ParticleSetRate {
                entity_id,
                multiplier: v,
            }
            | C::ParticleSetScale {
                entity_id,
                multiplier: v,
            }
            | C::ParticleSetTimeScale { entity_id, scale: v } => {
                w.u64(*entity_id);
                w.f32(*v);
            }
            C::ParticleSetTint {
                entity_id,
                r,
                g,
                b,
                a,
            } => {
                w.u64(*entity_id);
                w.f32(*r);
                w.f32(*g);
                w.f32(*b);
                w.f32(*a);
            }
            C::SetProperty {
                entity_id,
                property,
                value,
            } => {
                w.u64(*entity_id);
                w.str(property);
                value.encode(w);
            }
            C::SetComponentField {
                entity_id,
                entity_name,
                component_type,
                field_path,
                value,
            } => {
                w.opt_u64(*entity_id);
                w.opt_str(entity_name.as_deref());
                w.str(component_type);
                w.str(field_path);
                value.encode(w);
            }
            C::Action {
                name,
                target_entity,
                args,
            } => {
                w.str(name);
                w.opt_str(target_entity.as_deref());
                w.count(args.len());
                for (k, v) in args {
                    w.str(k);
                    v.encode(w);
                }
            }
            C::HttpRequest {
                method,
                url,
                body,
                callback,
            } => {
                w.str(method);
                w.str(url);
                w.opt_str(body.as_deref());
                w.str(callback);
            }
            C::Emit { name, args } => {
                w.str(name);
                w.count(args.len());
                for (k, v) in args {
                    w.str(k);
                    v.encode(w);
                }
            }
        }
    }

    /// Decode one command.
    ///
    /// A tag at or beyond [`VARIANT_COUNT`] is [`WireError::UnknownTag`] rather
    /// than a guess. That is not recoverable *within* the stream — the reader
    /// has no way to know how many bytes the unknown variant's fields occupy,
    /// so it cannot skip to the next one. The caller drops the rest of the
    /// batch; see [`decode_list`].
    pub fn decode(r: &mut Reader) -> Result<Self, WireError> {
        use ScriptCommand as C;
        let tag = r.u16()?;
        Ok(match tag {
            0 => C::SetPosition { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            1 => C::SetRotation { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            2 => C::SetScale { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            3 => C::Translate { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            4 => C::Rotate { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            5 => C::LookAt { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            6 => C::GotoCameraPreset { name: r.string()? },
            7 => C::ParentSetPosition { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            8 => C::ParentSetRotation { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            9 => C::ParentTranslate { x: r.f32()?, y: r.f32()?, z: r.f32()? },
            10 => C::ChildSetPosition {
                name: r.string()?,
                x: r.f32()?,
                y: r.f32()?,
                z: r.f32()?,
            },
            11 => C::ChildSetRotation {
                name: r.string()?,
                x: r.f32()?,
                y: r.f32()?,
                z: r.f32()?,
            },
            12 => C::ChildTranslate {
                name: r.string()?,
                x: r.f32()?,
                y: r.f32()?,
                z: r.f32()?,
            },
            13 => C::SetSunAngles {
                azimuth: r.f32()?,
                elevation: r.f32()?,
            },
            14 => C::SetAmbientBrightness { brightness: r.f32()? },
            15 => C::SetAmbientColor { r: r.f32()?, g: r.f32()?, b: r.f32()? },
            16 => C::SetSkyTopColor { r: r.f32()?, g: r.f32()?, b: r.f32()? },
            17 => C::SetSkyHorizonColor { r: r.f32()?, g: r.f32()?, b: r.f32()? },
            18 => C::SetFog {
                enabled: r.bool()?,
                start: r.f32()?,
                end: r.f32()?,
            },
            19 => C::SetFogColor { r: r.f32()?, g: r.f32()?, b: r.f32()? },
            20 => C::SetEv100 { value: r.f32()? },
            21 => C::SpawnEntity { name: r.string()? },
            22 => C::SpawnPrimitive {
                name: r.string()?,
                primitive_type: r.string()?,
                position: decode_opt_f32x3(r)?,
                scale: decode_opt_f32x3(r)?,
                color: if r.bool()? { Some(r.f32x4()?) } else { None },
            },
            23 => C::DespawnEntity { entity_id: r.u64()? },
            24 => C::DespawnSelf,
            25 => C::DespawnByPrefix { prefix: r.string()? },
            26 => C::SetEntityName {
                entity_id: r.u64()?,
                name: r.string()?,
            },
            27 => C::AddTag {
                entity_id: r.opt_u64()?,
                tag: r.string()?,
            },
            28 => C::RemoveTag {
                entity_id: r.opt_u64()?,
                tag: r.string()?,
            },
            29 => C::PlaySound {
                path: r.string()?,
                volume: r.f32()?,
                looping: r.bool()?,
                bus: r.string()?,
            },
            30 => C::PlaySound3D {
                path: r.string()?,
                volume: r.f32()?,
                position: r.f32x3()?,
                bus: r.string()?,
            },
            31 => C::PlayMusic {
                path: r.string()?,
                volume: r.f32()?,
                fade_in: r.f32()?,
                bus: r.string()?,
            },
            32 => C::StopMusic { fade_out: r.f32()? },
            33 => C::StopAllSounds,
            34 => C::SetMasterVolume { volume: r.f32()? },
            35 => C::PauseSound,
            36 => C::PauseSoundEntity { entity_id: r.u64()? },
            37 => C::ResumeSound,
            38 => C::ResumeSoundEntity { entity_id: r.u64()? },
            39 => C::SetSoundVolume {
                volume: r.f32()?,
                fade: r.f32()?,
            },
            40 => C::SetSoundVolumeEntity {
                entity_id: r.u64()?,
                volume: r.f32()?,
                fade: r.f32()?,
            },
            41 => C::SetSoundPitch {
                pitch: r.f32()?,
                fade: r.f32()?,
            },
            42 => C::SetSoundPitchEntity {
                entity_id: r.u64()?,
                pitch: r.f32()?,
                fade: r.f32()?,
            },
            43 => C::CrossfadeMusic {
                path: r.string()?,
                volume: r.f32()?,
                duration: r.f32()?,
                bus: r.string()?,
            },
            44 => C::Log {
                level: r.string()?,
                message: r.string()?,
            },
            45 => C::DrawLine {
                start: r.f32x3()?,
                end: r.f32x3()?,
                color: r.f32x4()?,
                duration: r.f32()?,
            },
            46 => C::DrawRay {
                origin: r.f32x3()?,
                direction: r.f32x3()?,
                length: r.f32()?,
                color: r.f32x4()?,
                duration: r.f32()?,
            },
            47 => C::DrawSphere {
                center: r.f32x3()?,
                radius: r.f32()?,
                color: r.f32x4()?,
                duration: r.f32()?,
            },
            48 => C::DrawBox {
                center: r.f32x3()?,
                half_extents: r.f32x3()?,
                color: r.f32x4()?,
                duration: r.f32()?,
            },
            49 => C::DrawPoint {
                position: r.f32x3()?,
                size: r.f32()?,
                color: r.f32x4()?,
                duration: r.f32()?,
            },
            50 => C::ApplyForce {
                entity_id: r.opt_u64()?,
                force: r.f32x3()?,
            },
            51 => C::ApplyImpulse {
                entity_id: r.opt_u64()?,
                impulse: r.f32x3()?,
            },
            52 => C::ApplyTorque {
                entity_id: r.opt_u64()?,
                torque: r.f32x3()?,
            },
            53 => C::SetVelocity {
                entity_id: r.opt_u64()?,
                velocity: r.f32x3()?,
            },
            54 => C::SetAngularVelocity {
                entity_id: r.opt_u64()?,
                velocity: r.f32x3()?,
            },
            55 => C::SetGravityScale {
                entity_id: r.opt_u64()?,
                scale: r.f32()?,
            },
            56 => C::Raycast {
                origin: r.f32x3()?,
                direction: r.f32x3()?,
                max_distance: r.f32()?,
                result_var: r.string()?,
            },
            57 => C::CharacterMove { direction: r.f32x2()? },
            58 => C::CharacterJump,
            59 => C::CharacterSprint { sprinting: r.bool()? },
            60 => C::StartTimer {
                name: r.string()?,
                duration: r.f32()?,
                repeat: r.bool()?,
            },
            61 => C::StopTimer { name: r.string()? },
            62 => C::PauseTimer { name: r.string()? },
            63 => C::ResumeTimer { name: r.string()? },
            64 => C::LoadScene { path: r.string()? },
            65 => C::UnloadScene { handle_id: r.u64()? },
            66 => C::SpawnPrefab {
                path: r.string()?,
                position: r.f32x3()?,
                rotation: r.f32x3()?,
            },
            67 => C::PlayAnimation {
                entity_id: r.opt_u64()?,
                name: r.string()?,
                looping: r.bool()?,
                speed: r.f32()?,
            },
            68 => C::StopAnimation { entity_id: r.opt_u64()? },
            69 => C::PauseAnimation { entity_id: r.opt_u64()? },
            70 => C::ResumeAnimation { entity_id: r.opt_u64()? },
            71 => C::SetAnimationSpeed {
                entity_id: r.opt_u64()?,
                speed: r.f32()?,
            },
            72 => C::SeekAnimation {
                entity_id: r.opt_u64()?,
                time: r.f32()?,
            },
            73 => C::CrossfadeAnimation {
                entity_id: r.opt_u64()?,
                name: r.string()?,
                duration: r.f32()?,
                looping: r.bool()?,
            },
            74 => C::SetAnimationParam {
                entity_id: r.opt_u64()?,
                name: r.string()?,
                value: r.f32()?,
            },
            75 => C::SetAnimationBoolParam {
                entity_id: r.opt_u64()?,
                name: r.string()?,
                value: r.bool()?,
            },
            76 => C::TriggerAnimation {
                entity_id: r.opt_u64()?,
                name: r.string()?,
            },
            77 => C::SetAnimationLayerWeight {
                entity_id: r.opt_u64()?,
                layer_name: r.string()?,
                weight: r.f32()?,
            },
            78 => C::PlaySpriteAnimation {
                entity_id: r.opt_u64()?,
                name: r.string()?,
                looping: r.bool()?,
            },
            79 => C::SetSpriteFrame {
                entity_id: r.opt_u64()?,
                frame: r.i64()?,
            },
            80 => C::Tween {
                entity_id: r.opt_u64()?,
                property: r.string()?,
                target: r.f32()?,
                duration: r.f32()?,
                easing: r.string()?,
            },
            81 => C::TweenPosition {
                entity_id: r.opt_u64()?,
                target: r.f32x3()?,
                duration: r.f32()?,
                easing: r.string()?,
            },
            82 => C::TweenRotation {
                entity_id: r.opt_u64()?,
                target: r.f32x3()?,
                duration: r.f32()?,
                easing: r.string()?,
            },
            83 => C::TweenScale {
                entity_id: r.opt_u64()?,
                target: r.f32x3()?,
                duration: r.f32()?,
                easing: r.string()?,
            },
            84 => C::SetVisibility {
                entity_id: r.opt_u64()?,
                visible: r.bool()?,
            },
            85 => C::SetMaterialColor {
                entity_id: r.opt_u64()?,
                color: r.f32x4()?,
            },
            86 => C::SetLightIntensity {
                entity_id: r.opt_u64()?,
                intensity: r.f32()?,
            },
            87 => C::SetLightColor {
                entity_id: r.opt_u64()?,
                color: r.f32x3()?,
            },
            88 => C::LockCursor,
            89 => C::UnlockCursor,
            90 => C::SetCameraTarget { position: r.f32x3()? },
            91 => C::SetCameraZoom { zoom: r.f32()? },
            92 => C::ScreenShake {
                intensity: r.f32()?,
                duration: r.f32()?,
            },
            93 => C::CameraFollow {
                entity_id: r.u64()?,
                offset: r.f32x3()?,
                smoothing: r.f32()?,
            },
            94 => C::StopCameraFollow,
            95 => C::SetHealth {
                entity_id: r.opt_u64()?,
                value: r.f32()?,
            },
            96 => C::SetMaxHealth {
                entity_id: r.opt_u64()?,
                value: r.f32()?,
            },
            97 => C::Damage {
                entity_id: r.opt_u64()?,
                amount: r.f32()?,
            },
            98 => C::Heal {
                entity_id: r.opt_u64()?,
                amount: r.f32()?,
            },
            99 => C::SetInvincible {
                entity_id: r.opt_u64()?,
                invincible: r.bool()?,
                duration: r.f32()?,
            },
            100 => C::Kill { entity_id: r.opt_u64()? },
            101 => C::Revive { entity_id: r.opt_u64()? },
            102 => C::ParticlePlay { entity_id: r.u64()? },
            103 => C::ParticlePause { entity_id: r.u64()? },
            104 => C::ParticleStop { entity_id: r.u64()? },
            105 => C::ParticleReset { entity_id: r.u64()? },
            106 => C::ParticleBurst {
                entity_id: r.u64()?,
                count: r.u32()?,
            },
            107 => C::ParticleSetRate {
                entity_id: r.u64()?,
                multiplier: r.f32()?,
            },
            108 => C::ParticleSetScale {
                entity_id: r.u64()?,
                multiplier: r.f32()?,
            },
            109 => C::ParticleSetTimeScale {
                entity_id: r.u64()?,
                scale: r.f32()?,
            },
            110 => C::ParticleSetTint {
                entity_id: r.u64()?,
                r: r.f32()?,
                g: r.f32()?,
                b: r.f32()?,
                a: r.f32()?,
            },
            111 => C::SetProperty {
                entity_id: r.u64()?,
                property: r.string()?,
                value: PropValue::decode(r)?,
            },
            112 => C::SetComponentField {
                entity_id: r.opt_u64()?,
                entity_name: r.opt_string()?,
                component_type: r.string()?,
                field_path: r.string()?,
                value: PropValue::decode(r)?,
            },
            113 => C::Action {
                name: r.string()?,
                target_entity: r.opt_string()?,
                args: r.list(|r| Ok((r.string()?, ActionValue::decode(r)?)))?,
            },
            114 => C::HttpRequest {
                method: r.string()?,
                url: r.string()?,
                body: r.opt_string()?,
                callback: r.string()?,
            },
            115 => C::Emit {
                name: r.string()?,
                args: r.list(|r| Ok((r.string()?, ActionValue::decode(r)?)))?,
            },
            t => return Err(WireError::UnknownTag(t as u32)),
        })
    }
}

fn encode_opt_f32x3(w: &mut Writer, v: &Option<[f32; 3]>) {
    match v {
        Some(v) => {
            w.bool(true);
            w.f32x3(*v);
        }
        None => w.bool(false),
    }
}

fn decode_opt_f32x3(r: &mut Reader) -> Result<Option<[f32; 3]>, WireError> {
    if r.bool()? {
        Ok(Some(r.f32x3()?))
    } else {
        Ok(None)
    }
}

/// Encode a batch, length-prefixed.
pub fn encode_list(w: &mut Writer, cmds: &[ScriptCommand]) {
    w.count(cmds.len());
    for c in cmds {
        c.encode(w);
    }
}

/// Decode a batch.
///
/// One bad command aborts the whole batch, and that is the only honest option:
/// a command's fields have no length prefix of their own, so a decoder that
/// hits an unknown tag cannot find where the next command starts. Returning
/// what was decoded so far would silently apply half a frame's intent — a
/// script that says "stop the car, then open the door" would open the door of a
/// moving car. Better to drop the batch and log it.
pub fn decode_list(r: &mut Reader) -> Result<Vec<ScriptCommand>, WireError> {
    r.list(ScriptCommand::decode)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One instance of every variant, in tag order.
    ///
    /// The point of building this by hand is that it fails to compile when a
    /// variant is added — non-exhaustive matches elsewhere would too, but this
    /// is the one that makes the *round trip* get tested rather than merely
    /// the enum being complete.
    fn all_variants() -> Vec<ScriptCommand> {
        use ScriptCommand as C;
        let v3 = [1.0, 2.0, 3.0];
        let v4 = [1.0, 2.0, 3.0, 4.0];
        let e = Some(7u64);
        vec![
            C::SetPosition { x: 1.0, y: 2.0, z: 3.0 },
            C::SetRotation { x: 1.0, y: 2.0, z: 3.0 },
            C::SetScale { x: 1.0, y: 2.0, z: 3.0 },
            C::Translate { x: 1.0, y: 2.0, z: 3.0 },
            C::Rotate { x: 1.0, y: 2.0, z: 3.0 },
            C::LookAt { x: 1.0, y: 2.0, z: 3.0 },
            C::GotoCameraPreset { name: "front".into() },
            C::ParentSetPosition { x: 1.0, y: 2.0, z: 3.0 },
            C::ParentSetRotation { x: 1.0, y: 2.0, z: 3.0 },
            C::ParentTranslate { x: 1.0, y: 2.0, z: 3.0 },
            C::ChildSetPosition { name: "arm".into(), x: 1.0, y: 2.0, z: 3.0 },
            C::ChildSetRotation { name: "arm".into(), x: 1.0, y: 2.0, z: 3.0 },
            C::ChildTranslate { name: "arm".into(), x: 1.0, y: 2.0, z: 3.0 },
            C::SetSunAngles { azimuth: 10.0, elevation: 20.0 },
            C::SetAmbientBrightness { brightness: 0.5 },
            C::SetAmbientColor { r: 1.0, g: 2.0, b: 3.0 },
            C::SetSkyTopColor { r: 1.0, g: 2.0, b: 3.0 },
            C::SetSkyHorizonColor { r: 1.0, g: 2.0, b: 3.0 },
            C::SetFog { enabled: true, start: 1.0, end: 2.0 },
            C::SetFogColor { r: 1.0, g: 2.0, b: 3.0 },
            C::SetEv100 { value: 9.0 },
            C::SpawnEntity { name: "box".into() },
            C::SpawnPrimitive {
                name: "box".into(),
                primitive_type: "cube".into(),
                position: Some(v3),
                scale: None,
                color: Some(v4),
            },
            C::DespawnEntity { entity_id: 3 },
            C::DespawnSelf,
            C::DespawnByPrefix { prefix: "chunk_".into() },
            C::SetEntityName { entity_id: 3, name: "n".into() },
            C::AddTag { entity_id: e, tag: "enemy".into() },
            C::RemoveTag { entity_id: None, tag: "enemy".into() },
            C::PlaySound {
                path: "a.ogg".into(),
                volume: 1.0,
                looping: true,
                bus: "sfx".into(),
            },
            C::PlaySound3D {
                path: "a.ogg".into(),
                volume: 1.0,
                position: v3,
                bus: "sfx".into(),
            },
            C::PlayMusic {
                path: "m.ogg".into(),
                volume: 1.0,
                fade_in: 2.0,
                bus: "music".into(),
            },
            C::StopMusic { fade_out: 1.0 },
            C::StopAllSounds,
            C::SetMasterVolume { volume: 0.5 },
            C::PauseSound,
            C::PauseSoundEntity { entity_id: 3 },
            C::ResumeSound,
            C::ResumeSoundEntity { entity_id: 3 },
            C::SetSoundVolume { volume: 1.0, fade: 2.0 },
            C::SetSoundVolumeEntity { entity_id: 3, volume: 1.0, fade: 2.0 },
            C::SetSoundPitch { pitch: 1.0, fade: 2.0 },
            C::SetSoundPitchEntity { entity_id: 3, pitch: 1.0, fade: 2.0 },
            C::CrossfadeMusic {
                path: "m.ogg".into(),
                volume: 1.0,
                duration: 2.0,
                bus: "music".into(),
            },
            C::Log { level: "info".into(), message: "hi".into() },
            C::DrawLine { start: v3, end: v3, color: v4, duration: 1.0 },
            C::DrawRay {
                origin: v3,
                direction: v3,
                length: 5.0,
                color: v4,
                duration: 1.0,
            },
            C::DrawSphere { center: v3, radius: 1.0, color: v4, duration: 1.0 },
            C::DrawBox {
                center: v3,
                half_extents: v3,
                color: v4,
                duration: 1.0,
            },
            C::DrawPoint { position: v3, size: 1.0, color: v4, duration: 1.0 },
            C::ApplyForce { entity_id: e, force: v3 },
            C::ApplyImpulse { entity_id: e, impulse: v3 },
            C::ApplyTorque { entity_id: e, torque: v3 },
            C::SetVelocity { entity_id: e, velocity: v3 },
            C::SetAngularVelocity { entity_id: None, velocity: v3 },
            C::SetGravityScale { entity_id: e, scale: 2.0 },
            C::Raycast {
                origin: v3,
                direction: v3,
                max_distance: 100.0,
                result_var: "hit".into(),
            },
            C::CharacterMove { direction: [1.0, 2.0] },
            C::CharacterJump,
            C::CharacterSprint { sprinting: true },
            C::StartTimer { name: "t".into(), duration: 1.0, repeat: true },
            C::StopTimer { name: "t".into() },
            C::PauseTimer { name: "t".into() },
            C::ResumeTimer { name: "t".into() },
            C::LoadScene { path: "s.scn".into() },
            C::UnloadScene { handle_id: 3 },
            C::SpawnPrefab { path: "p".into(), position: v3, rotation: v3 },
            C::PlayAnimation {
                entity_id: e,
                name: "run".into(),
                looping: true,
                speed: 1.0,
            },
            C::StopAnimation { entity_id: e },
            C::PauseAnimation { entity_id: e },
            C::ResumeAnimation { entity_id: None },
            C::SetAnimationSpeed { entity_id: e, speed: 2.0 },
            C::SeekAnimation { entity_id: e, time: 0.5 },
            C::CrossfadeAnimation {
                entity_id: e,
                name: "walk".into(),
                duration: 0.2,
                looping: true,
            },
            C::SetAnimationParam { entity_id: e, name: "speed".into(), value: 1.0 },
            C::SetAnimationBoolParam {
                entity_id: e,
                name: "grounded".into(),
                value: true,
            },
            C::TriggerAnimation { entity_id: e, name: "jump".into() },
            C::SetAnimationLayerWeight {
                entity_id: e,
                layer_name: "upper".into(),
                weight: 0.5,
            },
            C::PlaySpriteAnimation {
                entity_id: e,
                name: "idle".into(),
                looping: true,
            },
            C::SetSpriteFrame { entity_id: e, frame: 4 },
            C::Tween {
                entity_id: e,
                property: "x".into(),
                target: 1.0,
                duration: 2.0,
                easing: "linear".into(),
            },
            C::TweenPosition {
                entity_id: e,
                target: v3,
                duration: 2.0,
                easing: "linear".into(),
            },
            C::TweenRotation {
                entity_id: e,
                target: v3,
                duration: 2.0,
                easing: "linear".into(),
            },
            C::TweenScale {
                entity_id: None,
                target: v3,
                duration: 2.0,
                easing: "ease".into(),
            },
            C::SetVisibility { entity_id: e, visible: false },
            C::SetMaterialColor { entity_id: e, color: v4 },
            C::SetLightIntensity { entity_id: e, intensity: 100.0 },
            C::SetLightColor { entity_id: e, color: v3 },
            C::LockCursor,
            C::UnlockCursor,
            C::SetCameraTarget { position: v3 },
            C::SetCameraZoom { zoom: 2.0 },
            C::ScreenShake { intensity: 1.0, duration: 0.5 },
            C::CameraFollow { entity_id: 3, offset: v3, smoothing: 0.1 },
            C::StopCameraFollow,
            C::SetHealth { entity_id: e, value: 50.0 },
            C::SetMaxHealth { entity_id: e, value: 100.0 },
            C::Damage { entity_id: e, amount: 10.0 },
            C::Heal { entity_id: e, amount: 10.0 },
            C::SetInvincible { entity_id: e, invincible: true, duration: 2.0 },
            C::Kill { entity_id: e },
            C::Revive { entity_id: None },
            C::ParticlePlay { entity_id: 3 },
            C::ParticlePause { entity_id: 3 },
            C::ParticleStop { entity_id: 3 },
            C::ParticleReset { entity_id: 3 },
            C::ParticleBurst { entity_id: 3, count: 50 },
            C::ParticleSetRate { entity_id: 3, multiplier: 2.0 },
            C::ParticleSetScale { entity_id: 3, multiplier: 2.0 },
            C::ParticleSetTimeScale { entity_id: 3, scale: 0.5 },
            C::ParticleSetTint { entity_id: 3, r: 1.0, g: 2.0, b: 3.0, a: 4.0 },
            C::SetProperty {
                entity_id: 3,
                property: "hp".into(),
                value: PropValue::Float(1.0),
            },
            C::SetComponentField {
                entity_id: e,
                entity_name: Some("Sun".into()),
                component_type: "Sun".into(),
                field_path: "elevation".into(),
                value: PropValue::Color([1.0, 2.0, 3.0, 4.0]),
            },
            C::Action {
                name: "apply_force".into(),
                target_entity: Some("Player".into()),
                args: vec![
                    ("x".into(), ActionValue::Float(1.0)),
                    ("tag".into(), ActionValue::String("s".into())),
                ],
            },
            C::HttpRequest {
                method: "POST".into(),
                url: "https://example.test".into(),
                body: Some("{}".into()),
                callback: "on_done".into(),
            },
            C::Emit {
                name: "door_opened".into(),
                args: vec![
                    ("id".into(), ActionValue::Float(3.0)),
                    ("by".into(), ActionValue::String("Player".into())),
                ],
            },
        ]
    }

    #[test]
    fn the_sample_covers_every_variant_exactly_once_in_tag_order() {
        let all = all_variants();
        assert_eq!(
            all.len(),
            VARIANT_COUNT as usize,
            "all_variants() is out of step with VARIANT_COUNT — a variant was \
             added without a sample, so its round trip is untested"
        );
        for (i, c) in all.iter().enumerate() {
            assert_eq!(c.tag(), i as u16, "variant {c:?} is not at its own tag");
        }
    }

    #[test]
    fn every_variant_round_trips() {
        for c in all_variants() {
            let mut w = Writer::new();
            c.encode(&mut w);
            let bytes = w.into_bytes();
            let mut r = Reader::new(&bytes);
            let back = ScriptCommand::decode(&mut r).unwrap();
            assert_eq!(back, c);
            assert_eq!(
                r.remaining(),
                0,
                "decoder read fewer bytes than encoder wrote for {c:?} — the two \
                 arms disagree about this variant's fields"
            );
        }
    }

    #[test]
    fn a_batch_round_trips() {
        let all = all_variants();
        let mut w = Writer::new();
        encode_list(&mut w, &all);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(decode_list(&mut r).unwrap(), all);
        assert_eq!(r.remaining(), 0);
    }

    #[test]
    fn a_tag_from_a_newer_plugin_is_refused() {
        let mut w = Writer::new();
        w.u16(VARIANT_COUNT);
        let bytes = w.into_bytes();
        let mut r = Reader::new(&bytes);
        assert_eq!(
            ScriptCommand::decode(&mut r),
            Err(WireError::UnknownTag(VARIANT_COUNT as u32))
        );
    }
}
