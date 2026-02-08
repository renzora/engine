//! Commands that can be issued from Rhai scripts

use bevy::prelude::*;
use crate::component_system::PropertyValue;

/// Commands that can be issued from Rhai scripts
#[derive(Clone, Debug)]
pub enum RhaiCommand {
    // ===================
    // Self-Transform Commands
    // ===================
    SetPosition { x: f32, y: f32, z: f32 },
    SetRotation { x: f32, y: f32, z: f32 },
    SetScale { x: f32, y: f32, z: f32 },
    Translate { x: f32, y: f32, z: f32 },
    Rotate { x: f32, y: f32, z: f32 },
    LookAt { x: f32, y: f32, z: f32 },

    // ===================
    // Parent Transform Commands
    // ===================
    ParentSetPosition { x: f32, y: f32, z: f32 },
    ParentSetRotation { x: f32, y: f32, z: f32 },
    ParentTranslate { x: f32, y: f32, z: f32 },

    // ===================
    // Child Transform Commands
    // ===================
    ChildSetPosition { name: String, x: f32, y: f32, z: f32 },
    ChildSetRotation { name: String, x: f32, y: f32, z: f32 },
    ChildTranslate { name: String, x: f32, y: f32, z: f32 },

    // ===================
    // Environment Commands
    // ===================
    SetSunAngles { azimuth: f32, elevation: f32 },
    SetAmbientBrightness { brightness: f32 },
    SetAmbientColor { r: f32, g: f32, b: f32 },
    SetSkyTopColor { r: f32, g: f32, b: f32 },
    SetSkyHorizonColor { r: f32, g: f32, b: f32 },
    SetFog { enabled: bool, start: f32, end: f32 },
    SetFogColor { r: f32, g: f32, b: f32 },
    SetEv100 { value: f32 },

    // ===================
    // Property Write (cross-entity)
    // ===================
    SetProperty { entity_id: u64, property: String, value: PropertyValue },

    // ===================
    // ECS Commands
    // ===================
    SpawnEntity { name: String },
    /// Spawn a primitive mesh (cube, sphere, plane, cylinder, capsule)
    SpawnPrimitive {
        name: String,
        primitive_type: String,
        position: Option<Vec3>,
        scale: Option<Vec3>,
    },
    DespawnEntity { entity_id: u64 },
    DespawnSelf,
    SetEntityName { entity_id: u64, name: String },
    AddTag { entity_id: Option<u64>, tag: String },
    RemoveTag { entity_id: Option<u64>, tag: String },

    // ===================
    // Audio Commands
    // ===================
    PlaySound { path: String, volume: f32, looping: bool },
    PlaySound3D { path: String, volume: f32, position: Vec3 },
    PlayMusic { path: String, volume: f32, fade_in: f32 },
    StopMusic { fade_out: f32 },
    StopAllSounds,
    SetMasterVolume { volume: f32 },

    // ===================
    // Debug Commands
    // ===================
    Log { level: String, message: String },
    DrawLine { start: Vec3, end: Vec3, color: [f32; 4], duration: f32 },
    DrawRay { origin: Vec3, direction: Vec3, length: f32, color: [f32; 4], duration: f32 },
    DrawSphere { center: Vec3, radius: f32, color: [f32; 4], duration: f32 },
    DrawBox { center: Vec3, half_extents: Vec3, color: [f32; 4], duration: f32 },
    DrawPoint { position: Vec3, size: f32, color: [f32; 4], duration: f32 },

    // ===================
    // Physics Commands
    // ===================
    ApplyForce { entity_id: Option<u64>, force: Vec3 },
    ApplyImpulse { entity_id: Option<u64>, impulse: Vec3 },
    ApplyTorque { entity_id: Option<u64>, torque: Vec3 },
    SetVelocity { entity_id: Option<u64>, velocity: Vec3 },
    SetAngularVelocity { entity_id: Option<u64>, velocity: Vec3 },
    SetGravityScale { entity_id: Option<u64>, scale: f32 },
    Raycast { origin: Vec3, direction: Vec3, max_distance: f32, result_var: String },

    // ===================
    // Timer Commands
    // ===================
    StartTimer { name: String, duration: f32, repeat: bool },
    StopTimer { name: String },
    PauseTimer { name: String },
    ResumeTimer { name: String },

    // ===================
    // Scene Commands
    // ===================
    LoadScene { path: String },
    UnloadScene { handle_id: u64 },
    SpawnPrefab { path: String, position: Vec3, rotation: Vec3 },

    // ===================
    // Animation Commands
    // ===================
    PlayAnimation { entity_id: Option<u64>, name: String, looping: bool, speed: f32 },
    StopAnimation { entity_id: Option<u64> },
    PauseAnimation { entity_id: Option<u64> },
    ResumeAnimation { entity_id: Option<u64> },
    SetAnimationSpeed { entity_id: Option<u64>, speed: f32 },

    // ===================
    // Sprite Animation Commands
    // ===================
    PlaySpriteAnimation { entity_id: Option<u64>, name: String, looping: bool },
    SetSpriteFrame { entity_id: Option<u64>, frame: i64 },

    // ===================
    // Tween Commands
    // ===================
    Tween { entity_id: Option<u64>, property: String, target: f32, duration: f32, easing: String },
    TweenPosition { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },
    TweenRotation { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },
    TweenScale { entity_id: Option<u64>, target: Vec3, duration: f32, easing: String },

    // ===================
    // Rendering Commands
    // ===================
    SetVisibility { entity_id: Option<u64>, visible: bool },
    SetMaterialColor { entity_id: Option<u64>, color: [f32; 4] },
    SetLightIntensity { entity_id: Option<u64>, intensity: f32 },
    SetLightColor { entity_id: Option<u64>, color: [f32; 3] },

    // ===================
    // Camera Commands
    // ===================
    SetCameraTarget { position: Vec3 },
    SetCameraZoom { zoom: f32 },
    ScreenShake { intensity: f32, duration: f32 },
    CameraFollow { entity_id: u64, offset: Vec3, smoothing: f32 },
    StopCameraFollow,

    // ===================
    // Component Commands
    // ===================
    /// Generic set component field
    SetComponentField {
        entity_id: Option<u64>,
        component_type: String,
        field_name: String,
        value: ComponentValue,
    },

    // Health-specific commands
    SetHealth { entity_id: Option<u64>, value: f32 },
    SetMaxHealth { entity_id: Option<u64>, value: f32 },
    Damage { entity_id: Option<u64>, amount: f32 },
    Heal { entity_id: Option<u64>, amount: f32 },
    SetInvincible { entity_id: Option<u64>, invincible: bool, duration: f32 },
    Kill { entity_id: Option<u64> },
    Revive { entity_id: Option<u64> },

    // ===================
    // Particle Commands
    // ===================
    /// Start/resume playing the particle effect
    ParticlePlay { entity_id: u64 },
    /// Pause the particle effect
    ParticlePause { entity_id: u64 },
    /// Stop and reset the particle effect
    ParticleStop { entity_id: u64 },
    /// Reset the effect to initial state
    ParticleReset { entity_id: u64 },
    /// Emit a burst of particles
    ParticleBurst { entity_id: u64, count: u32 },
    /// Set the spawn rate multiplier
    ParticleSetRate { entity_id: u64, multiplier: f32 },
    /// Set the particle size multiplier
    ParticleSetScale { entity_id: u64, multiplier: f32 },
    /// Set the time scale
    ParticleSetTimeScale { entity_id: u64, scale: f32 },
    /// Set the color tint
    ParticleSetTint { entity_id: u64, r: f32, g: f32, b: f32, a: f32 },
    /// Set a custom float variable
    ParticleSetVariableFloat { entity_id: u64, name: String, value: f32 },
    /// Set a custom color variable
    ParticleSetVariableColor { entity_id: u64, name: String, r: f32, g: f32, b: f32, a: f32 },
    /// Set a custom vec3 variable
    ParticleSetVariableVec3 { entity_id: u64, name: String, x: f32, y: f32, z: f32 },
    /// Move emitter and emit at position
    ParticleEmitAt { entity_id: u64, x: f32, y: f32, z: f32, count: Option<u32> },
}

/// Value types for component fields
#[derive(Clone, Debug)]
pub enum ComponentValue {
    Float(f32),
    Int(i64),
    Bool(bool),
    String(String),
    Vec3([f32; 3]),
    Color([f32; 4]),
}
