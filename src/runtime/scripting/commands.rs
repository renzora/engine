//! Rhai script commands for the runtime

use bevy::prelude::*;

/// Commands that scripts can issue
#[derive(Clone, Debug)]
pub enum RhaiCommand {
    // Logging
    Log { message: String },

    // Entity management
    SpawnEntity { name: String },
    DespawnEntity { entity_id: u64 },
    SpawnPrimitive { name: String, primitive_type: String, position: Option<Vec3> },

    // Transform
    SetPosition { entity_id: u64, position: Vec3 },
    SetRotation { entity_id: u64, rotation: Quat },
    SetScale { entity_id: u64, scale: Vec3 },
    Translate { entity_id: u64, delta: Vec3 },
    Rotate { entity_id: u64, rotation: Quat },
    LookAt { entity_id: u64, target: Vec3 },

    // Physics
    ApplyForce { entity_id: u64, force: Vec3 },
    ApplyImpulse { entity_id: u64, impulse: Vec3 },
    ApplyTorque { entity_id: u64, torque: Vec3 },
    SetVelocity { entity_id: u64, velocity: Vec3 },
    SetAngularVelocity { entity_id: u64, velocity: Vec3 },
    SetGravityScale { entity_id: u64, scale: f32 },
    Raycast { origin: Vec3, direction: Vec3, max_distance: f32, result_var: String },

    // Audio
    PlaySound { path: String, volume: f32, looping: bool },
    PlaySound3D { path: String, position: Vec3, volume: f32, looping: bool },
    PlayMusic { path: String, volume: f32, fade_in: f32 },
    StopMusic { fade_out: f32 },
    SetMasterVolume { volume: f32 },
    StopAllSounds,

    // Timers
    StartTimer { name: String, duration: f32, repeat: bool },
    StopTimer { name: String },
    PauseTimer { name: String },
    ResumeTimer { name: String },

    // Debug drawing
    DrawLine { start: Vec3, end: Vec3, color: [f32; 4], duration: f32 },
    DrawSphere { center: Vec3, radius: f32, color: [f32; 4], duration: f32 },
    DrawBox { center: Vec3, half_extents: Vec3, color: [f32; 4], duration: f32 },
    DrawRay { origin: Vec3, direction: Vec3, color: [f32; 4], duration: f32 },
    DrawPoint { position: Vec3, color: [f32; 4], duration: f32 },

    // Rendering
    SetMaterialColor { entity_id: u64, color: [f32; 4] },
    SetLightIntensity { entity_id: u64, intensity: f32 },
    SetLightColor { entity_id: u64, color: [f32; 4] },
    SetVisibility { entity_id: u64, visible: bool },

    // Camera
    SetCameraTarget { target_entity_id: Option<u64> },
    SetCameraZoom { zoom: f32 },
    ScreenShake { intensity: f32, duration: f32 },
    SetCameraOffset { offset: Vec3 },

    // Animation
    PlayAnimation { entity_id: u64, clip_name: String, looping: bool, speed: f32 },
    StopAnimation { entity_id: u64 },
    PauseAnimation { entity_id: u64 },
    ResumeAnimation { entity_id: u64 },
    SetAnimationSpeed { entity_id: u64, speed: f32 },

    // Health
    Damage { entity_id: u64, amount: f32 },
    Heal { entity_id: u64, amount: f32 },
    SetHealth { entity_id: u64, amount: f32 },
    SetMaxHealth { entity_id: u64, amount: f32 },
    SetInvincible { entity_id: u64, invincible: bool, duration: f32 },
    Kill { entity_id: u64 },
    Revive { entity_id: u64 },

    // Tweening
    TweenTo { entity_id: u64, property: String, target: Vec3, duration: f32, easing: String },

    // Entity properties
    SetName { entity_id: u64, name: String },
    AddTag { entity_id: u64, tag: String },
    RemoveTag { entity_id: u64, tag: String },

    // Scene management
    LoadScene { path: String },
    UnloadScene { path: String },
    SpawnPrefab { path: String, position: Vec3, rotation: Vec3 },

    // Environment
    SetEnvironment { property: String, value: Vec3 },

    // Component access
    SetComponent { entity_id: u64, component_type: String, property: String, value: String },

    // Sprite animation
    PlaySpriteAnimation { entity_id: u64, animation_name: String, looping: bool, speed: f32 },
    SetSpriteFrame { entity_id: u64, frame: usize },
}
