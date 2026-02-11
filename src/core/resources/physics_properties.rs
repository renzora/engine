//! Physics properties state for global simulation settings

use bevy::prelude::*;
use avian3d::schedule::PhysicsTime;

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

/// Commands for modifying global physics properties
#[derive(Clone, Debug)]
pub enum PhysicsPropertyCommand {
    SetGravity(Vec3),
    SetGravityPreset(GravityPreset),
    SetTimeScale(f32),
    SetSubsteps(u32),
    ResetAll,
}

/// State resource for the Physics Properties panel
#[derive(Resource)]
pub struct PhysicsPropertiesState {
    /// Current gravity vector (synced from Avian)
    pub gravity: Vec3,
    /// Active gravity preset
    pub gravity_preset: GravityPreset,
    /// Time scale (1.0 = normal)
    pub time_scale: f32,
    /// Number of solver substeps
    pub substeps: u32,
    /// Pending commands from the UI
    pub commands: Vec<PhysicsPropertyCommand>,
    /// Whether physics is available
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

/// System that syncs physics properties between UI state and Avian resources
pub fn sync_physics_properties(
    mut state: ResMut<PhysicsPropertiesState>,
    mut gravity: ResMut<avian3d::prelude::Gravity>,
    mut time: ResMut<Time<avian3d::prelude::Physics>>,
    mut substeps: ResMut<avian3d::prelude::SubstepCount>,
) {
    // Process commands from UI
    let cmds: Vec<PhysicsPropertyCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            PhysicsPropertyCommand::SetGravity(g) => {
                gravity.0 = g;
                state.gravity = g;
                state.gravity_preset = GravityPreset::Custom;
            }
            PhysicsPropertyCommand::SetGravityPreset(preset) => {
                let g = preset.gravity_vec();
                gravity.0 = g;
                state.gravity = g;
                state.gravity_preset = preset;
            }
            PhysicsPropertyCommand::SetTimeScale(scale) => {
                time.set_relative_speed(scale);
                state.time_scale = scale;
            }
            PhysicsPropertyCommand::SetSubsteps(count) => {
                substeps.0 = count;
                state.substeps = count;
            }
            PhysicsPropertyCommand::ResetAll => {
                let default_g = GravityPreset::Earth.gravity_vec();
                gravity.0 = default_g;
                state.gravity = default_g;
                state.gravity_preset = GravityPreset::Earth;
                time.set_relative_speed(1.0);
                state.time_scale = 1.0;
                substeps.0 = 6;
                state.substeps = 6;
            }
        }
    }

    // Read back current values from Avian (in case something else changed them)
    if state.commands.is_empty() {
        state.gravity = gravity.0;
        state.substeps = substeps.0;
    }
}
