use bevy::prelude::*;
use bevy_egui::egui;
use serde::{Deserialize, Serialize};

use crate::component_system::{ComponentCategory, ComponentRegistry, PropertyValue, PropertyValueType};
use crate::register_component;

use egui_phosphor::regular::PATH;

// ============================================================================
// Data Types
// ============================================================================

/// Editor-facing serializable navigation agent data.
/// When added, automatically inserts the vleue_navigator ORCA agent components.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
pub struct NavigationAgentData {
    pub radius: f32,
    pub max_speed: f32,
    pub target: Option<Vec3>,
    pub arrival_threshold: f32,
}

impl Default for NavigationAgentData {
    fn default() -> Self {
        Self {
            radius: 0.5,
            max_speed: 5.0,
            target: None,
            arrival_threshold: 0.3,
        }
    }
}

/// Runtime pathfinding state (not serialized).
#[derive(Component, Default)]
pub struct NavigationAgentState {
    pub path: Vec<Vec3>,
    pub current_waypoint: usize,
    pub arrived: bool,
    /// Cached target so we can detect when it changes.
    prev_target: Option<Vec3>,
}

// ============================================================================
// Custom Add / Remove
// ============================================================================

fn add_navigation_agent(
    commands: &mut Commands,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) {
    commands.entity(entity).insert((
        NavigationAgentData::default(),
        NavigationAgentState::default(),
        vleue_navigator::prelude::NavigationAgent::default(),
        vleue_navigator::prelude::NavMeshAgentExclusion,
    ));
}

fn remove_navigation_agent(commands: &mut Commands, entity: Entity) {
    commands
        .entity(entity)
        .remove::<NavigationAgentData>()
        .remove::<NavigationAgentState>()
        .remove::<vleue_navigator::prelude::NavigationAgent>()
        .remove::<vleue_navigator::prelude::NavMeshAgentExclusion>();
}

// ============================================================================
// Custom Inspector
// ============================================================================

fn inspect_navigation_agent(
    ui: &mut egui::Ui,
    world: &mut World,
    entity: Entity,
    _meshes: &mut Assets<Mesh>,
    _materials: &mut Assets<StandardMaterial>,
) -> bool {
    let mut changed = false;

    if let Some(mut data) = world.get_mut::<NavigationAgentData>(entity) {
        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.nav_agent.radius"));
            if ui
                .add(egui::DragValue::new(&mut data.radius).speed(0.05).range(0.1..=10.0))
                .changed()
            {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label(&crate::locale::t("comp.nav_agent.speed"));
            if ui
                .add(egui::DragValue::new(&mut data.max_speed).speed(0.1).range(0.1..=100.0))
                .changed()
            {
                changed = true;
            }
        });

        ui.horizontal(|ui| {
            ui.label("Arrival Threshold:");
            if ui
                .add(
                    egui::DragValue::new(&mut data.arrival_threshold)
                        .speed(0.05)
                        .range(0.05..=5.0),
                )
                .changed()
            {
                changed = true;
            }
        });

        // Read-only target display
        ui.horizontal(|ui| {
            ui.label("Target:");
            match &data.target {
                Some(t) => {
                    ui.label(format!("({:.1}, {:.1}, {:.1})", t.x, t.y, t.z));
                }
                None => {
                    ui.weak("None");
                }
            }
        });
    }

    // Read-only arrived display (from runtime state)
    if let Some(state) = world.get::<NavigationAgentState>(entity) {
        ui.horizontal(|ui| {
            ui.label("Arrived:");
            ui.label(if state.arrived { "Yes" } else { "No" });
        });
    }

    changed
}

// ============================================================================
// Script Property Access
// ============================================================================

fn nav_agent_property_meta() -> Vec<(&'static str, PropertyValueType)> {
    vec![
        ("radius", PropertyValueType::Float),
        ("max_speed", PropertyValueType::Float),
        ("target_x", PropertyValueType::Float),
        ("target_y", PropertyValueType::Float),
        ("target_z", PropertyValueType::Float),
        ("arrival_threshold", PropertyValueType::Float),
        ("arrived", PropertyValueType::Bool),
    ]
}

fn nav_agent_get_props(world: &World, entity: Entity) -> Vec<(&'static str, PropertyValue)> {
    let Some(data) = world.get::<NavigationAgentData>(entity) else {
        return vec![];
    };
    let arrived = world
        .get::<NavigationAgentState>(entity)
        .map(|s| s.arrived)
        .unwrap_or(false);
    let (tx, ty, tz) = data
        .target
        .map(|t| (t.x, t.y, t.z))
        .unwrap_or((0.0, 0.0, 0.0));
    vec![
        ("radius", PropertyValue::Float(data.radius)),
        ("max_speed", PropertyValue::Float(data.max_speed)),
        ("target_x", PropertyValue::Float(tx)),
        ("target_y", PropertyValue::Float(ty)),
        ("target_z", PropertyValue::Float(tz)),
        ("arrival_threshold", PropertyValue::Float(data.arrival_threshold)),
        ("arrived", PropertyValue::Bool(arrived)),
    ]
}

fn nav_agent_set_prop(world: &mut World, entity: Entity, prop: &str, val: &PropertyValue) -> bool {
    let Some(mut data) = world.get_mut::<NavigationAgentData>(entity) else {
        return false;
    };
    match prop {
        "radius" => {
            if let PropertyValue::Float(v) = val {
                data.radius = *v;
                true
            } else {
                false
            }
        }
        "max_speed" => {
            if let PropertyValue::Float(v) = val {
                data.max_speed = *v;
                true
            } else {
                false
            }
        }
        "arrival_threshold" => {
            if let PropertyValue::Float(v) = val {
                data.arrival_threshold = *v;
                true
            } else {
                false
            }
        }
        "target_x" => {
            if let PropertyValue::Float(v) = val {
                let t = data.target.get_or_insert(Vec3::ZERO);
                t.x = *v;
                true
            } else {
                false
            }
        }
        "target_y" => {
            if let PropertyValue::Float(v) = val {
                let t = data.target.get_or_insert(Vec3::ZERO);
                t.y = *v;
                true
            } else {
                false
            }
        }
        "target_z" => {
            if let PropertyValue::Float(v) = val {
                let t = data.target.get_or_insert(Vec3::ZERO);
                t.z = *v;
                true
            } else {
                false
            }
        }
        _ => false,
    }
}

// ============================================================================
// Movement System
// ============================================================================

/// Drives pathfinding + ORCA avoidance + movement for every NavigationAgentData entity.
pub fn navigation_agent_system(
    time: Res<Time>,
    navmeshes: Res<Assets<vleue_navigator::NavMesh>>,
    managed_query: Query<&vleue_navigator::prelude::ManagedNavMesh>,
    mut agents: Query<(
        &mut NavigationAgentData,
        &mut NavigationAgentState,
        &mut vleue_navigator::prelude::NavigationAgent,
        &mut Transform,
    )>,
) {
    // Find the first available navmesh handle
    let navmesh = managed_query
        .iter()
        .next()
        .and_then(|managed| navmeshes.get(managed));

    let dt = time.delta_secs();

    for (data, mut state, mut orca, mut transform) in agents.iter_mut() {
        // 1. Sync data → ORCA agent
        orca.radius = data.radius;
        orca.max_speed = data.max_speed;

        // 2. Handle target = None → clear everything
        let Some(target) = data.target else {
            if !state.path.is_empty() {
                state.path.clear();
                state.current_waypoint = 0;
                state.arrived = false;
                state.prev_target = None;
                orca.preferred_velocity = Vec3::ZERO;
            }
            continue;
        };

        // 3. Path query when target changed or path is empty
        let target_changed = state.prev_target.map_or(true, |prev| prev.distance(target) > 0.01);
        if target_changed || state.path.is_empty() {
            state.prev_target = Some(target);
            state.arrived = false;
            state.current_waypoint = 0;
            state.path.clear();

            if let Some(nm) = navmesh {
                if let Some(tp) = nm.transformed_path(transform.translation, target) {
                    state.path = tp.path;
                    // Skip first waypoint if it's essentially the current position
                    if state.path.len() > 1
                        && state.path[0].distance(transform.translation) < data.arrival_threshold
                    {
                        state.current_waypoint = 1;
                    }
                }
            }
        }

        // 4. Waypoint following
        if !state.path.is_empty() && !state.arrived {
            let wp = state.path[state.current_waypoint];
            let diff = wp - transform.translation;
            let dist = diff.length();

            if dist < data.arrival_threshold {
                // Advance to next waypoint
                if state.current_waypoint + 1 < state.path.len() {
                    state.current_waypoint += 1;
                } else {
                    // Reached final waypoint
                    state.arrived = true;
                    orca.preferred_velocity = Vec3::ZERO;
                }
            } else {
                orca.preferred_velocity = (diff / dist) * data.max_speed;
            }
        }

        // 5. Apply movement from ORCA avoidance velocity
        if !state.arrived {
            transform.translation += orca.avoidance_velocity * dt;
        }
    }
}

// ============================================================================
// Registration
// ============================================================================

pub fn register(registry: &mut ComponentRegistry) {
    registry.register_owned(register_component!(NavigationAgentData {
        type_id: "navigation_agent",
        display_name: "Navigation Agent",
        category: ComponentCategory::Gameplay,
        icon: PATH,
        priority: 0,
        custom_inspector: inspect_navigation_agent,
        custom_add: add_navigation_agent,
        custom_remove: remove_navigation_agent,
        custom_script_properties: nav_agent_get_props,
        custom_script_set: nav_agent_set_prop,
        custom_script_meta: nav_agent_property_meta,
    }));
}
