use bevy::prelude::*;

/// Gravity preset identifiers
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GravityPreset {
    #[default]
    Earth,
    Moon,
    Mars,
    Jupiter,
    ZeroG,
    Custom,
}

impl GravityPreset {
    pub fn label(&self) -> &'static str {
        match self {
            GravityPreset::Earth => "Earth",
            GravityPreset::Moon => "Moon",
            GravityPreset::Mars => "Mars",
            GravityPreset::Jupiter => "Jupiter",
            GravityPreset::ZeroG => "Zero-G",
            GravityPreset::Custom => "Custom",
        }
    }

    pub fn gravity_vec(&self) -> Vec3 {
        match self {
            GravityPreset::Earth => Vec3::new(0.0, -9.81, 0.0),
            GravityPreset::Moon => Vec3::new(0.0, -1.62, 0.0),
            GravityPreset::Mars => Vec3::new(0.0, -3.72, 0.0),
            GravityPreset::Jupiter => Vec3::new(0.0, -24.79, 0.0),
            GravityPreset::ZeroG => Vec3::ZERO,
            GravityPreset::Custom => Vec3::new(0.0, -9.81, 0.0),
        }
    }

    pub const ALL: &'static [GravityPreset] = &[
        GravityPreset::Earth,
        GravityPreset::Moon,
        GravityPreset::Mars,
        GravityPreset::Jupiter,
        GravityPreset::ZeroG,
        GravityPreset::Custom,
    ];
}

#[derive(Clone, Debug)]
pub enum PhysicsPropertyCommand {
    SetGravity(Vec3),
    SetGravityPreset(GravityPreset),
    SetTimeScale(f32),
    SetSubsteps(u32),
    ResetAll,
}

#[derive(Resource)]
pub struct PhysicsPropertiesState {
    pub gravity: Vec3,
    pub gravity_preset: GravityPreset,
    pub time_scale: f32,
    pub substeps: u32,
    pub commands: Vec<PhysicsPropertyCommand>,
    pub physics_available: bool,
}

impl Default for PhysicsPropertiesState {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            gravity_preset: GravityPreset::Earth,
            time_scale: 1.0,
            substeps: 6,
            commands: Vec::new(),
            physics_available: true,
        }
    }
}
