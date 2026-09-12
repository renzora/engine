//! The command vocabulary — everything a script can ask the engine to do.
//!
//! A hook returns a list of these and the queue in [`systems::commands`](crate::systems)
//! applies them to the world once the hook has finished. That indirection is
//! the reason a language backend needs no `&mut World`: it describes what it
//! wants, and the engine decides when and how.
//!
//! Language-agnostic by construction — a Lua binding, a Wren binding and the
//! Rust script backend all produce these same values.
//!
//! ## Why the fields are arrays rather than `Vec3`
//!
//! `[f32; 3]` instead of `Vec3`, `[f32; 4]` instead of `Color`. In memory they
//! were always that, and the conversion happens where the commands are
//! *applied*, which is one place per command instead of one per construction
//! site. It also keeps this enum readable as a list of what scripts can do,
//! rather than a list of Bevy types.

pub use renzora::{CharacterCommand, CharacterCommandQueue};

/// The reflected-property value, as every other crate knows it.
pub use renzora::PropertyValue;

/// The argument type for [`ScriptCommand::Action`] and [`ScriptCommand::Emit`],
/// shared with the `renzora::ScriptAction` event those commands fire.
pub use renzora::ScriptActionValue;

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
        value: PropertyValue,
    },

    // === Generic reflection ===
    /// Set any reflected component field by path. `component_type` is the short
    /// type name (e.g. `"Sun"`); `field_path` is dot-separated (`"color.x"`).
    SetComponentField {
        entity_id: Option<u64>,
        entity_name: Option<String>,
        component_type: String,
        field_path: String,
        value: PropertyValue,
    },

    // === Generic script action ===
    /// Fires a `renzora::ScriptAction` event that domain crates observe. This is
    /// what the declarative bindings compile down to, and it is why a domain
    /// crate can add script functions without this enum growing.
    ///
    /// `args` is a list rather than a map: the lists are two or three entries
    /// long, order is deterministic, and the engine builds a map once where it
    /// fires the event.
    Action {
        name: String,
        target_entity: Option<String>,
        args: Vec<(String, ScriptActionValue)>,
    },

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
        args: Vec<(String, ScriptActionValue)>,
    },
}
