//! Movement trails global state — master controls and trail rendering

use bevy::prelude::*;
use std::collections::VecDeque;

/// Color mode for trail rendering
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TrailColorMode {
    #[default]
    Solid,
    VelocityHeatmap,
    AgeGradient,
}

impl TrailColorMode {
    pub fn label(&self) -> &'static str {
        match self {
            TrailColorMode::Solid => "Solid",
            TrailColorMode::VelocityHeatmap => "Velocity Heatmap",
            TrailColorMode::AgeGradient => "Age Gradient",
        }
    }

    pub const ALL: &'static [TrailColorMode] = &[
        TrailColorMode::Solid,
        TrailColorMode::VelocityHeatmap,
        TrailColorMode::AgeGradient,
    ];
}

/// A single point in a trail
#[derive(Clone, Debug)]
pub struct TrailPoint {
    pub position: Vec3,
    pub velocity: f32,
    pub timestamp: f32,
}

/// Per-entity movement trail data component
#[derive(Component, Clone, Debug)]
pub struct MovementTrailData {
    /// Whether this trail is actively sampling
    pub enabled: bool,
    /// Maximum number of points to keep
    pub max_points: usize,
    /// Frames between samples
    pub sample_interval: u32,
    /// Color mode
    pub color_mode: TrailColorMode,
    /// Trail color (RGBA) for Solid mode
    pub trail_color: [f32; 4],
    /// Whether to fade alpha over age
    pub fade_alpha: bool,
    /// Trail points (runtime, not serialized)
    pub points: VecDeque<TrailPoint>,
    /// Frame counter for sampling interval
    pub frame_counter: u32,
}

impl Default for MovementTrailData {
    fn default() -> Self {
        Self {
            enabled: true,
            max_points: 200,
            sample_interval: 2,
            color_mode: TrailColorMode::Solid,
            trail_color: [0.2, 0.8, 1.0, 0.8],
            fade_alpha: true,
            points: VecDeque::with_capacity(200),
            frame_counter: 0,
        }
    }
}

/// Commands from the trails UI
#[derive(Clone, Debug)]
pub enum TrailCommand {
    ClearAll,
    AddToSelected,
    RemoveFromSelected,
}

/// Global state resource for the Movement Trails panel
#[derive(Resource)]
pub struct MovementTrailsState {
    /// Master visibility toggle
    pub show_all: bool,
    /// Global color mode override (None = use per-entity)
    pub global_color_mode: Option<TrailColorMode>,
    /// Pending commands
    pub commands: Vec<TrailCommand>,
    /// Number of entities with trails
    pub trail_entity_count: usize,
}

impl Default for MovementTrailsState {
    fn default() -> Self {
        Self {
            show_all: true,
            global_color_mode: None,
            commands: Vec::new(),
            trail_entity_count: 0,
        }
    }
}

/// System that updates trail point data for all entities with MovementTrailData
pub fn update_movement_trails(
    mut state: ResMut<MovementTrailsState>,
    mut trails: Query<(&mut MovementTrailData, &Transform, Option<&avian3d::prelude::LinearVelocity>)>,
    time: Res<Time>,
) {
    state.trail_entity_count = trails.iter().count();

    for (mut trail, transform, lin_vel) in trails.iter_mut() {
        if !trail.enabled {
            continue;
        }

        trail.frame_counter += 1;
        if trail.frame_counter < trail.sample_interval {
            continue;
        }
        trail.frame_counter = 0;

        let velocity = lin_vel.map(|v| v.0.length()).unwrap_or(0.0);

        trail.points.push_back(TrailPoint {
            position: transform.translation,
            velocity,
            timestamp: time.elapsed_secs(),
        });

        while trail.points.len() > trail.max_points {
            trail.points.pop_front();
        }
    }
}

/// System that renders trail polylines using gizmos
pub fn render_trail_gizmos(
    state: Res<MovementTrailsState>,
    trails: Query<&MovementTrailData>,
    mut gizmos: Gizmos<crate::gizmo::physics_viz::PhysicsVizGizmoGroup>,
    time: Res<Time>,
) {
    if !state.show_all {
        return;
    }

    let now = time.elapsed_secs();

    for trail in trails.iter() {
        if trail.points.len() < 2 {
            continue;
        }

        let color_mode = state.global_color_mode.unwrap_or(trail.color_mode);

        for pair in trail.points.iter().collect::<Vec<_>>().windows(2) {
            let p0 = pair[0];
            let p1 = pair[1];

            let age_factor = if trail.fade_alpha {
                let age = now - p0.timestamp;
                let max_age = trail.max_points as f32 * 0.05; // rough estimate
                (1.0 - age / max_age.max(0.1)).clamp(0.1, 1.0)
            } else {
                1.0
            };

            let color = match color_mode {
                TrailColorMode::Solid => {
                    Color::srgba(
                        trail.trail_color[0],
                        trail.trail_color[1],
                        trail.trail_color[2],
                        trail.trail_color[3] * age_factor,
                    )
                }
                TrailColorMode::VelocityHeatmap => {
                    let t = (p0.velocity / 20.0).min(1.0);
                    Color::srgba(t, 0.3, 1.0 - t, 0.8 * age_factor)
                }
                TrailColorMode::AgeGradient => {
                    Color::srgba(0.2, 0.8 * age_factor, 1.0 * age_factor, 0.8 * age_factor)
                }
            };

            gizmos.line(p0.position, p1.position, color);
        }
    }
}

/// System that processes trail commands (add/remove from selected, clear all)
pub fn process_trail_commands(
    mut state: ResMut<MovementTrailsState>,
    mut commands: Commands,
    selection: Res<crate::core::SelectionState>,
    trail_entities: Query<Entity, With<MovementTrailData>>,
    mut trails: Query<&mut MovementTrailData>,
) {
    let cmds: Vec<TrailCommand> = state.commands.drain(..).collect();
    for cmd in cmds {
        match cmd {
            TrailCommand::ClearAll => {
                for mut trail in trails.iter_mut() {
                    trail.points.clear();
                }
            }
            TrailCommand::AddToSelected => {
                if let Some(entity) = selection.selected_entity {
                    if trail_entities.get(entity).is_err() {
                        commands.entity(entity).insert(MovementTrailData::default());
                    }
                }
            }
            TrailCommand::RemoveFromSelected => {
                if let Some(entity) = selection.selected_entity {
                    if trail_entities.get(entity).is_ok() {
                        commands.entity(entity).remove::<MovementTrailData>();
                    }
                }
            }
        }
    }
}
