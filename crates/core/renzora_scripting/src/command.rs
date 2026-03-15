use bevy::prelude::*;

use crate::extension::ScriptExtensionCommand;

/// Commands that scripts can issue, processed after execution.
/// Language-agnostic — all backends produce these same commands.
#[derive(Debug)]
pub enum ScriptCommand {
    // === Self-Transform ===
    SetPosition { x: f32, y: f32, z: f32 },
    SetRotation { x: f32, y: f32, z: f32 },
    SetScale { x: f32, y: f32, z: f32 },
    Translate { x: f32, y: f32, z: f32 },
    Rotate { x: f32, y: f32, z: f32 },
    LookAt { x: f32, y: f32, z: f32 },

    // === Parent Transform ===
    ParentSetPosition { x: f32, y: f32, z: f32 },
    ParentSetRotation { x: f32, y: f32, z: f32 },
    ParentTranslate { x: f32, y: f32, z: f32 },

    // === Child Transform ===
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
    SpawnEntity { name: String },
    SpawnPrimitive { name: String, primitive_type: String, position: Option<Vec3>, scale: Option<Vec3> },
    DespawnEntity { entity_id: u64 },
    DespawnSelf,
    SetEntityName { entity_id: u64, name: String },
    AddTag { entity_id: Option<u64>, tag: String },
    RemoveTag { entity_id: Option<u64>, tag: String },

    // === Audio ===
    PlaySound { path: String, volume: f32, looping: bool, bus: String },
    PlaySound3D { path: String, volume: f32, position: Vec3, bus: String },
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

    // === Debug ===
    Log { level: String, message: String },
    DrawLine { start: Vec3, end: Vec3, color: [f32; 4], duration: f32 },
    DrawRay { origin: Vec3, direction: Vec3, length: f32, color: [f32; 4], duration: f32 },
    DrawSphere { center: Vec3, radius: f32, color: [f32; 4], duration: f32 },
    DrawBox { center: Vec3, half_extents: Vec3, color: [f32; 4], duration: f32 },
    DrawPoint { position: Vec3, size: f32, color: [f32; 4], duration: f32 },

    // === Physics ===
    ApplyForce { entity_id: Option<u64>, force: Vec3 },
    ApplyImpulse { entity_id: Option<u64>, impulse: Vec3 },
    ApplyTorque { entity_id: Option<u64>, torque: Vec3 },
    SetVelocity { entity_id: Option<u64>, velocity: Vec3 },
    SetAngularVelocity { entity_id: Option<u64>, velocity: Vec3 },
    SetGravityScale { entity_id: Option<u64>, scale: f32 },
    Raycast { origin: Vec3, direction: Vec3, max_distance: f32, result_var: String },

    // === Timers ===
    StartTimer { name: String, duration: f32, repeat: bool },
    StopTimer { name: String },
    PauseTimer { name: String },
    ResumeTimer { name: String },

    // === Scene ===
    LoadScene { path: String },
    UnloadScene { handle_id: u64 },
    SpawnPrefab { path: String, position: Vec3, rotation: Vec3 },

    // === Animation ===
    PlayAnimation { entity_id: Option<u64>, name: String, looping: bool, speed: f32 },
    StopAnimation { entity_id: Option<u64> },
    PauseAnimation { entity_id: Option<u64> },
    ResumeAnimation { entity_id: Option<u64> },
    SetAnimationSpeed { entity_id: Option<u64>, speed: f32 },
    CrossfadeAnimation { entity_id: Option<u64>, name: String, duration: f32, looping: bool },
    SetAnimationParam { entity_id: Option<u64>, name: String, value: f32 },
    SetAnimationBoolParam { entity_id: Option<u64>, name: String, value: bool },
    TriggerAnimation { entity_id: Option<u64>, name: String },
    SetAnimationLayerWeight { entity_id: Option<u64>, layer_name: String, weight: f32 },

    // === Sprite Animation ===
    PlaySpriteAnimation { entity_id: Option<u64>, name: String, looping: bool },
    SetSpriteFrame { entity_id: Option<u64>, frame: i64 },

    // === Tweens ===
    Tween { entity_id: Option<u64>, property: String, target: f32, duration: f32, easing: String },
    TweenPosition { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },
    TweenRotation { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },
    TweenScale { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },

    // === Rendering ===
    SetVisibility { entity_id: Option<u64>, visible: bool },
    SetMaterialColor { entity_id: Option<u64>, color: [f32; 4] },
    SetLightIntensity { entity_id: Option<u64>, intensity: f32 },
    SetLightColor { entity_id: Option<u64>, color: [f32; 3] },

    // === Camera ===
    SetCameraTarget { position: Vec3 },
    SetCameraZoom { zoom: f32 },
    ScreenShake { intensity: f32, duration: f32 },
    CameraFollow { entity_id: u64, offset: Vec3, smoothing: f32 },
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
    SetProperty { entity_id: u64, property: String, value: PropertyValue },

    // === Generic Reflection ===
    /// Set any reflected component field by path.
    /// `component_type` is the short type name (e.g. "Sun").
    /// `field_path` is dot-separated (e.g. "elevation" or "color.x").
    SetComponentField {
        entity_id: Option<u64>,
        entity_name: Option<String>,
        component_type: String,
        field_path: String,
        value: PropertyValue,
    },

    // === Extension ===
    /// Custom command from a script extension. Downcasted by the extension's
    /// command processor via `as_any()`.
    Extension(Box<dyn ScriptExtensionCommand>),
}

/// Value types for property writes
#[derive(Clone, Debug)]
pub enum PropertyValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}
