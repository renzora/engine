//! Physics forces state for interactive force application

use bevy::prelude::*;
use bevy::ecs::system::ParamSet;
use avian3d::prelude::RigidBodyForces;

/// Force application mode
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ForceMode {
    #[default]
    Force,
    Impulse,
    Torque,
    VelocityOverride,
}

impl ForceMode {
    pub fn label(&self) -> &'static str {
        match self {
            ForceMode::Force => "Force",
            ForceMode::Impulse => "Impulse",
            ForceMode::Torque => "Torque",
            ForceMode::VelocityOverride => "Velocity",
        }
    }

    pub const ALL: &'static [ForceMode] = &[
        ForceMode::Force,
        ForceMode::Impulse,
        ForceMode::Torque,
        ForceMode::VelocityOverride,
    ];
}

/// Direction preset for force application
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectionPreset {
    Up,
    Down,
    Left,
    Right,
    Forward,
    Back,
    Custom,
}

impl DirectionPreset {
    pub fn to_vec3(&self) -> Vec3 {
        match self {
            DirectionPreset::Up => Vec3::Y,
            DirectionPreset::Down => Vec3::NEG_Y,
            DirectionPreset::Left => Vec3::NEG_X,
            DirectionPreset::Right => Vec3::X,
            DirectionPreset::Forward => Vec3::NEG_Z,
            DirectionPreset::Back => Vec3::Z,
            DirectionPreset::Custom => Vec3::Y,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            DirectionPreset::Up => "Up",
            DirectionPreset::Down => "Down",
            DirectionPreset::Left => "Left",
            DirectionPreset::Right => "Right",
            DirectionPreset::Forward => "Fwd",
            DirectionPreset::Back => "Back",
            DirectionPreset::Custom => "Custom",
        }
    }
}

/// Commands from the forces UI
#[derive(Clone, Debug)]
pub enum ForceCommand {
    /// Apply force/impulse/torque to the selected entity
    Apply {
        entity: Entity,
        mode: ForceMode,
        direction: Vec3,
        magnitude: f32,
    },
    /// Explosion: apply radial impulse to all dynamic bodies within radius
    Explosion {
        origin: Vec3,
        radius: f32,
        magnitude: f32,
    },
    /// Set velocity directly
    SetVelocity {
        entity: Entity,
        linear: Vec3,
        angular: Vec3,
    },
    /// Zero out all motion on entity
    ZeroMotion {
        entity: Entity,
    },
}

/// State resource for the Forces & Impulses panel
#[derive(Resource)]
pub struct PhysicsForcesState {
    /// Current force mode
    pub mode: ForceMode,
    /// Direction preset
    pub direction_preset: DirectionPreset,
    /// Custom direction vector
    pub custom_direction: Vec3,
    /// Force/impulse magnitude
    pub magnitude: f32,
    /// Explosion radius
    pub explosion_radius: f32,
    /// Explosion magnitude
    pub explosion_magnitude: f32,
    /// Velocity override values
    pub velocity_linear: Vec3,
    pub velocity_angular: Vec3,
    /// Currently selected entity (read from selection state)
    pub selected_entity: Option<Entity>,
    /// Whether selected entity has a rigid body
    pub selected_has_rigidbody: bool,
    /// Selected entity's current linear velocity
    pub selected_linear_velocity: Vec3,
    /// Selected entity's current angular velocity
    pub selected_angular_velocity: Vec3,
    /// Pending commands
    pub commands: Vec<ForceCommand>,
}

impl Default for PhysicsForcesState {
    fn default() -> Self {
        Self {
            mode: ForceMode::Impulse,
            direction_preset: DirectionPreset::Up,
            custom_direction: Vec3::Y,
            magnitude: 10.0,
            explosion_radius: 10.0,
            explosion_magnitude: 20.0,
            velocity_linear: Vec3::ZERO,
            velocity_angular: Vec3::ZERO,
            selected_entity: None,
            selected_has_rigidbody: false,
            selected_linear_velocity: Vec3::ZERO,
            selected_angular_velocity: Vec3::ZERO,
            commands: Vec::new(),
        }
    }
}

/// System that reads the current selection into the forces panel state
pub fn update_forces_panel_state(
    mut state: ResMut<PhysicsForcesState>,
    selection: Res<crate::core::SelectionState>,
    rigid_bodies: Query<&avian3d::prelude::RigidBody>,
    linear_vels: Query<&avian3d::prelude::LinearVelocity>,
    angular_vels: Query<&avian3d::prelude::AngularVelocity>,
) {
    let selected = selection.selected_entity;
    state.selected_entity = selected;

    if let Some(entity) = selected {
        state.selected_has_rigidbody = rigid_bodies.get(entity).is_ok();
        state.selected_linear_velocity = linear_vels
            .get(entity)
            .map(|v| v.0)
            .unwrap_or(Vec3::ZERO);
        state.selected_angular_velocity = angular_vels
            .get(entity)
            .map(|v| v.0)
            .unwrap_or(Vec3::ZERO);
    } else {
        state.selected_has_rigidbody = false;
        state.selected_linear_velocity = Vec3::ZERO;
        state.selected_angular_velocity = Vec3::ZERO;
    }
}

/// System that processes force commands using Avian's force API
pub fn process_force_commands(
    mut state: ResMut<PhysicsForcesState>,
    mut physics_params: ParamSet<(
        Query<avian3d::prelude::Forces>,
        Query<&mut avian3d::prelude::LinearVelocity>,
        Query<&mut avian3d::prelude::AngularVelocity>,
    )>,
    positions: Query<&Transform>,
    rigid_bodies: Query<Entity, With<avian3d::prelude::RigidBody>>,
) {
    let cmds: Vec<ForceCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() {
        return;
    }

    for cmd in cmds {
        match cmd {
            ForceCommand::Apply { entity, mode, direction, magnitude } => {
                let force_vec = direction.normalize_or_zero() * magnitude;
                match mode {
                    ForceMode::Force => {
                        if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                            forces.apply_force(force_vec);
                        }
                    }
                    ForceMode::Impulse => {
                        if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                            forces.apply_linear_impulse(force_vec);
                        }
                    }
                    ForceMode::Torque => {
                        if let Ok(mut forces) = physics_params.p0().get_mut(entity) {
                            forces.apply_torque(force_vec);
                        }
                    }
                    ForceMode::VelocityOverride => {
                        if let Ok(mut vel) = physics_params.p1().get_mut(entity) {
                            vel.0 = force_vec;
                        }
                    }
                }
            }
            ForceCommand::Explosion { origin, radius, magnitude } => {
                // Collect entities and their positions first
                let targets: Vec<(Entity, Vec3)> = rigid_bodies
                    .iter()
                    .filter_map(|e| {
                        positions.get(e).ok().map(|t| (e, t.translation))
                    })
                    .collect();

                let mut forces_query = physics_params.p0();
                for (entity, pos) in targets {
                    let diff = pos - origin;
                    let dist = diff.length();
                    if dist < radius && dist > 0.001 {
                        let falloff = 1.0 - (dist / radius);
                        let impulse = diff.normalize() * magnitude * falloff;
                        if let Ok(mut forces) = forces_query.get_mut(entity) {
                            forces.apply_linear_impulse(impulse);
                        }
                    }
                }
            }
            ForceCommand::SetVelocity { entity, linear, angular } => {
                if let Ok(mut vel) = physics_params.p1().get_mut(entity) {
                    vel.0 = linear;
                }
                if let Ok(mut vel) = physics_params.p2().get_mut(entity) {
                    vel.0 = angular;
                }
            }
            ForceCommand::ZeroMotion { entity } => {
                if let Ok(mut vel) = physics_params.p1().get_mut(entity) {
                    vel.0 = Vec3::ZERO;
                }
                if let Ok(mut vel) = physics_params.p2().get_mut(entity) {
                    vel.0 = Vec3::ZERO;
                }
            }
        }
    }
}
