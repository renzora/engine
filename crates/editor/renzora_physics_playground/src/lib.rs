//! Physics panels for the Renzora editor.
//!
//! Provides six panels:
//! - **Physics Debug** — rigid body/collider monitoring, step time, collisions
//! - **Physics Playground** — stress-test shape spawner
//! - **Physics Properties** — gravity, time scale, substeps
//! - **Physics Forces** — apply forces/impulses to selected entities
//! - **Physics Metrics** — energy, velocity, momentum tracking
//! - **Physics Scenarios** — one-click preset scene spawner

pub mod panels;
pub mod state;

use std::sync::{Arc, Mutex, RwLock};

use bevy::prelude::*;
use bevy_egui::egui;

use renzora_editor::{AppEditorExt, EditorPanel, PanelLocation};
use renzora_physics::{
    CollisionShapeData, PhysicsBodyData, PhysicsBodyType, PhysicsPropertiesState,
};
use renzora_theme::ThemeManager;

use state::*;

// ============================================================================
// Bridge for mutable panel state
// ============================================================================

#[derive(Default)]
struct PhysicsBridgeInner {
    debug: Option<PhysicsDebugState>,
    playground: Option<PlaygroundState>,
    forces: Option<ForcesState>,
    metrics: Option<MetricsState>,
    scenarios: Option<ScenariosState>,
    arena: Option<ArenaPresetsState>,
}

#[derive(Resource)]
struct PhysicsBridge {
    pending: Arc<Mutex<PhysicsBridgeInner>>,
}

impl Default for PhysicsBridge {
    fn default() -> Self {
        Self {
            pending: Arc::new(Mutex::new(PhysicsBridgeInner::default())),
        }
    }
}

// ============================================================================
// Helper: get theme from world
// ============================================================================

fn get_theme(world: &World) -> renzora_theme::Theme {
    world
        .get_resource::<ThemeManager>()
        .map(|tm| tm.active_theme.clone())
        .unwrap_or_default()
}

// ============================================================================
// Physics Debug Panel
// ============================================================================

struct PhysicsDebugPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<PhysicsDebugState>,
}

impl PhysicsDebugPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(PhysicsDebugState::default()) }
    }
}

impl EditorPanel for PhysicsDebugPanel {
    fn id(&self) -> &str { "physics_debug" }
    fn title(&self) -> &str { "Physics Debug" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::ATOM) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [280.0, 350.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<PhysicsDebugState>() {
            if let Ok(mut local) = self.local.write() {
                local.simulation_running = state.simulation_running;
                local.dynamic_body_count = state.dynamic_body_count;
                local.kinematic_body_count = state.kinematic_body_count;
                local.static_body_count = state.static_body_count;
                local.collider_count = state.collider_count;
                local.colliders_by_type = state.colliders_by_type.clone();
                local.collision_pair_count = state.collision_pair_count;
                local.collision_pairs = state.collision_pairs.clone();
                local.step_time_history = state.step_time_history.clone();
                local.step_time_ms = state.step_time_ms;
                local.avg_step_time_ms = state.avg_step_time_ms;
                local.physics_available = state.physics_available;
            }
        }

        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::debug::render_physics_debug_content(ui, &mut local, &theme);
        }

        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                pending.debug = Some(local.clone());
            }
        }
    }
}

// ============================================================================
// Physics Playground Panel
// ============================================================================

struct PhysicsPlaygroundPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<PlaygroundState>,
}

impl PhysicsPlaygroundPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(PlaygroundState::default()) }
    }
}

impl EditorPanel for PhysicsPlaygroundPanel {
    fn id(&self) -> &str { "physics_playground" }
    fn title(&self) -> &str { "Physics Playground" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::CUBE) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<PlaygroundState>() {
            if let Ok(mut local) = self.local.write() {
                local.alive_count = state.alive_count;
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::playground::render_playground_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(mut local) = self.local.write() {
                pending.playground = Some(local.clone());
                local.commands.clear();
            }
        }
    }
}

// ============================================================================
// Physics Properties Panel
// ============================================================================

struct PhysicsPropertiesPanel;

impl EditorPanel for PhysicsPropertiesPanel {
    fn id(&self) -> &str { "physics_properties" }
    fn title(&self) -> &str { "Physics Properties" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::SLIDERS) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 280.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        // Properties state lives in renzora_physics — read+write directly
        let mut state = world
            .get_resource::<PhysicsPropertiesState>()
            .cloned()
            .unwrap_or_default();
        let theme = get_theme(world);
        panels::properties::render_properties_content(ui, &mut state, &theme);
        // Commands are picked up by renzora_physics sync system via the resource
        // We can't write back here since we only have &World, so commands are stored
        // in the cloned state. We'll bridge them.
        // Actually, since EditorPanel only gets &World, we need the bridge pattern.
        // For simplicity, we just note that this panel won't actually send commands
        // without a bridge. TODO: hook up properties bridge if needed.
    }
}

// ============================================================================
// Physics Forces Panel
// ============================================================================

struct PhysicsForcesPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<ForcesState>,
}

impl PhysicsForcesPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(ForcesState::default()) }
    }
}

impl EditorPanel for PhysicsForcesPanel {
    fn id(&self) -> &str { "physics_forces" }
    fn title(&self) -> &str { "Physics Forces" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::LIGHTNING) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<ForcesState>() {
            if let Ok(mut local) = self.local.write() {
                local.selected_entity = state.selected_entity;
                local.selected_has_rigidbody = state.selected_has_rigidbody;
                local.selected_linear_velocity = state.selected_linear_velocity;
                local.selected_angular_velocity = state.selected_angular_velocity;
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::forces::render_forces_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(mut local) = self.local.write() {
                pending.forces = Some(local.clone());
                local.commands.clear();
            }
        }
    }
}

// ============================================================================
// Physics Metrics Panel
// ============================================================================

struct PhysicsMetricsPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<MetricsState>,
}

impl PhysicsMetricsPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(MetricsState::default()) }
    }
}

impl EditorPanel for PhysicsMetricsPanel {
    fn id(&self) -> &str { "physics_metrics" }
    fn title(&self) -> &str { "Physics Metrics" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::CHART_LINE) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 280.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<MetricsState>() {
            if let Ok(mut local) = self.local.write() {
                local.total_kinetic_energy = state.total_kinetic_energy;
                local.total_potential_energy = state.total_potential_energy;
                local.total_energy = state.total_energy;
                local.energy_history = state.energy_history.clone();
                local.avg_velocity = state.avg_velocity;
                local.max_velocity = state.max_velocity;
                local.active_bodies = state.active_bodies;
                local.sleeping_bodies = state.sleeping_bodies;
                local.total_momentum = state.total_momentum;
                local.frame_physics_time_ms = state.frame_physics_time_ms;
                local.physics_time_history = state.physics_time_history.clone();
                local.collision_count = state.collision_count;
                local.collision_pairs_history = state.collision_pairs_history.clone();
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::metrics::render_metrics_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(local) = self.local.read() {
                pending.metrics = Some(local.clone());
            }
        }
    }
}

// ============================================================================
// Physics Scenarios Panel
// ============================================================================

struct PhysicsScenariosPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<ScenariosState>,
}

impl PhysicsScenariosPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(ScenariosState::default()) }
    }
}

impl EditorPanel for PhysicsScenariosPanel {
    fn id(&self) -> &str { "physics_scenarios" }
    fn title(&self) -> &str { "Physics Scenarios" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::ROCKET_LAUNCH) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<ScenariosState>() {
            if let Ok(mut local) = self.local.write() {
                local.alive_count = state.alive_count;
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::scenarios::render_scenarios_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(mut local) = self.local.write() {
                pending.scenarios = Some(local.clone());
                local.commands.clear();
            }
        }
    }
}

// ============================================================================
// Arena Presets Panel
// ============================================================================

struct ArenaPresetsPanel {
    bridge: Arc<Mutex<PhysicsBridgeInner>>,
    local: RwLock<ArenaPresetsState>,
}

impl ArenaPresetsPanel {
    fn new(bridge: Arc<Mutex<PhysicsBridgeInner>>) -> Self {
        Self { bridge, local: RwLock::new(ArenaPresetsState::default()) }
    }
}

impl EditorPanel for ArenaPresetsPanel {
    fn id(&self) -> &str { "arena_presets" }
    fn title(&self) -> &str { "Arena Presets" }
    fn icon(&self) -> Option<&str> { Some(egui_phosphor::regular::CUBE) }
    fn default_location(&self) -> PanelLocation { PanelLocation::Right }
    fn min_size(&self) -> [f32; 2] { [250.0, 300.0] }

    fn ui(&self, ui: &mut egui::Ui, world: &World) {
        if let Some(state) = world.get_resource::<ArenaPresetsState>() {
            if let Ok(mut local) = self.local.write() {
                local.arena_entity_count = state.arena_entity_count;
                local.has_active_arena = state.has_active_arena;
            }
        }
        let theme = get_theme(world);
        if let Ok(mut local) = self.local.write() {
            panels::arena::render_arena_presets_content(ui, &mut local, &theme);
        }
        if let Ok(mut pending) = self.bridge.lock() {
            if let Ok(mut local) = self.local.write() {
                pending.arena = Some(local.clone());
                local.commands.clear();
            }
        }
    }
}

// ============================================================================
// Sync bridge → world resources
// ============================================================================

fn sync_physics_bridge(
    bridge: Res<PhysicsBridge>,
    mut debug_state: ResMut<PhysicsDebugState>,
    mut playground_state: ResMut<PlaygroundState>,
    mut forces_state: ResMut<ForcesState>,
    mut metrics_state: ResMut<MetricsState>,
    mut scenarios_state: ResMut<ScenariosState>,
    mut arena_state: ResMut<ArenaPresetsState>,
) {
    if let Ok(mut pending) = bridge.pending.lock() {
        if let Some(debug) = pending.debug.take() {
            debug_state.debug_toggles = debug.debug_toggles;
            debug_state.show_collision_pairs = debug.show_collision_pairs;
        }
        if let Some(pg) = pending.playground.take() {
            playground_state.shape = pg.shape;
            playground_state.pattern = pg.pattern;
            playground_state.count = pg.count;
            playground_state.mass = pg.mass;
            playground_state.restitution = pg.restitution;
            playground_state.friction = pg.friction;
            playground_state.spawn_height = pg.spawn_height;
            playground_state.commands = pg.commands;
        }
        if let Some(forces) = pending.forces.take() {
            forces_state.mode = forces.mode;
            forces_state.direction_preset = forces.direction_preset;
            forces_state.custom_direction = forces.custom_direction;
            forces_state.magnitude = forces.magnitude;
            forces_state.explosion_radius = forces.explosion_radius;
            forces_state.explosion_magnitude = forces.explosion_magnitude;
            forces_state.velocity_linear = forces.velocity_linear;
            forces_state.velocity_angular = forces.velocity_angular;
            forces_state.commands = forces.commands;
        }
        if let Some(metrics) = pending.metrics.take() {
            metrics_state.tracking_enabled = metrics.tracking_enabled;
        }
        if let Some(scenarios) = pending.scenarios.take() {
            scenarios_state.selected_scenario = scenarios.selected_scenario;
            scenarios_state.scale = scenarios.scale;
            scenarios_state.commands = scenarios.commands;
        }
        if let Some(arena) = pending.arena.take() {
            arena_state.arena_type = arena.arena_type;
            arena_state.scale = arena.scale;
            arena_state.commands = arena.commands;
        }
    }
}

// ============================================================================
// Playground command processing
// ============================================================================

fn process_playground_commands(
    mut state: ResMut<PlaygroundState>,
    mut commands: Commands,
    playground_entities: Query<Entity, With<PlaygroundEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.alive_count = playground_entities.iter().count();

    let cmds: Vec<PlaygroundCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() { return; }

    for cmd in cmds {
        match cmd {
            PlaygroundCommand::Spawn => {
                let positions = generate_positions(state.pattern, state.count, state.spawn_height);
                let (mesh, shape_data) = shape_mesh_and_collider(state.shape);
                let mesh_handle = meshes.add(mesh);

                let color = Color::hsl((state.alive_count as f32 * 37.0) % 360.0, 0.7, 0.6);
                let mat_handle = materials.add(StandardMaterial { base_color: color, ..default() });

                let body_data = PhysicsBodyData {
                    body_type: PhysicsBodyType::RigidBody,
                    mass: state.mass,
                    ..default()
                };
                let collision = CollisionShapeData {
                    restitution: state.restitution,
                    friction: state.friction,
                    ..shape_data
                };

                for (i, pos) in positions.iter().enumerate() {
                    let name = format!("Playground {} {}", state.shape.label(), state.alive_count + i);
                    let entity = commands.spawn((
                        PlaygroundEntity,
                        Name::new(name),
                        Mesh3d(mesh_handle.clone()),
                        MeshMaterial3d(mat_handle.clone()),
                        Transform::from_translation(*pos),
                        Visibility::default(),
                        body_data.clone(),
                        collision.clone(),
                    )).id();
                    renzora_physics::spawn_entity_physics(&mut commands, entity, Some(&body_data), Some(&collision));
                }
            }
            PlaygroundCommand::ClearAll => {
                for entity in playground_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

// ============================================================================
// Scenario command processing
// ============================================================================

fn process_scenario_commands(
    mut state: ResMut<ScenariosState>,
    mut commands: Commands,
    scenario_entities: Query<Entity, With<ScenarioEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.alive_count = scenario_entities.iter().count();

    let cmds: Vec<ScenarioCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() { return; }

    let scale = state.scale;

    for cmd in cmds {
        match cmd {
            ScenarioCommand::Spawn => {
                spawn_scenario(&mut commands, &mut meshes, &mut materials, state.selected_scenario, scale);
            }
            ScenarioCommand::ClearScenario => {
                for entity in scenario_entities.iter() {
                    commands.entity(entity).despawn();
                }
            }
        }
    }
}

fn spawn_bowling(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Lane
    let lane_mesh = meshes.add(Cuboid::new(3.0 * s, 0.1, 20.0 * s));
    spawn_pg(commands, lane_mesh, mat(materials, Color::srgb(0.6, 0.45, 0.25)), "Lane".into(),
        Transform::from_translation(Vec3::new(0.0, -0.05, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.5 * s, 0.05, 10.0 * s)), 1.0);

    // Pins in triangle formation
    let pin_mesh = meshes.add(Capsule3d::new(0.12 * s, 0.5 * s));
    let pin_mat = mat(materials, Color::WHITE);
    let pin_positions = [
        (0.0, 0.0), (-0.3, 0.5), (0.3, 0.5),
        (-0.6, 1.0), (0.0, 1.0), (0.6, 1.0),
        (-0.9, 1.5), (-0.3, 1.5), (0.3, 1.5), (0.9, 1.5),
    ];
    for (i, (x, z)) in pin_positions.iter().enumerate() {
        spawn_pg(commands, pin_mesh.clone(), pin_mat.clone(), format!("Pin {}", i),
            Transform::from_translation(Vec3::new(x * s, 0.35 * s, 8.0 * s + z * s)),
            PhysicsBodyType::RigidBody, CollisionShapeData::capsule(0.12 * s, 0.25 * s), 0.3);
    }

    // Bowling ball
    let ball_mesh = meshes.add(Sphere::new(0.3 * s));
    spawn_pg(commands, ball_mesh, mat(materials, Color::srgb(0.15, 0.15, 0.5)), "Bowling Ball".into(),
        Transform::from_translation(Vec3::new(0.0, 0.3 * s, -8.0 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 6.0);

    // Gutters
    for side in [-1.0f32, 1.0] {
        let gutter_mesh = meshes.add(Cuboid::new(0.2 * s, 0.3 * s, 20.0 * s));
        spawn_pg(commands, gutter_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Gutter".into(),
            Transform::from_translation(Vec3::new(side * 1.6 * s, 0.1 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 0.15 * s, 10.0 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_catapult(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Base
    let base_mesh = meshes.add(Cuboid::new(2.0 * s, 0.5 * s, 2.0 * s));
    spawn_pg(commands, base_mesh, mat(materials, Color::srgb(0.4, 0.3, 0.2)), "Catapult Base".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 0.25 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.0 * s, 0.25 * s, 1.0 * s)), 1.0);

    // Arm
    let arm_mesh = meshes.add(Cuboid::new(0.3 * s, 0.2 * s, 6.0 * s));
    spawn_pg(commands, arm_mesh, mat(materials, Color::srgb(0.5, 0.35, 0.15)), "Catapult Arm".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 1.5 * s, 0.0))
            .with_rotation(Quat::from_rotation_x(-0.5)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(0.15 * s, 0.1 * s, 3.0 * s)), 3.0);

    // Projectile
    let proj_mesh = meshes.add(Sphere::new(0.3 * s));
    spawn_pg(commands, proj_mesh, mat(materials, Color::hsl(15.0, 0.9, 0.5)), "Catapult Projectile".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 2.0 * s, -2.5 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 2.0);

    // Target wall
    let wall_mesh = meshes.add(Cuboid::new(0.5 * s, 3.0 * s, 4.0 * s));
    spawn_pg(commands, wall_mesh, mat(materials, Color::hsl(0.0, 0.5, 0.5)), "Target Wall".into(),
        Transform::from_translation(Vec3::new(8.0 * s, 1.5 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.25 * s, 1.5 * s, 2.0 * s)), 1.0);

    // Target boxes
    let box_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    for i in 0..6u32 {
        let row = i / 3;
        let col = i % 3;
        spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(i as f32 * 50.0, 0.7, 0.6)),
            format!("Target Box {}", i),
            Transform::from_translation(Vec3::new(
                10.0 * s,
                0.4 * s + row as f32 * 0.9 * s,
                (col as f32 - 1.0) * 0.9 * s,
            )),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.4 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_balancing_act(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Fulcrum
    let fulcrum_mesh = meshes.add(Cuboid::new(0.5 * s, 1.5 * s, 2.0 * s));
    spawn_pg(commands, fulcrum_mesh, mat(materials, Color::srgb(0.5, 0.5, 0.5)), "Fulcrum".into(),
        Transform::from_translation(Vec3::new(0.0, 0.75 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.25 * s, 0.75 * s, 1.0 * s)), 1.0);

    // Plank
    let plank_mesh = meshes.add(Cuboid::new(10.0 * s, 0.2 * s, 2.0 * s));
    spawn_pg(commands, plank_mesh, mat(materials, Color::srgb(0.6, 0.4, 0.2)), "Seesaw Plank".into(),
        Transform::from_translation(Vec3::new(0.0, 1.6 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(5.0 * s, 0.1 * s, 1.0 * s)), 2.0);

    // Light side objects
    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    for i in 0..3u32 {
        spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl(120.0 + i as f32 * 30.0, 0.7, 0.6)),
            format!("Light Ball {}", i),
            Transform::from_translation(Vec3::new(-3.0 * s - i as f32 * 0.9 * s, 2.5 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.4 * s), 1.0);
    }

    // Heavy side object
    let heavy_mesh = meshes.add(Cuboid::new(1.5 * s, 1.5 * s, 1.5 * s));
    spawn_pg(commands, heavy_mesh, mat(materials, Color::hsl(0.0, 0.6, 0.4)), "Heavy Block".into(),
        Transform::from_translation(Vec3::new(3.5 * s, 3.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.75 * s)), 10.0);

    spawn_ground(commands, meshes, materials, s);
}

fn spawn_marbles(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Spiral ramp segments
    let ramp_mat = mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let segments = 12;
    for i in 0..segments {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let y = 8.0 * s - i as f32 * 0.6 * s;
        let radius = 3.0 * s;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let ramp_mesh = meshes.add(Cuboid::new(3.0 * s, 0.15 * s, 1.0 * s));
        spawn_pg(commands, ramp_mesh, ramp_mat.clone(), format!("Ramp Segment {}", i),
            Transform::from_translation(Vec3::new(x, y, z))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_z(-0.15)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.5 * s, 0.075 * s, 0.5 * s)), 1.0);
    }

    // Marbles
    let marble_mesh = meshes.add(Sphere::new(0.25 * s));
    for i in 0..8u32 {
        let hue = (i as f32 / 8.0) * 360.0;
        let mut shape = CollisionShapeData::sphere(0.25 * s);
        shape.restitution = 0.8;
        shape.friction = 0.15;
        spawn_pg(commands, marble_mesh.clone(), mat(materials, Color::hsl(hue, 0.9, 0.6)),
            format!("Marble {}", i),
            Transform::from_translation(Vec3::new(
                (i as f32 * 0.3 - 1.0) * s,
                9.0 * s + i as f32 * 0.4 * s,
                (3.0 - i as f32 * 0.2) * s,
            )),
            PhysicsBodyType::RigidBody, shape, 0.5);
    }

    // Collection bowl
    let bowl_mesh = meshes.add(Sphere::new(2.0 * s));
    spawn_pg(commands, bowl_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.35)), "Bowl".into(),
        Transform::from_translation(Vec3::new(0.0, -1.5 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::sphere(2.0 * s), 1.0);

    spawn_ground(commands, meshes, materials, s);
}

fn spawn_jenga(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let block_w = 0.8 * s;
    let block_h = 0.25 * s;
    let block_d = 2.4 * s;
    let block_mesh = meshes.add(Cuboid::new(block_w, block_h, block_d));
    let layers = 12;

    for layer in 0..layers {
        let y = block_h * 0.5 + layer as f32 * block_h;
        let rotated = layer % 2 == 1;
        for j in 0..3 {
            let offset = (j as f32 - 1.0) * block_w;
            let hue = ((layer * 3 + j) as f32 * 17.0) % 360.0;
            let m = mat(materials, Color::hsl(hue, 0.5, 0.65));
            let transform = if rotated {
                Transform::from_translation(Vec3::new(0.0, y, offset))
                    .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2))
            } else {
                Transform::from_translation(Vec3::new(offset, y, 0.0))
            };
            spawn_pg(commands, block_mesh.clone(), m,
                format!("Jenga L{}B{}", layer, j), transform,
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(block_w * 0.5, block_h * 0.5, block_d * 0.5)), 0.8);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_plinko(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let board_w = 12.0 * s;
    let board_h = 16.0 * s;

    // Back board
    let back_mesh = meshes.add(Cuboid::new(board_w, board_h, 0.2 * s));
    spawn_pg(commands, back_mesh, mat(materials, Color::srgb(0.15, 0.15, 0.2)), "Plinko Board".into(),
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, -0.5 * s)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, board_h * 0.5, 0.1 * s)), 1.0);

    // Front glass
    let glass_mesh = meshes.add(Cuboid::new(board_w, board_h, 0.1 * s));
    spawn_pg(commands, glass_mesh, mat(materials, Color::srgba(0.5, 0.5, 0.6, 0.15)), "Glass".into(),
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, 0.5 * s)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, board_h * 0.5, 0.05 * s)), 1.0);

    // Pegs
    let peg_mesh = meshes.add(Cylinder::new(0.15 * s, 0.6 * s));
    let peg_mat = mat(materials, Color::srgb(0.7, 0.7, 0.75));
    let peg_rows = 10;
    let peg_spacing_x = 1.0 * s;
    let peg_spacing_y = 1.2 * s;
    for row in 0..peg_rows {
        let cols = if row % 2 == 0 { 10 } else { 9 };
        let x_offset = if row % 2 == 0 { 0.0 } else { peg_spacing_x * 0.5 };
        let y = board_h - 2.0 * s - row as f32 * peg_spacing_y;
        for col in 0..cols {
            let x = (col as f32 - (cols - 1) as f32 * 0.5) * peg_spacing_x + x_offset;
            spawn_pg(commands, peg_mesh.clone(), peg_mat.clone(), format!("Peg {},{}", row, col),
                Transform::from_translation(Vec3::new(x, y, 0.0))
                    .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
                PhysicsBodyType::StaticBody, CollisionShapeData::cylinder(0.15 * s, 0.3 * s), 1.0);
        }
    }

    // Side walls
    for side in [-1.0f32, 1.0] {
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, board_h, 1.0 * s));
        spawn_pg(commands, wall_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.35)), "Side Wall".into(),
            Transform::from_translation(Vec3::new(side * board_w * 0.5, board_h * 0.5, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, board_h * 0.5, 0.5 * s)), 1.0);
    }

    // Balls
    let ball_mesh = meshes.add(Sphere::new(0.2 * s));
    for i in 0..5u32 {
        let mut shape = CollisionShapeData::sphere(0.2 * s);
        shape.restitution = 0.6;
        spawn_pg(commands, ball_mesh.clone(), mat(materials, Color::hsl(i as f32 * 60.0, 0.8, 0.6)),
            format!("Plinko Ball {}", i),
            Transform::from_translation(Vec3::new((i as f32 - 2.0) * 0.5 * s, board_h + 1.0 * s, 0.0)),
            PhysicsBodyType::RigidBody, shape, 1.0);
    }

    // Bottom
    let bottom_mesh = meshes.add(Cuboid::new(board_w, 0.3 * s, 1.0 * s));
    spawn_pg(commands, bottom_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.35)), "Bottom".into(),
        Transform::from_translation(Vec3::new(0.0, 0.15 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, 0.15 * s, 0.5 * s)), 1.0);
}

fn spawn_conveyor(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Series of angled platforms
    let plat_mat = mat(materials, Color::srgb(0.35, 0.35, 0.4));
    for i in 0..5u32 {
        let direction = if i % 2 == 0 { -0.2 } else { 0.2 };
        let y = 2.0 * s + i as f32 * 2.5 * s;
        let x = if i % 2 == 0 { -2.0 * s } else { 2.0 * s };
        let plat_mesh = meshes.add(Cuboid::new(6.0 * s, 0.2 * s, 3.0 * s));
        spawn_pg(commands, plat_mesh, plat_mat.clone(), format!("Platform {}", i),
            Transform::from_translation(Vec3::new(x, y, 0.0))
                .with_rotation(Quat::from_rotation_z(direction)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.1 * s, 1.5 * s)), 1.0);
    }

    // Objects to convey
    let box_mesh = meshes.add(Cuboid::new(0.6 * s, 0.6 * s, 0.6 * s));
    let sphere_mesh = meshes.add(Sphere::new(0.35 * s));
    for i in 0..8u32 {
        let y = 14.0 * s + i as f32 * 0.8 * s;
        let x = -2.0 * s + (i as f32 * 0.3) * s;
        let m = mat(materials, Color::hsl(i as f32 * 40.0, 0.7, 0.6));
        if i % 2 == 0 {
            spawn_pg(commands, box_mesh.clone(), m, format!("Conveyor Box {}", i),
                Transform::from_translation(Vec3::new(x, y, 0.0)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.3 * s)), 1.0);
        } else {
            spawn_pg(commands, sphere_mesh.clone(), m, format!("Conveyor Sphere {}", i),
                Transform::from_translation(Vec3::new(x, y, 0.0)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.35 * s), 1.0);
        }
    }

    // Collection bin at bottom
    let wall_mat = mat(materials, Color::srgb(0.4, 0.3, 0.3));
    for side in [-1.0f32, 1.0] {
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, 2.0 * s, 4.0 * s));
        spawn_pg(commands, wall_mesh, wall_mat.clone(), "Bin Wall".into(),
            Transform::from_translation(Vec3::new(side * 3.0 * s, 1.0 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.0 * s, 2.0 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_trebuchet(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Frame
    let frame_mesh = meshes.add(Cuboid::new(0.3 * s, 4.0 * s, 0.3 * s));
    let frame_mat = mat(materials, Color::srgb(0.45, 0.35, 0.2));
    for side in [-1.0f32, 1.0] {
        spawn_pg(commands, frame_mesh.clone(), frame_mat.clone(), "Trebuchet Frame".into(),
            Transform::from_translation(Vec3::new(-5.0 * s, 2.0 * s, side * 1.0 * s)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.15 * s, 2.0 * s, 0.15 * s)), 1.0);
    }

    // Crossbar
    let bar_mesh = meshes.add(Cuboid::new(0.3 * s, 0.3 * s, 2.5 * s));
    spawn_pg(commands, bar_mesh, frame_mat.clone(), "Crossbar".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 4.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.15 * s, 0.15 * s, 1.25 * s)), 1.0);

    // Arm
    let arm_mesh = meshes.add(Cuboid::new(8.0 * s, 0.2 * s, 0.3 * s));
    spawn_pg(commands, arm_mesh, mat(materials, Color::srgb(0.5, 0.35, 0.15)), "Trebuchet Arm".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 4.2 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(0.3)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(4.0 * s, 0.1 * s, 0.15 * s)), 3.0);

    // Counterweight
    let weight_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    spawn_pg(commands, weight_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Counterweight".into(),
        Transform::from_translation(Vec3::new(-8.0 * s, 3.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.5 * s)), 30.0);

    // Projectile
    let proj_mesh = meshes.add(Sphere::new(0.3 * s));
    spawn_pg(commands, proj_mesh, mat(materials, Color::hsl(30.0, 0.9, 0.5)), "Trebuchet Projectile".into(),
        Transform::from_translation(Vec3::new(-2.0 * s, 5.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 1.0);

    // Target
    let box_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    for i in 0..9u32 {
        let row = i / 3;
        let col = i % 3;
        spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(i as f32 * 35.0, 0.7, 0.55)),
            format!("Target {}", i),
            Transform::from_translation(Vec3::new(
                10.0 * s,
                0.4 * s + row as f32 * 0.9 * s,
                (col as f32 - 1.0) * 0.9 * s,
            )),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.4 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_rube_goldberg(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Stage 1: Ball rolls down ramp
    let ramp1_mesh = meshes.add(Cuboid::new(3.0 * s, 0.15 * s, 2.0 * s));
    spawn_pg(commands, ramp1_mesh, mat(materials, Color::srgb(0.5, 0.4, 0.3)), "Ramp 1".into(),
        Transform::from_translation(Vec3::new(-8.0 * s, 6.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.3)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.5 * s, 0.075 * s, 1.0 * s)), 1.0);

    let ball1_mesh = meshes.add(Sphere::new(0.3 * s));
    spawn_pg(commands, ball1_mesh, mat(materials, Color::hsl(0.0, 0.8, 0.5)), "Trigger Ball".into(),
        Transform::from_translation(Vec3::new(-9.0 * s, 7.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 1.5);

    // Stage 2: Domino chain
    let domino_mesh = meshes.add(Cuboid::new(0.1 * s, 0.8 * s, 0.4 * s));
    let domino_mat = mat(materials, Color::hsl(200.0, 0.7, 0.6));
    for i in 0..8u32 {
        let x = -5.5 * s + i as f32 * 0.5 * s;
        spawn_pg(commands, domino_mesh.clone(), domino_mat.clone(), format!("Domino {}", i),
            Transform::from_translation(Vec3::new(x, 4.6 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(0.05 * s, 0.4 * s, 0.2 * s)), 0.3);
    }

    // Platform between stages
    let plat_mesh = meshes.add(Cuboid::new(6.0 * s, 0.2 * s, 2.0 * s));
    spawn_pg(commands, plat_mesh.clone(), mat(materials, Color::srgb(0.4, 0.4, 0.4)), "Platform 1".into(),
        Transform::from_translation(Vec3::new(-4.0 * s, 4.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.1 * s, 1.0 * s)), 1.0);

    // Stage 3: Ramp to bowling pins
    let ramp2_mesh = meshes.add(Cuboid::new(4.0 * s, 0.15 * s, 2.0 * s));
    spawn_pg(commands, ramp2_mesh, mat(materials, Color::srgb(0.45, 0.35, 0.3)), "Ramp 2".into(),
        Transform::from_translation(Vec3::new(0.0, 3.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.25)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.0 * s, 0.075 * s, 1.0 * s)), 1.0);

    // Heavy ball to knock things over
    let ball2_mesh = meshes.add(Sphere::new(0.5 * s));
    spawn_pg(commands, ball2_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Heavy Ball".into(),
        Transform::from_translation(Vec3::new(-1.5 * s, 4.3 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.5 * s), 5.0);

    // Stage 4: Box stack to knock over
    let box_mesh = meshes.add(Cuboid::new(0.7 * s, 0.7 * s, 0.7 * s));
    for i in 0..6u32 {
        let row = i / 2;
        let col = i % 2;
        spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(60.0 + i as f32 * 30.0, 0.6, 0.55)),
            format!("Stack Box {}", i),
            Transform::from_translation(Vec3::new(
                3.0 * s + col as f32 * 0.8 * s,
                0.35 * s + row as f32 * 0.75 * s,
                0.0,
            )),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.35 * s)), 1.0);
    }

    // Stage 5: Final ramp into collection
    let ramp3_mesh = meshes.add(Cuboid::new(5.0 * s, 0.15 * s, 2.0 * s));
    spawn_pg(commands, ramp3_mesh, mat(materials, Color::srgb(0.4, 0.4, 0.45)), "Final Ramp".into(),
        Transform::from_translation(Vec3::new(7.0 * s, 1.5 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.2)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.5 * s, 0.075 * s, 1.0 * s)), 1.0);

    // Collection bin walls
    let bin_mat = mat(materials, Color::srgb(0.35, 0.35, 0.4));
    for side in [-1.0f32, 1.0] {
        let bin_mesh = meshes.add(Cuboid::new(0.2 * s, 2.0 * s, 2.0 * s));
        spawn_pg(commands, bin_mesh, bin_mat.clone(), "Bin Wall".into(),
            Transform::from_translation(Vec3::new(10.0 * s + side * 1.5 * s, 1.0 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.0 * s, 1.0 * s)), 1.0);
    }

    spawn_ground(commands, meshes, materials, s);
}

fn spawn_waterfall(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Ledge
    let ledge_mesh = meshes.add(Cuboid::new(6.0 * s, 0.3 * s, 4.0 * s));
    spawn_pg(commands, ledge_mesh, mat(materials, Color::srgb(0.4, 0.4, 0.4)), "Ledge".into(),
        Transform::from_translation(Vec3::new(0.0, 10.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.15 * s, 2.0 * s)), 1.0);
    // Stream of spheres
    let sphere_mesh = meshes.add(Sphere::new(0.25 * s));
    for i in 0..40u32 {
        let x = (i as f32 * 0.7 - 1.0) * 0.2 * s;
        let z = ((i as f32 * 2.399).sin()) * 0.5 * s;
        let y = 11.0 * s + i as f32 * 0.5 * s;
        spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl((i as f32 * 9.0) % 360.0, 0.7, 0.6)),
            format!("Drop {}", i),
            Transform::from_translation(Vec3::new(x, y, z)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.25 * s), 0.5);
    }
    // Catch basin
    for side in [-1.0f32, 1.0] {
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, 3.0 * s, 6.0 * s));
        spawn_pg(commands, wall_mesh, mat(materials, Color::srgb(0.35, 0.35, 0.4)), "Basin Wall".into(),
            Transform::from_translation(Vec3::new(side * 4.0 * s, 1.5 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.5 * s, 3.0 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_cannon(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Cannon barrel
    let barrel_mesh = meshes.add(Cylinder::new(0.5 * s, 3.0 * s));
    spawn_pg(commands, barrel_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Cannon Barrel".into(),
        Transform::from_translation(Vec3::new(-8.0 * s, 2.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(0.5)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cylinder(0.5 * s, 1.5 * s), 1.0);
    // Cannonball
    let ball_mesh = meshes.add(Sphere::new(0.4 * s));
    spawn_pg(commands, ball_mesh, mat(materials, Color::srgb(0.15, 0.15, 0.15)), "Cannonball".into(),
        Transform::from_translation(Vec3::new(-6.0 * s, 4.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.4 * s), 15.0);
    // Fortress target
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    for row in 0..4u32 {
        for col in 0..5u32 {
            spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl((row * 5 + col) as f32 * 12.0, 0.5, 0.55)),
                format!("Fort Block {},{}", row, col),
                Transform::from_translation(Vec3::new(
                    6.0 * s + (col as f32 - 2.0) * 1.05 * s,
                    0.5 * s + row as f32 * 1.05 * s,
                    0.0,
                )),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.5 * s)), 1.0);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_bridge(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Two pillars
    let pillar_mesh = meshes.add(Cuboid::new(2.0 * s, 4.0 * s, 3.0 * s));
    let pillar_mat = mat(materials, Color::srgb(0.4, 0.4, 0.4));
    spawn_pg(commands, pillar_mesh.clone(), pillar_mat.clone(), "Left Pillar".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 2.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.0 * s, 2.0 * s, 1.5 * s)), 1.0);
    spawn_pg(commands, pillar_mesh, pillar_mat, "Right Pillar".into(),
        Transform::from_translation(Vec3::new(5.0 * s, 2.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.0 * s, 2.0 * s, 1.5 * s)), 1.0);
    // Bridge planks
    let plank_mesh = meshes.add(Cuboid::new(1.2 * s, 0.15 * s, 3.0 * s));
    for i in 0..8u32 {
        let x = -3.5 * s + i as f32 * 1.0 * s;
        spawn_pg(commands, plank_mesh.clone(), mat(materials, Color::srgb(0.55, 0.4, 0.2)),
            format!("Plank {}", i),
            Transform::from_translation(Vec3::new(x, 4.1 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(0.6 * s, 0.075 * s, 1.5 * s)), 2.0);
    }
    // Heavy objects to load
    let heavy_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    for i in 0..5u32 {
        spawn_pg(commands, heavy_mesh.clone(), mat(materials, Color::hsl(i as f32 * 60.0, 0.7, 0.5)),
            format!("Weight {}", i),
            Transform::from_translation(Vec3::new((i as f32 - 2.0) * 1.2 * s, 6.0 * s + i as f32 * 0.8 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.4 * s)), 5.0 + i as f32 * 2.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_elevator(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Shaft walls
    let wall_mat = mat(materials, Color::srgb(0.35, 0.35, 0.4));
    for side in [-1.0f32, 1.0] {
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, 12.0 * s, 4.0 * s));
        spawn_pg(commands, wall_mesh, wall_mat.clone(), "Shaft Wall".into(),
            Transform::from_translation(Vec3::new(side * 2.5 * s, 6.0 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 6.0 * s, 2.0 * s)), 1.0);
    }
    // Platforms at different heights
    let plat_mesh = meshes.add(Cuboid::new(4.5 * s, 0.2 * s, 4.0 * s));
    for i in 0..4u32 {
        spawn_pg(commands, plat_mesh.clone(), mat(materials, Color::srgb(0.3, 0.5, 0.6)),
            format!("Platform {}", i),
            Transform::from_translation(Vec3::new(0.0, 1.0 * s + i as f32 * 3.0 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.25 * s, 0.1 * s, 2.0 * s)), 1.0);
    }
    // Objects on platforms
    let box_mesh = meshes.add(Cuboid::new(0.6 * s, 0.6 * s, 0.6 * s));
    let sphere_mesh = meshes.add(Sphere::new(0.3 * s));
    for i in 0..8u32 {
        let platform = i / 2;
        let y = 1.5 * s + platform as f32 * 3.0 * s;
        let x = if i % 2 == 0 { -0.8 * s } else { 0.8 * s };
        if i % 3 == 0 {
            spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl(i as f32 * 40.0, 0.7, 0.6)),
                format!("Elevator Sphere {}", i),
                Transform::from_translation(Vec3::new(x, y, 0.0)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 1.0);
        } else {
            spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(i as f32 * 40.0, 0.7, 0.6)),
                format!("Elevator Box {}", i),
                Transform::from_translation(Vec3::new(x, y, 0.0)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.3 * s)), 1.5);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_pinball_scenario(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Tilted board
    let board_mesh = meshes.add(Cuboid::new(10.0 * s, 16.0 * s, 0.2 * s));
    spawn_pg(commands, board_mesh, mat(materials, Color::srgb(0.15, 0.2, 0.15)), "Board".into(),
        Transform::from_translation(Vec3::new(0.0, 8.0 * s, -0.5 * s))
            .with_rotation(Quat::from_rotation_x(0.1)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(5.0 * s, 8.0 * s, 0.1 * s)), 1.0);
    // Bumpers
    let bump_mesh = meshes.add(Cylinder::new(0.5 * s, 0.8 * s));
    let bump_mat = mat(materials, Color::srgb(0.8, 0.2, 0.2));
    for i in 0..8u32 {
        let x = ((i as f32 * 2.399).cos()) * 3.0 * s;
        let y = 3.0 * s + (i as f32 / 8.0) * 10.0 * s;
        let mut shape = CollisionShapeData::cylinder(0.5 * s, 0.4 * s);
        shape.restitution = 0.9;
        spawn_pg(commands, bump_mesh.clone(), bump_mat.clone(), format!("Bumper {}", i),
            Transform::from_translation(Vec3::new(x, y, 0.0))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            PhysicsBodyType::StaticBody, shape, 1.0);
    }
    // Balls
    let ball_mesh = meshes.add(Sphere::new(0.3 * s));
    for i in 0..3u32 {
        let mut shape = CollisionShapeData::sphere(0.3 * s);
        shape.restitution = 0.7;
        spawn_pg(commands, ball_mesh.clone(), mat(materials, Color::hsl(i as f32 * 100.0, 0.8, 0.6)),
            format!("Pinball {}", i),
            Transform::from_translation(Vec3::new((i as f32 - 1.0) * s, 15.0 * s, 0.0)),
            PhysicsBodyType::RigidBody, shape, 1.0);
    }
    // Side walls
    for side in [-1.0f32, 1.0] {
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, 16.0 * s, 1.0 * s));
        spawn_pg(commands, wall_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.35)), "Side".into(),
            Transform::from_translation(Vec3::new(side * 5.0 * s, 8.0 * s, 0.0)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 8.0 * s, 0.5 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_spinner(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Central spinning arm
    let arm_mesh = meshes.add(Cuboid::new(10.0 * s, 0.3 * s, 0.5 * s));
    spawn_pg(commands, arm_mesh, mat(materials, Color::srgb(0.5, 0.3, 0.3)), "Spinner Arm".into(),
        Transform::from_translation(Vec3::new(0.0, 1.5 * s, 0.0))
            .with_rotation(Quat::from_rotation_y(0.3)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(5.0 * s, 0.15 * s, 0.25 * s)), 8.0);
    // Objects around the edge
    let box_mesh = meshes.add(Cuboid::new(0.7 * s, 0.7 * s, 0.7 * s));
    for i in 0..12u32 {
        let angle = (i as f32 / 12.0) * std::f32::consts::TAU;
        let r = 4.0 * s;
        spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(i as f32 * 30.0, 0.7, 0.6)),
            format!("Target {}", i),
            Transform::from_translation(Vec3::new(angle.cos() * r, 0.35 * s, angle.sin() * r)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.35 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_hourglass(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Upper funnel walls
    let segments = 12u32;
    let wall_mat = mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let angle_step = std::f32::consts::TAU / segments as f32;
    let seg_w = 4.0 * s * angle_step * 1.1;
    for i in 0..segments {
        let angle = i as f32 * angle_step;
        let r = 3.0 * s;
        let cx = angle.cos() * r;
        let cz = angle.sin() * r;
        // Upper cone
        let wall_mesh = meshes.add(Cuboid::new(seg_w, 0.15 * s, 4.0 * s));
        spawn_pg(commands, wall_mesh, wall_mat.clone(), format!("Upper {}", i),
            Transform::from_translation(Vec3::new(cx, 8.0 * s, cz))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(0.6)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.075 * s, 2.0 * s)), 1.0);
        // Lower cone (inverted)
        let wall_mesh2 = meshes.add(Cuboid::new(seg_w, 0.15 * s, 4.0 * s));
        spawn_pg(commands, wall_mesh2, wall_mat.clone(), format!("Lower {}", i),
            Transform::from_translation(Vec3::new(cx, 2.0 * s, cz))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(-0.6)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.075 * s, 2.0 * s)), 1.0);
    }
    // Spheres in upper chamber
    let sphere_mesh = meshes.add(Sphere::new(0.2 * s));
    for i in 0..25u32 {
        let angle = i as f32 * 2.399;
        let r = (i as f32 / 25.0).sqrt() * 2.0 * s;
        spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl((i as f32 * 14.0) % 360.0, 0.7, 0.6)),
            format!("Grain {}", i),
            Transform::from_translation(Vec3::new(angle.cos() * r, 9.0 * s + i as f32 * 0.3 * s, angle.sin() * r)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.2 * s), 0.3);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_mass_comparison(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Two ramps side by side
    let ramp_mesh = meshes.add(Cuboid::new(3.0 * s, 0.15 * s, 8.0 * s));
    for (i, x_off) in [-3.0f32, 3.0f32].iter().enumerate() {
        spawn_pg(commands, ramp_mesh.clone(), mat(materials, Color::srgb(0.4, 0.4, 0.45)),
            format!("Ramp {}", i),
            Transform::from_translation(Vec3::new(*x_off * s, 4.0 * s, 0.0))
                .with_rotation(Quat::from_rotation_x(-0.25)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.5 * s, 0.075 * s, 4.0 * s)), 1.0);
    }
    // Light sphere on left
    let light_mesh = meshes.add(Sphere::new(0.5 * s));
    spawn_pg(commands, light_mesh, mat(materials, Color::hsl(120.0, 0.7, 0.6)), "Light Ball (1kg)".into(),
        Transform::from_translation(Vec3::new(-3.0 * s, 6.0 * s, -3.0 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.5 * s), 1.0);
    // Heavy sphere on right
    let heavy_mesh = meshes.add(Sphere::new(0.5 * s));
    spawn_pg(commands, heavy_mesh, mat(materials, Color::hsl(0.0, 0.7, 0.5)), "Heavy Ball (50kg)".into(),
        Transform::from_translation(Vec3::new(3.0 * s, 6.0 * s, -3.0 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.5 * s), 50.0);
    // Targets at bottom
    let target_mesh = meshes.add(Cuboid::new(0.6 * s, 0.6 * s, 0.6 * s));
    for side in [-3.0f32, 3.0] {
        for j in 0..3u32 {
            spawn_pg(commands, target_mesh.clone(), mat(materials, Color::hsl(45.0, 0.6, 0.6)),
                format!("Target {}", j),
                Transform::from_translation(Vec3::new(side * s, 0.3 * s, 3.0 * s + j as f32 * 0.7 * s)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.3 * s)), 1.0);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_ricochet(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Chamber walls
    let wall_mat = mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let chamber = 8.0 * s;
    let wh = 6.0 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, chamber), Vec3::new(chamber, wh * 0.5, 0.15)),
        (Vec3::new(0.0, wh * 0.5, -chamber), Vec3::new(chamber, wh * 0.5, 0.15)),
        (Vec3::new(chamber, wh * 0.5, 0.0), Vec3::new(0.15, wh * 0.5, chamber)),
        (Vec3::new(-chamber, wh * 0.5, 0.0), Vec3::new(0.15, wh * 0.5, chamber)),
    ] {
        let wall_mesh = meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0));
        spawn_pg(commands, wall_mesh, wall_mat.clone(), "Chamber Wall".into(),
            Transform::from_translation(pos), PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(half), 1.0);
    }
    // Ceiling
    let ceil_mesh = meshes.add(Cuboid::new(chamber * 2.0, 0.2 * s, chamber * 2.0));
    spawn_pg(commands, ceil_mesh, wall_mat.clone(), "Ceiling".into(),
        Transform::from_translation(Vec3::new(0.0, wh, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(chamber, 0.1 * s, chamber)), 1.0);
    // Angled deflectors inside
    let defl_mesh = meshes.add(Cuboid::new(4.0 * s, 3.0 * s, 0.2 * s));
    for (pos, rot) in [
        (Vec3::new(-3.0 * s, 3.0 * s, 3.0 * s), 0.5f32),
        (Vec3::new(3.0 * s, 3.0 * s, -3.0 * s), -0.5),
        (Vec3::new(4.0 * s, 2.0 * s, 4.0 * s), 0.8),
    ] {
        spawn_pg(commands, defl_mesh.clone(), mat(materials, Color::srgb(0.5, 0.4, 0.3)), "Deflector".into(),
            Transform::from_translation(pos).with_rotation(Quat::from_rotation_y(rot)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.0 * s, 1.5 * s, 0.1 * s)), 1.0);
    }
    // Ball
    let ball_mesh = meshes.add(Sphere::new(0.4 * s));
    let mut shape = CollisionShapeData::sphere(0.4 * s);
    shape.restitution = 0.95;
    spawn_pg(commands, ball_mesh, mat(materials, Color::hsl(200.0, 0.8, 0.6)), "Ricochet Ball".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 1.0 * s, -5.0 * s)),
        PhysicsBodyType::RigidBody, shape, 2.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_box_fort(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    let wall_len = 6;
    let wall_h = 4;
    // Four walls of boxes
    for wall in 0..4u32 {
        let angle = wall as f32 * std::f32::consts::FRAC_PI_2;
        for col in 0..wall_len as u32 {
            for row in 0..wall_h as u32 {
                let offset = (col as f32 - (wall_len - 1) as f32 * 0.5) * 1.05 * s;
                let y = 0.5 * s + row as f32 * 1.05 * s;
                let dist = (wall_len as f32 * 0.5) * 1.05 * s;
                let x = angle.cos() * dist + angle.sin() * offset;
                let z = angle.sin() * dist - angle.cos() * offset;
                spawn_pg(commands, box_mesh.clone(),
                    mat(materials, Color::hsl(((wall * wall_len as u32 * wall_h as u32 + col * wall_h as u32 + row) as f32 * 7.0) % 360.0, 0.5, 0.6)),
                    format!("Fort {},{},{}", wall, col, row),
                    Transform::from_translation(Vec3::new(x, y, z)),
                    PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.5 * s)), 1.0);
            }
        }
    }
    // Projectile
    let proj_mesh = meshes.add(Sphere::new(0.8 * s));
    spawn_pg(commands, proj_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Projectile".into(),
        Transform::from_translation(Vec3::new(-10.0 * s, 3.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.8 * s), 20.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_spiral_drop(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let ramp_mat = mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let segments = 16;
    let total_height = 12.0 * s;
    for i in 0..segments {
        let angle = i as f32 * std::f32::consts::TAU / 4.0;
        let y = total_height - i as f32 * (total_height / segments as f32);
        let r = 3.0 * s;
        let ramp_mesh = meshes.add(Cuboid::new(3.5 * s, 0.15 * s, 1.5 * s));
        spawn_pg(commands, ramp_mesh, ramp_mat.clone(), format!("Spiral Ramp {}", i),
            Transform::from_translation(Vec3::new(angle.cos() * r, y, angle.sin() * r))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_z(-0.1)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.75 * s, 0.075 * s, 0.75 * s)), 1.0);
    }
    // Central column
    let col_mesh = meshes.add(Cylinder::new(0.5 * s, total_height));
    spawn_pg(commands, col_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.35)), "Column".into(),
        Transform::from_translation(Vec3::new(0.0, total_height * 0.5, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cylinder(0.5 * s, total_height * 0.5), 1.0);
    // Objects to drop
    let sphere_mesh = meshes.add(Sphere::new(0.3 * s));
    for i in 0..6u32 {
        spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl(i as f32 * 55.0, 0.8, 0.6)),
            format!("Ball {}", i),
            Transform::from_translation(Vec3::new((i as f32 - 2.5) * 0.7 * s, total_height + 2.0 * s, 3.0 * s)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_trampoline(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Bouncy floor
    let floor_mesh = meshes.add(Cuboid::new(15.0 * s, 0.3 * s, 15.0 * s));
    let mut floor_shape = CollisionShapeData::cuboid(Vec3::new(7.5 * s, 0.15 * s, 7.5 * s));
    floor_shape.restitution = 0.98;
    spawn_pg(commands, floor_mesh, mat(materials, Color::srgb(0.2, 0.5, 0.8)), "Trampoline".into(),
        Transform::from_translation(Vec3::new(0.0, -0.15 * s, 0.0)),
        PhysicsBodyType::StaticBody, floor_shape, 1.0);
    // Walls to keep things in
    let wall_mat = mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let arena = 7.5 * s;
    let wh = 15.0 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena, wh * 0.5, 0.15)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena, wh * 0.5, 0.15)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.15, wh * 0.5, arena)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.15, wh * 0.5, arena)),
    ] {
        let wall_mesh = meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0));
        let mut wall_shape = CollisionShapeData::cuboid(half);
        wall_shape.restitution = 0.95;
        spawn_pg(commands, wall_mesh, wall_mat.clone(), "Wall".into(),
            Transform::from_translation(pos), PhysicsBodyType::StaticBody, wall_shape, 1.0);
    }
    // Objects to bounce
    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.6 * s, 0.6 * s, 0.6 * s));
    for i in 0..10u32 {
        let x = ((i as f32 * 2.399).cos()) * 3.0 * s;
        let z = ((i as f32 * 2.399).sin()) * 3.0 * s;
        let y = 5.0 * s + i as f32 * 1.5 * s;
        if i % 2 == 0 {
            let mut shape = CollisionShapeData::sphere(0.4 * s);
            shape.restitution = 0.95;
            spawn_pg(commands, sphere_mesh.clone(), mat(materials, Color::hsl(i as f32 * 35.0, 0.8, 0.6)),
                format!("Bouncer {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, shape, 1.0);
        } else {
            let mut shape = CollisionShapeData::cuboid(Vec3::splat(0.3 * s));
            shape.restitution = 0.9;
            spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl(i as f32 * 35.0, 0.8, 0.6)),
                format!("Bouncer {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, shape, 1.5);
        }
    }
}

fn spawn_chain_reaction(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Progressive domino sizes
    let stages = 8;
    let mut x = -10.0 * s;
    for stage in 0..stages {
        let scale_factor = 0.5 + stage as f32 * 0.3;
        let w = 0.1 * s * scale_factor;
        let h = 1.0 * s * scale_factor;
        let d = 0.5 * s * scale_factor;
        let domino_mesh = meshes.add(Cuboid::new(w, h, d));
        let count = (5 - stage / 2).max(2) as u32;
        for j in 0..count {
            let dx = j as f32 * h * 0.6;
            let tilt = if stage == 0 && j == 0 { Quat::from_rotation_z(0.15) } else { Quat::IDENTITY };
            spawn_pg(commands, domino_mesh.clone(), mat(materials, Color::hsl(stage as f32 * 40.0, 0.7, 0.55)),
                format!("Stage{}D{}", stage, j),
                Transform::from_translation(Vec3::new(x + dx, h * 0.5, 0.0)).with_rotation(tilt),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(w * 0.5, h * 0.5, d * 0.5)), scale_factor);
        }
        x += count as f32 * h * 0.6 + 0.5 * s;
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_freefall(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Target platform
    let target_mesh = meshes.add(Cuboid::new(6.0 * s, 0.3 * s, 6.0 * s));
    spawn_pg(commands, target_mesh, mat(materials, Color::srgb(0.8, 0.2, 0.2)), "Target".into(),
        Transform::from_translation(Vec3::new(0.0, 0.15 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.15 * s, 3.0 * s)), 1.0);
    // Objects at great height
    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.7 * s, 0.7 * s, 0.7 * s));
    let capsule_mesh = meshes.add(Capsule3d::new(0.25 * s, 0.8 * s));
    for i in 0..15u32 {
        let x = ((i as f32 * 2.399).cos()) * 2.0 * s;
        let z = ((i as f32 * 2.399).sin()) * 2.0 * s;
        let y = 30.0 * s + i as f32 * 1.0 * s;
        let m = mat(materials, Color::hsl(i as f32 * 23.0, 0.7, 0.6));
        match i % 3 {
            0 => spawn_pg(commands, sphere_mesh.clone(), m, format!("Fall {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.4 * s), 1.0 + i as f32 * 0.5),
            1 => spawn_pg(commands, box_mesh.clone(), m, format!("Fall {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.35 * s)), 1.5 + i as f32 * 0.3),
            _ => spawn_pg(commands, capsule_mesh.clone(), m, format!("Fall {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::capsule(0.25 * s, 0.4 * s), 1.2),
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_slingshot(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    // Y-frame
    let frame_mat = mat(materials, Color::srgb(0.45, 0.35, 0.2));
    let base_mesh = meshes.add(Cuboid::new(0.3 * s, 3.0 * s, 0.3 * s));
    spawn_pg(commands, base_mesh, frame_mat.clone(), "Frame Base".into(),
        Transform::from_translation(Vec3::new(-6.0 * s, 1.5 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.15 * s, 1.5 * s, 0.15 * s)), 1.0);
    let arm_mesh = meshes.add(Cuboid::new(0.2 * s, 2.0 * s, 0.2 * s));
    for side in [-1.0f32, 1.0] {
        spawn_pg(commands, arm_mesh.clone(), frame_mat.clone(), "Frame Arm".into(),
            Transform::from_translation(Vec3::new(-6.0 * s + side * 0.8 * s, 3.5 * s, 0.0))
                .with_rotation(Quat::from_rotation_z(side * -0.3)),
            PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.0 * s, 0.1 * s)), 1.0);
    }
    // Projectiles
    let proj_mesh = meshes.add(Sphere::new(0.3 * s));
    for i in 0..3u32 {
        spawn_pg(commands, proj_mesh.clone(), mat(materials, Color::hsl(i as f32 * 100.0, 0.8, 0.5)),
            format!("Ammo {}", i),
            Transform::from_translation(Vec3::new(-6.0 * s, 4.5 * s + i as f32 * 0.8 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 2.0);
    }
    // Target wall
    let box_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    for row in 0..4u32 {
        for col in 0..4u32 {
            spawn_pg(commands, box_mesh.clone(), mat(materials, Color::hsl((row * 4 + col) as f32 * 20.0, 0.6, 0.6)),
                format!("Target {},{}", row, col),
                Transform::from_translation(Vec3::new(
                    8.0 * s,
                    0.4 * s + row as f32 * 0.85 * s,
                    (col as f32 - 1.5) * 0.85 * s,
                )),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.4 * s)), 1.0);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

// ============================================================================
// Arena command processing
// ============================================================================

fn process_arena_commands(
    mut state: ResMut<ArenaPresetsState>,
    mut commands: Commands,
    arena_entities: Query<Entity, With<ArenaEntity>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    state.arena_entity_count = arena_entities.iter().count();

    let cmds: Vec<ArenaCommand> = state.commands.drain(..).collect();
    if cmds.is_empty() { return; }

    let scale = state.scale;
    let arena_type = state.arena_type;

    for cmd in cmds {
        match cmd {
            ArenaCommand::Spawn => {
                // Clear existing arena first
                for entity in arena_entities.iter() {
                    commands.entity(entity).despawn();
                }
                spawn_arena(&mut commands, &mut meshes, &mut materials, arena_type, scale);
                state.has_active_arena = true;
            }
            ArenaCommand::Clear => {
                for entity in arena_entities.iter() {
                    commands.entity(entity).despawn();
                }
                state.has_active_arena = false;
                state.arena_entity_count = 0;
            }
        }
    }
}

// ============================================================================
// Spawn helpers
// ============================================================================

fn shape_mesh_and_collider(shape: PlaygroundShape) -> (Mesh, CollisionShapeData) {
    match shape {
        PlaygroundShape::Sphere => (Sphere::new(0.5).into(), CollisionShapeData::sphere(0.5)),
        PlaygroundShape::Box => (Cuboid::new(1.0, 1.0, 1.0).into(), CollisionShapeData::cuboid(Vec3::splat(0.5))),
        PlaygroundShape::Capsule => (Capsule3d::new(0.3, 1.0).into(), CollisionShapeData::capsule(0.3, 0.5)),
        PlaygroundShape::Cylinder => (Cylinder::new(0.4, 1.0).into(), CollisionShapeData::cylinder(0.4, 0.5)),
    }
}

fn generate_positions(pattern: SpawnPattern, count: u32, height: f32) -> Vec<Vec3> {
    let count = count.max(1) as usize;
    match pattern {
        SpawnPattern::Single => vec![Vec3::new(0.0, height, 0.0)],
        SpawnPattern::Stack => (0..count).map(|i| Vec3::new(0.0, height + i as f32 * 1.2, 0.0)).collect(),
        SpawnPattern::Wall => {
            let cols = (count as f32).sqrt().ceil() as usize;
            let rows = (count + cols - 1) / cols;
            let mut positions = Vec::with_capacity(count);
            for row in 0..rows {
                for col in 0..cols {
                    if positions.len() >= count { break; }
                    positions.push(Vec3::new(
                        (col as f32 - cols as f32 / 2.0) * 1.2,
                        height + row as f32 * 1.2,
                        0.0,
                    ));
                }
            }
            positions
        }
        SpawnPattern::Rain => {
            let spread = (count as f32).sqrt() * 2.0;
            (0..count).map(|i| {
                let angle = i as f32 * 2.399;
                let radius = (i as f32 / count as f32).sqrt() * spread;
                Vec3::new(angle.cos() * radius, height + (i as f32 * 0.3), angle.sin() * radius)
            }).collect()
        }
        SpawnPattern::Pyramid => {
            let mut positions = Vec::new();
            let layers = (count as f32).cbrt().ceil() as usize;
            let mut remaining = count;
            for layer in 0..layers {
                let size = layers - layer;
                for x in 0..size {
                    for z in 0..size {
                        if remaining == 0 { break; }
                        remaining -= 1;
                        let offset = -(size as f32 - 1.0) / 2.0;
                        positions.push(Vec3::new(
                            (x as f32 + offset) * 1.2,
                            height + layer as f32 * 1.2,
                            (z as f32 + offset) * 1.2,
                        ));
                    }
                }
            }
            positions
        }
        SpawnPattern::Explosion => {
            (0..count).map(|i| {
                let phi = (i as f32 / count as f32) * std::f32::consts::PI * 2.0;
                let theta = (i as f32 * 2.399) % std::f32::consts::PI;
                let r = 0.5;
                Vec3::new(r * theta.sin() * phi.cos(), height + r * theta.cos(), r * theta.sin() * phi.sin())
            }).collect()
        }
    }
}

// ============================================================================
// Scenario spawning
// ============================================================================

fn spawn_scenario(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, scenario: ScenarioType, s: f32) {
    match scenario {
        ScenarioType::StackTest => spawn_stack_test(commands, meshes, materials, s),
        ScenarioType::DominoChain => spawn_domino_chain(commands, meshes, materials, s),
        ScenarioType::BilliardBreak => spawn_billiard_break(commands, meshes, materials, s),
        ScenarioType::InclinedPlane => spawn_inclined_plane(commands, meshes, materials, s),
        ScenarioType::Avalanche => spawn_avalanche(commands, meshes, materials, s),
        ScenarioType::WreckingBall => spawn_wrecking_ball(commands, meshes, materials, s),
        ScenarioType::NewtonsCradle => spawn_newtons_cradle(commands, meshes, materials, s),
        ScenarioType::Pendulum => spawn_pendulum(commands, meshes, materials, s),
        ScenarioType::ProjectileLaunch => spawn_projectile(commands, meshes, materials, s),
        ScenarioType::Gauntlet => spawn_gauntlet(commands, meshes, materials, s),
        ScenarioType::WedgeStress => spawn_wedge_stress(commands, meshes, materials, s),
        ScenarioType::BowlingAlley => spawn_bowling(commands, meshes, materials, s),
        ScenarioType::Catapult => spawn_catapult(commands, meshes, materials, s),
        ScenarioType::BalancingAct => spawn_balancing_act(commands, meshes, materials, s),
        ScenarioType::Marbles => spawn_marbles(commands, meshes, materials, s),
        ScenarioType::Jenga => spawn_jenga(commands, meshes, materials, s),
        ScenarioType::Plinko => spawn_plinko(commands, meshes, materials, s),
        ScenarioType::Conveyor => spawn_conveyor(commands, meshes, materials, s),
        ScenarioType::Trebuchet => spawn_trebuchet(commands, meshes, materials, s),
        ScenarioType::RubeGoldberg => spawn_rube_goldberg(commands, meshes, materials, s),
        ScenarioType::Waterfall => spawn_waterfall(commands, meshes, materials, s),
        ScenarioType::Cannon => spawn_cannon(commands, meshes, materials, s),
        ScenarioType::Bridge => spawn_bridge(commands, meshes, materials, s),
        ScenarioType::Elevator => spawn_elevator(commands, meshes, materials, s),
        ScenarioType::Pinball => spawn_pinball_scenario(commands, meshes, materials, s),
        ScenarioType::Spinner => spawn_spinner(commands, meshes, materials, s),
        ScenarioType::Hourglass => spawn_hourglass(commands, meshes, materials, s),
        ScenarioType::MassComparison => spawn_mass_comparison(commands, meshes, materials, s),
        ScenarioType::Ricochet => spawn_ricochet(commands, meshes, materials, s),
        ScenarioType::BoxFort => spawn_box_fort(commands, meshes, materials, s),
        ScenarioType::Spiral => spawn_spiral_drop(commands, meshes, materials, s),
        ScenarioType::Trampoline => spawn_trampoline(commands, meshes, materials, s),
        ScenarioType::ChainReaction => spawn_chain_reaction(commands, meshes, materials, s),
        ScenarioType::Freefall => spawn_freefall(commands, meshes, materials, s),
        ScenarioType::Slingshot => spawn_slingshot(commands, meshes, materials, s),
    }
}

fn spawn_pg(commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>, name: String, transform: Transform, body_type: PhysicsBodyType, shape: CollisionShapeData, mass: f32) {
    let body_data = PhysicsBodyData { body_type, mass, ..default() };
    let entity = commands.spawn((
        ScenarioEntity, Name::new(name), Mesh3d(mesh), MeshMaterial3d(material),
        transform, Visibility::default(), body_data.clone(), shape.clone(),
    )).id();
    renzora_physics::spawn_entity_physics(commands, entity, Some(&body_data), Some(&shape));
}

fn mat(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: color, ..default() })
}

fn spawn_ground(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let mesh = meshes.add(Cuboid::new(50.0 * s, 0.1, 50.0 * s));
    let material = mat(materials, Color::srgb(0.35, 0.35, 0.35));
    spawn_pg(commands, mesh, material, "Scenario Ground".into(),
        Transform::from_translation(Vec3::new(0.0, -0.05, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(25.0 * s, 0.05, 25.0 * s)), 1.0);
}

fn spawn_stack_test(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    for i in 0..15 {
        let hue = (i as f32 / 15.0) * 360.0;
        let m = mat(materials, Color::hsl(hue, 0.7, 0.6));
        spawn_pg(commands, box_mesh.clone(), m, format!("Stack Box {}", i),
            Transform::from_translation(Vec3::new(0.0, 0.5 * s + i as f32 * 1.05 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.5 * s)), 1.0);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_domino_chain(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let mesh = meshes.add(Cuboid::new(0.1 * s, 1.0 * s, 0.5 * s));
    let m = mat(materials, Color::hsl(30.0, 0.8, 0.5));
    for i in 0..20 {
        let angle = i as f32 * 0.15;
        let radius = 3.0 * s + i as f32 * 0.3 * s;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let facing = angle + std::f32::consts::FRAC_PI_2;
        let tilt = if i == 0 { Quat::from_rotation_z(0.15) * Quat::from_rotation_y(facing) } else { Quat::from_rotation_y(facing) };
        spawn_pg(commands, mesh.clone(), m.clone(), format!("Domino {}", i),
            Transform::from_translation(Vec3::new(x, 0.5 * s, z)).with_rotation(tilt),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::new(0.05 * s, 0.5 * s, 0.25 * s)), 0.5);
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_billiard_break(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let mesh = meshes.add(Sphere::new(0.3 * s));
    let r = 0.3 * s;
    let spacing = r * 2.1;
    let ball_mat = mat(materials, Color::hsl(45.0, 0.8, 0.5));
    let mut shape = CollisionShapeData::sphere(r);
    shape.restitution = 0.95;
    shape.friction = 0.1;

    for row in 0..5 {
        for col in 0..=row {
            let x = (col as f32 - row as f32 / 2.0) * spacing;
            let z = row as f32 * spacing * 0.866;
            spawn_pg(commands, mesh.clone(), ball_mat.clone(), format!("Ball {},{}", row, col),
                Transform::from_translation(Vec3::new(x, r, z)),
                PhysicsBodyType::RigidBody, shape.clone(), 1.0);
        }
    }

    let cue_mat = mat(materials, Color::WHITE);
    spawn_pg(commands, mesh, cue_mat, "Cue Ball".into(),
        Transform::from_translation(Vec3::new(0.0, r, -5.0 * s)),
        PhysicsBodyType::RigidBody, shape, 1.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_inclined_plane(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let ramp_mesh = meshes.add(Cuboid::new(4.0 * s, 0.2 * s, 6.0 * s));
    spawn_pg(commands, ramp_mesh, mat(materials, Color::srgb(0.6, 0.6, 0.6)), "Ramp".into(),
        Transform::from_translation(Vec3::new(0.0, 3.0 * s, 0.0)).with_rotation(Quat::from_rotation_x(0.4)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.0 * s, 0.1 * s, 3.0 * s)), 1.0);

    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.8 * s, 0.8 * s, 0.8 * s));
    spawn_pg(commands, sphere_mesh, mat(materials, Color::hsl(120.0, 0.7, 0.5)), "Ramp Sphere".into(),
        Transform::from_translation(Vec3::new(-0.5 * s, 5.0 * s, -1.0 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.4 * s), 1.0);
    spawn_pg(commands, box_mesh, mat(materials, Color::hsl(240.0, 0.7, 0.5)), "Ramp Box".into(),
        Transform::from_translation(Vec3::new(0.5 * s, 5.5 * s, -1.5 * s)),
        PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.4 * s)), 2.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_avalanche(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let slope_mesh = meshes.add(Cuboid::new(8.0 * s, 0.3 * s, 8.0 * s));
    spawn_pg(commands, slope_mesh, mat(materials, Color::srgb(0.45, 0.4, 0.35)), "Slope".into(),
        Transform::from_translation(Vec3::new(0.0, 5.0 * s, 0.0)).with_rotation(Quat::from_rotation_x(0.6)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(4.0 * s, 0.15 * s, 4.0 * s)), 1.0);

    let sphere_mesh = meshes.add(Sphere::new(0.4 * s));
    let box_mesh = meshes.add(Cuboid::new(0.7 * s, 0.7 * s, 0.7 * s));
    for i in 0..30 {
        let angle = i as f32 * 2.399;
        let radius = (i as f32 / 30.0).sqrt() * 3.0 * s;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let y = 8.0 * s + (i as f32 * 0.2) * s;
        let m = mat(materials, Color::hsl((i as f32 * 17.0) % 360.0, 0.7, 0.6));
        if i % 2 == 0 {
            spawn_pg(commands, sphere_mesh.clone(), m, format!("Avalanche Sphere {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.4 * s), 1.0);
        } else {
            spawn_pg(commands, box_mesh.clone(), m, format!("Avalanche Box {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.35 * s)), 1.5);
        }
    }
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_wrecking_ball(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let box_mesh = meshes.add(Cuboid::new(1.0 * s, 1.0 * s, 1.0 * s));
    let wall_mat = mat(materials, Color::hsl(0.0, 0.6, 0.6));
    for row in 0..5 {
        for col in 0..4 {
            spawn_pg(commands, box_mesh.clone(), wall_mat.clone(), format!("Wall {},{}", row, col),
                Transform::from_translation(Vec3::new((col as f32 - 1.5) * 1.1 * s, 0.5 * s + row as f32 * 1.1 * s, 0.0)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.5 * s)), 1.0);
        }
    }
    let ball_mesh = meshes.add(Sphere::new(1.5 * s));
    spawn_pg(commands, ball_mesh, mat(materials, Color::srgb(0.3, 0.3, 0.3)), "Wrecking Ball".into(),
        Transform::from_translation(Vec3::new(-8.0 * s, 6.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(1.5 * s), 50.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_newtons_cradle(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let sphere_mesh = meshes.add(Sphere::new(0.5 * s));
    let m = mat(materials, Color::hsl(200.0, 0.7, 0.6));
    let r = 0.5 * s;
    let spacing = r * 2.05;
    for i in 0..5 {
        let x = (i as f32 - 2.0) * spacing;
        let y = 5.0 * s;
        let (px, py) = if i == 0 { (x - 3.0 * s, y + 1.0 * s) } else { (x, y) };
        spawn_pg(commands, sphere_mesh.clone(), m.clone(), format!("Cradle Ball {}", i),
            Transform::from_translation(Vec3::new(px, py, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(r), 1.0);
    }
    let bar_mesh = meshes.add(Cuboid::new(spacing * 6.0, 0.2 * s, 0.2 * s));
    spawn_pg(commands, bar_mesh, mat(materials, Color::srgb(0.5, 0.5, 0.5)), "Cradle Anchor".into(),
        Transform::from_translation(Vec3::new(0.0, 8.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(spacing * 3.0, 0.1 * s, 0.1 * s)), 1.0);
}

fn spawn_pendulum(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let anchor_mesh = meshes.add(Cuboid::new(0.3 * s, 0.3 * s, 0.3 * s));
    spawn_pg(commands, anchor_mesh, mat(materials, Color::srgb(0.5, 0.5, 0.5)), "Pendulum Anchor".into(),
        Transform::from_translation(Vec3::new(0.0, 8.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::splat(0.15 * s)), 1.0);
    let bob_mesh = meshes.add(Sphere::new(0.6 * s));
    spawn_pg(commands, bob_mesh, mat(materials, Color::hsl(280.0, 0.7, 0.6)), "Pendulum Bob".into(),
        Transform::from_translation(Vec3::new(4.0 * s, 5.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.6 * s), 2.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_projectile(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let pad_mesh = meshes.add(Cuboid::new(2.0 * s, 0.5 * s, 2.0 * s));
    spawn_pg(commands, pad_mesh, mat(materials, Color::srgb(0.4, 0.4, 0.4)), "Launch Pad".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 0.25 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(1.0 * s, 0.25 * s, 1.0 * s)), 1.0);
    let proj_mesh = meshes.add(Sphere::new(0.3 * s));
    spawn_pg(commands, proj_mesh, mat(materials, Color::hsl(15.0, 0.9, 0.5)), "Projectile".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 1.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.3 * s), 1.0);
    let wall_mesh = meshes.add(Cuboid::new(0.5 * s, 4.0 * s, 4.0 * s));
    spawn_pg(commands, wall_mesh, mat(materials, Color::hsl(0.0, 0.5, 0.5)), "Target Wall".into(),
        Transform::from_translation(Vec3::new(10.0 * s, 2.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.25 * s, 2.0 * s, 2.0 * s)), 1.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_gauntlet(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let plat_mesh = meshes.add(Cuboid::new(6.0 * s, 0.3 * s, 3.0 * s));
    let plat_mat = mat(materials, Color::srgb(0.5, 0.5, 0.5));
    spawn_pg(commands, plat_mesh.clone(), plat_mat.clone(), "Gauntlet Ramp 1".into(),
        Transform::from_translation(Vec3::new(-6.0 * s, 4.0 * s, 0.0)).with_rotation(Quat::from_rotation_z(-0.3)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.15 * s, 1.5 * s)), 1.0);
    spawn_pg(commands, plat_mesh.clone(), plat_mat.clone(), "Gauntlet Platform".into(),
        Transform::from_translation(Vec3::new(2.0 * s, 2.0 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.15 * s, 1.5 * s)), 1.0);
    spawn_pg(commands, plat_mesh, plat_mat, "Gauntlet Ramp 2".into(),
        Transform::from_translation(Vec3::new(10.0 * s, 1.0 * s, 0.0)).with_rotation(Quat::from_rotation_z(-0.2)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 0.15 * s, 1.5 * s)), 1.0);

    let obs_mesh = meshes.add(Sphere::new(0.8 * s));
    let obs_mat = mat(materials, Color::hsl(0.0, 0.8, 0.5));
    for i in 0..3 {
        spawn_pg(commands, obs_mesh.clone(), obs_mat.clone(), format!("Gauntlet Obstacle {}", i),
            Transform::from_translation(Vec3::new(-2.0 * s + i as f32 * 4.0 * s, 6.0 * s, 0.0)),
            PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.8 * s), 5.0);
    }

    let ball_mesh = meshes.add(Sphere::new(0.5 * s));
    spawn_pg(commands, ball_mesh, mat(materials, Color::hsl(180.0, 0.8, 0.5)), "Gauntlet Ball".into(),
        Transform::from_translation(Vec3::new(-8.0 * s, 6.0 * s, 0.0)),
        PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.5 * s), 1.0);
    spawn_ground(commands, meshes, materials, s);
}

fn spawn_wedge_stress(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let wall_mat = mat(materials, Color::srgb(0.5, 0.5, 0.55));
    let wedge_wall = meshes.add(Cuboid::new(0.2 * s, 3.0 * s, 8.0 * s));

    spawn_pg(commands, wedge_wall.clone(), wall_mat.clone(), "V-Wedge Left".into(),
        Transform::from_translation(Vec3::new(-6.0 * s, 1.5 * s, 0.0)).with_rotation(Quat::from_rotation_z(0.55)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.5 * s, 4.0 * s)), 1.0);
    spawn_pg(commands, wedge_wall, wall_mat.clone(), "V-Wedge Right".into(),
        Transform::from_translation(Vec3::new(-4.0 * s, 1.5 * s, 0.0)).with_rotation(Quat::from_rotation_z(-0.55)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.5 * s, 4.0 * s)), 1.0);

    let sphere_mesh = meshes.add(Sphere::new(0.35 * s));
    let box_mesh = meshes.add(Cuboid::new(0.6 * s, 0.6 * s, 0.6 * s));
    let capsule_mesh = meshes.add(Capsule3d::new(0.25 * s, 0.8 * s));

    for i in 0..6 {
        let z = (i as f32 - 2.5) * 1.2 * s;
        let y = 5.0 * s + i as f32 * 0.5 * s;
        let m = mat(materials, Color::hsl(i as f32 * 50.0, 0.75, 0.55));
        match i % 3 {
            0 => spawn_pg(commands, sphere_mesh.clone(), m, format!("V-Wedge Sphere {}", i),
                Transform::from_translation(Vec3::new(-5.0 * s, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.35 * s), 1.0),
            1 => spawn_pg(commands, box_mesh.clone(), m, format!("V-Wedge Box {}", i),
                Transform::from_translation(Vec3::new(-5.0 * s, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.3 * s)), 1.5),
            _ => spawn_pg(commands, capsule_mesh.clone(), m, format!("V-Wedge Capsule {}", i),
                Transform::from_translation(Vec3::new(-5.0 * s, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::capsule(0.25 * s, 0.4 * s), 1.2),
        }
    }

    let corner_wall = meshes.add(Cuboid::new(6.0 * s, 3.0 * s, 0.2 * s));
    let side_wall = meshes.add(Cuboid::new(0.2 * s, 3.0 * s, 6.0 * s));
    spawn_pg(commands, corner_wall, wall_mat.clone(), "Corner Back Wall".into(),
        Transform::from_translation(Vec3::new(3.0 * s, 1.5 * s, -3.0 * s)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(3.0 * s, 1.5 * s, 0.1 * s)), 1.0);
    spawn_pg(commands, side_wall, wall_mat.clone(), "Corner Side Wall".into(),
        Transform::from_translation(Vec3::new(0.0, 1.5 * s, 0.0)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.5 * s, 3.0 * s)), 1.0);

    for i in 0..4 {
        let m = mat(materials, Color::hsl(200.0 + i as f32 * 30.0, 0.7, 0.55));
        spawn_pg(commands, box_mesh.clone(), m, format!("Corner Box {}", i),
            Transform::from_translation(Vec3::new(1.5 * s + i as f32 * 1.0 * s, 0.5 * s, -1.0 * s)),
            PhysicsBodyType::RigidBody, CollisionShapeData::cuboid(Vec3::splat(0.3 * s)), 2.0);
    }

    let funnel_wall = meshes.add(Cuboid::new(0.2 * s, 2.0 * s, 10.0 * s));
    spawn_pg(commands, funnel_wall.clone(), wall_mat.clone(), "Funnel Left".into(),
        Transform::from_translation(Vec3::new(11.0 * s, 1.0 * s, 0.0)).with_rotation(Quat::from_rotation_y(0.2)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.0 * s, 5.0 * s)), 1.0);
    spawn_pg(commands, funnel_wall, wall_mat.clone(), "Funnel Right".into(),
        Transform::from_translation(Vec3::new(15.0 * s, 1.0 * s, 0.0)).with_rotation(Quat::from_rotation_y(-0.2)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(0.1 * s, 1.0 * s, 5.0 * s)), 1.0);

    let cap_mesh = meshes.add(Cuboid::new(4.0 * s, 2.0 * s, 0.2 * s));
    spawn_pg(commands, cap_mesh, wall_mat, "Funnel Cap".into(),
        Transform::from_translation(Vec3::new(13.0 * s, 1.0 * s, 5.0 * s)),
        PhysicsBodyType::StaticBody, CollisionShapeData::cuboid(Vec3::new(2.0 * s, 1.0 * s, 0.1 * s)), 1.0);

    for i in 0..8 {
        let x = 12.0 * s + (i as f32 % 3.0) * 0.8 * s;
        let z = -4.0 * s + (i as f32 / 3.0).floor() * 0.9 * s;
        let y = 0.5 * s + (i as f32 % 2.0) * 0.6 * s;
        let m = mat(materials, Color::hsl((i as f32 * 40.0 + 120.0) % 360.0, 0.8, 0.55));
        if i % 2 == 0 {
            spawn_pg(commands, sphere_mesh.clone(), m, format!("Funnel Sphere {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::sphere(0.35 * s), 1.0);
        } else {
            spawn_pg(commands, capsule_mesh.clone(), m, format!("Funnel Capsule {}", i),
                Transform::from_translation(Vec3::new(x, y, z)),
                PhysicsBodyType::RigidBody, CollisionShapeData::capsule(0.25 * s, 0.4 * s), 1.2);
        }
    }

    spawn_ground(commands, meshes, materials, s);
}

// ============================================================================
// Arena spawning
// ============================================================================

fn spawn_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, arena_type: ArenaType, s: f32) {
    match arena_type {
        ArenaType::Walled => spawn_walled_arena(commands, meshes, materials, s),
        ArenaType::StairFall => spawn_stair_fall(commands, meshes, materials, s),
        ArenaType::MovingPlatforms => spawn_moving_platforms(commands, meshes, materials, s),
        ArenaType::HillSlide => spawn_hill_slide(commands, meshes, materials, s),
        ArenaType::Pinball => spawn_pinball_arena(commands, meshes, materials, s),
        ArenaType::GolfCourse => spawn_golf_course(commands, meshes, materials, s),
        ArenaType::Funnel => spawn_funnel_arena(commands, meshes, materials, s),
        ArenaType::Colosseum => spawn_colosseum(commands, meshes, materials, s),
        ArenaType::Maze => spawn_maze(commands, meshes, materials, s),
        ArenaType::Halfpipe => spawn_halfpipe(commands, meshes, materials, s),
        ArenaType::TieredPlatforms => spawn_tiered_platforms(commands, meshes, materials, s),
        ArenaType::Canyon => spawn_canyon(commands, meshes, materials, s),
        ArenaType::Fortress => spawn_fortress(commands, meshes, materials, s),
        ArenaType::Spiral => spawn_spiral_arena(commands, meshes, materials, s),
        ArenaType::Bowl => spawn_bowl_arena(commands, meshes, materials, s),
        ArenaType::Pillars => spawn_pillars_arena(commands, meshes, materials, s),
        ArenaType::IceRink => spawn_ice_rink(commands, meshes, materials, s),
        ArenaType::Volcano => spawn_volcano(commands, meshes, materials, s),
        ArenaType::Labyrinth => spawn_labyrinth(commands, meshes, materials, s),
    }
}

fn spawn_arena_static(
    commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>,
    name: String, transform: Transform, shape: CollisionShapeData,
) {
    let body_data = PhysicsBodyData { body_type: PhysicsBodyType::StaticBody, mass: 1.0, ..default() };
    let entity = commands.spawn((
        ArenaEntity, Name::new(name), Mesh3d(mesh), MeshMaterial3d(material),
        transform, Visibility::default(), body_data.clone(), shape.clone(),
    )).id();
    renzora_physics::spawn_entity_physics(commands, entity, Some(&body_data), Some(&shape));
}

fn arena_mat(materials: &mut Assets<StandardMaterial>, color: Color) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial { base_color: color, ..default() })
}

fn spawn_walled_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let arena = 20.0 * s;
    let wall_mat = arena_mat(materials, Color::srgba(0.3, 0.3, 0.35, 0.8));
    let ramp_mat = arena_mat(materials, Color::srgba(0.4, 0.35, 0.25, 0.9));
    let obs_mat = arena_mat(materials, Color::srgba(0.5, 0.25, 0.25, 0.9));

    // Floor
    spawn_arena_static(commands, meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0)), wall_mat.clone(),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(arena, 0.25, arena)));

    // Walls
    let wh = 4.0 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
    ] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0)), wall_mat.clone(),
            "Wall".into(), Transform::from_translation(pos), CollisionShapeData::cuboid(half));
    }

    // Ramps
    let rl = arena * 0.6;
    let rw = arena * 0.4;
    for (pos, rot) in [
        (Vec3::new(arena * 0.4, 1.5 * s, 0.0), Quat::from_rotation_z(-0.25)),
        (Vec3::new(-arena * 0.4, 1.5 * s, 0.0), Quat::from_rotation_z(0.25)),
    ] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(rl, 0.3, rw)), ramp_mat.clone(),
            "Ramp".into(), Transform::from_translation(pos).with_rotation(rot),
            CollisionShapeData::cuboid(Vec3::new(rl * 0.5, 0.15, rw * 0.5)));
    }

    // Pillars
    for i in 0..8u32 {
        let angle = (i as f32 / 8.0) * std::f32::consts::TAU;
        let dist = arena * 0.3;
        let pos = Vec3::new(angle.cos() * dist, 1.5 * s, angle.sin() * dist);
        if i % 2 == 0 {
            spawn_arena_static(commands, meshes.add(Cylinder::new(0.8 * s, 3.0 * s)), obs_mat.clone(),
                format!("Pillar {}", i), Transform::from_translation(pos),
                CollisionShapeData::cylinder(0.8 * s, 1.5 * s));
        } else {
            spawn_arena_static(commands, meshes.add(Cuboid::new(1.5 * s, 2.0 * s, 1.5 * s)), obs_mat.clone(),
                format!("Obstacle {}", i), Transform::from_translation(pos),
                CollisionShapeData::cuboid(Vec3::splat(0.75 * s)));
        }
    }
}

fn spawn_stair_fall(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let step_count = 20;
    let step_w = 20.0 * s;
    let step_d = 2.0 * s;
    let step_h = 0.6 * s;
    let stair_mat = arena_mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let rail_mat = arena_mat(materials, Color::srgb(0.5, 0.3, 0.2));

    for i in 0..step_count {
        let y = i as f32 * step_h;
        let z = i as f32 * step_d;
        spawn_arena_static(commands, meshes.add(Cuboid::new(step_w, step_h, step_d)), stair_mat.clone(),
            format!("Step {}", i), Transform::from_translation(Vec3::new(0.0, y + step_h * 0.5, z)),
            CollisionShapeData::cuboid(Vec3::new(step_w * 0.5, step_h * 0.5, step_d * 0.5)));
    }

    // Side rails
    let total_len = step_count as f32 * step_d;
    let total_h = step_count as f32 * step_h;
    let rail_angle = (total_h / total_len).atan();
    let rail_len = (total_len * total_len + total_h * total_h).sqrt();
    for side in [-1.0f32, 1.0] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(0.3, 2.0 * s, rail_len)), rail_mat.clone(),
            "Rail".into(),
            Transform::from_translation(Vec3::new(side * step_w * 0.5, total_h * 0.5 + 1.0 * s, total_len * 0.5))
                .with_rotation(Quat::from_rotation_x(-rail_angle)),
            CollisionShapeData::cuboid(Vec3::new(0.15, 1.0 * s, rail_len * 0.5)));
    }

    // Landing
    spawn_arena_static(commands, meshes.add(Cuboid::new(step_w * 1.5, 0.5, step_w)), stair_mat,
        "Landing".into(), Transform::from_translation(Vec3::new(0.0, -0.25, -step_w * 0.3)),
        CollisionShapeData::cuboid(Vec3::new(step_w * 0.75, 0.25, step_w * 0.5)));
}

fn spawn_moving_platforms(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let plat_mat = arena_mat(materials, Color::srgb(0.2, 0.5, 0.6));
    let floor_mat = arena_mat(materials, Color::srgb(0.3, 0.3, 0.35));
    let arena = 20.0 * s;

    // Ground
    spawn_arena_static(commands, meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0)), floor_mat.clone(),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(arena, 0.25, arena)));

    // Platforms (static for now — kinematic animation requires backend-specific code)
    let pw = arena * 0.4;
    let pd = arena * 0.3;
    for i in 0..6u32 {
        let y = 2.0 * s + i as f32 * 2.5 * s;
        let z = (i as f32 - 3.0) * pd * 0.8;
        spawn_arena_static(commands, meshes.add(Cuboid::new(pw, 0.4, pd)), plat_mat.clone(),
            format!("Platform {}", i), Transform::from_translation(Vec3::new(0.0, y, z)),
            CollisionShapeData::cuboid(Vec3::new(pw * 0.5, 0.2, pd * 0.5)));
    }

    // Walls
    let wh = 15.0 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
    ] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0)), floor_mat.clone(),
            "Wall".into(), Transform::from_translation(pos), CollisionShapeData::cuboid(half));
    }
}

fn spawn_hill_slide(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let hill_len = 20.0 * s;
    let hill_w = 20.0 * s;
    let slope_angle: f32 = 0.35;
    let hill_mat = arena_mat(materials, Color::srgb(0.35, 0.5, 0.25));
    let bump_mat = arena_mat(materials, Color::srgb(0.45, 0.35, 0.25));
    let wall_mat = arena_mat(materials, Color::srgb(0.3, 0.3, 0.35));

    let slope_len = hill_len * 1.5;
    let slope_rise = slope_len * slope_angle.sin();

    // Main slope
    spawn_arena_static(commands, meshes.add(Cuboid::new(hill_w, 0.5, slope_len)), hill_mat.clone(),
        "Slope".into(),
        Transform::from_translation(Vec3::new(0.0, slope_rise * 0.5, 0.0))
            .with_rotation(Quat::from_rotation_x(-slope_angle)),
        CollisionShapeData::cuboid(Vec3::new(hill_w * 0.5, 0.25, slope_len * 0.5)));

    // Landing
    let landing_z = slope_len * slope_angle.cos() * 0.5 + hill_w * 0.5;
    spawn_arena_static(commands, meshes.add(Cuboid::new(hill_w * 1.5, 0.5, hill_w)), hill_mat,
        "Landing".into(), Transform::from_translation(Vec3::new(0.0, -0.25, landing_z)),
        CollisionShapeData::cuboid(Vec3::new(hill_w * 0.75, 0.25, hill_w * 0.5)));

    // Bumps
    let bump_count = (hill_len as usize / 3).clamp(3, 12);
    for i in 0..bump_count {
        let t = (i as f32 + 0.5) / bump_count as f32;
        let z = (t - 0.5) * slope_len * slope_angle.cos();
        let y = (t - 0.5) * slope_len * slope_angle.sin() + slope_rise * 0.5 + 0.5;
        let x = ((i as f32 * 3.7).sin()) * hill_w * 0.3;
        spawn_arena_static(commands, meshes.add(Sphere::new(0.8 * s)), bump_mat.clone(),
            format!("Bump {}", i), Transform::from_translation(Vec3::new(x, y, z)),
            CollisionShapeData::sphere(0.8 * s));
    }

    // Side rails
    for side in [-1.0f32, 1.0] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(0.3, 1.5 * s, slope_len)), wall_mat.clone(),
            "Rail".into(),
            Transform::from_translation(Vec3::new(side * hill_w * 0.5, slope_rise * 0.5 + 0.75 * s, 0.0))
                .with_rotation(Quat::from_rotation_x(-slope_angle)),
            CollisionShapeData::cuboid(Vec3::new(0.15, 0.75 * s, slope_len * 0.5)));
    }
}

fn spawn_pinball_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let board_w = 20.0 * s;
    let board_h = board_w * 2.0;
    let tilt: f32 = 0.15;
    let board_mat = arena_mat(materials, Color::srgb(0.15, 0.2, 0.15));
    let bumper_mat = arena_mat(materials, Color::srgb(0.8, 0.2, 0.2));
    let wall_mat = arena_mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let defl_mat = arena_mat(materials, Color::srgb(0.6, 0.5, 0.1));

    // Back board
    spawn_arena_static(commands, meshes.add(Cuboid::new(board_w, board_h, 0.3)), board_mat,
        "Board".into(),
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, -1.0 * s))
            .with_rotation(Quat::from_rotation_x(tilt)),
        CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, board_h * 0.5, 0.15)));

    // Front glass
    let glass_mat = arena_mat(materials, Color::srgba(0.5, 0.5, 0.6, 0.2));
    spawn_arena_static(commands, meshes.add(Cuboid::new(board_w, board_h, 0.1)), glass_mat,
        "Glass".into(),
        Transform::from_translation(Vec3::new(0.0, board_h * 0.5, 0.8 * s))
            .with_rotation(Quat::from_rotation_x(tilt)),
        CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, board_h * 0.5, 0.05)));

    // Sides
    for side in [-1.0f32, 1.0] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(0.3, board_h, 2.0 * s)), wall_mat.clone(),
            "Side".into(),
            Transform::from_translation(Vec3::new(side * board_w * 0.5, board_h * 0.5, 0.0))
                .with_rotation(Quat::from_rotation_x(tilt)),
            CollisionShapeData::cuboid(Vec3::new(0.15, board_h * 0.5, 1.0 * s)));
    }

    // Bottom
    spawn_arena_static(commands, meshes.add(Cuboid::new(board_w, 0.5, 2.0 * s)), wall_mat,
        "Bottom".into(), Transform::from_translation(Vec3::new(0.0, 0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(board_w * 0.5, 0.25, 1.0 * s)));

    // Bumpers
    for i in 0..10u32 {
        let bx = ((i as f32 * 2.399).cos()) * board_w * 0.35;
        let by = 2.0 * s + (i as f32 / 10.0) * (board_h - 4.0 * s);
        spawn_arena_static(commands, meshes.add(Cylinder::new(0.6 * s, 1.5 * s)), bumper_mat.clone(),
            format!("Bumper {}", i),
            Transform::from_translation(Vec3::new(bx, by, 0.0))
                .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
            CollisionShapeData::cylinder(0.6 * s, 0.75 * s));
    }

    // Deflectors
    for (pos, angle) in [
        (Vec3::new(-board_w * 0.25, board_h * 0.2, 0.0), 0.4f32),
        (Vec3::new(board_w * 0.25, board_h * 0.2, 0.0), -0.4),
        (Vec3::new(-board_w * 0.15, board_h * 0.55, 0.0), -0.3),
        (Vec3::new(board_w * 0.15, board_h * 0.55, 0.0), 0.3),
    ] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(board_w * 0.2, 0.2, 1.2 * s)), defl_mat.clone(),
            "Deflector".into(),
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_z(angle) * Quat::from_rotation_x(tilt)),
            CollisionShapeData::cuboid(Vec3::new(board_w * 0.1, 0.1, 0.6 * s)));
    }
}

fn spawn_golf_course(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let course_len = 40.0 * s;
    let course_w = 20.0 * s;
    let green_mat = arena_mat(materials, Color::srgb(0.2, 0.55, 0.2));
    let sand_mat = arena_mat(materials, Color::srgb(0.75, 0.65, 0.45));
    let wall_mat = arena_mat(materials, Color::srgb(0.3, 0.3, 0.35));

    // Fairway
    spawn_arena_static(commands, meshes.add(Cuboid::new(course_w, 0.5, course_len)), green_mat.clone(),
        "Fairway".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(course_w * 0.5, 0.25, course_len * 0.5)));

    // Hills
    let hill_count = (course_len as usize / 8).clamp(3, 10);
    for i in 0..hill_count {
        let z = (i as f32 / hill_count as f32 - 0.5) * course_len * 0.8;
        let x = ((i as f32 * 2.1).sin()) * course_w * 0.2;
        let hill_r = (2.0 + (i as f32 * 0.7).sin().abs() * 2.0) * s;
        spawn_arena_static(commands, meshes.add(Sphere::new(hill_r)), green_mat.clone(),
            format!("Hill {}", i),
            Transform::from_translation(Vec3::new(x, hill_r * 0.3, z)),
            CollisionShapeData::sphere(hill_r));
    }

    // Sand traps
    for i in 0..3u32 {
        let z = (i as f32 - 1.0) * course_len * 0.25;
        let x = ((i as f32 * 3.5 + 1.0).sin()) * course_w * 0.25;
        spawn_arena_static(commands, meshes.add(Cuboid::new(3.0 * s, 0.1, 3.0 * s)), sand_mat.clone(),
            format!("Sand Trap {}", i), Transform::from_translation(Vec3::new(x, 0.05, z)),
            CollisionShapeData::cuboid(Vec3::new(1.5 * s, 0.05, 1.5 * s)));
    }

    // Side fences
    for side in [-1.0f32, 1.0] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(0.3, 1.5 * s, course_len)), wall_mat.clone(),
            "Fence".into(), Transform::from_translation(Vec3::new(side * course_w * 0.5, 0.75 * s, 0.0)),
            CollisionShapeData::cuboid(Vec3::new(0.15, 0.75 * s, course_len * 0.5)));
    }

    // End walls
    for side in [-1.0f32, 1.0] {
        spawn_arena_static(commands, meshes.add(Cuboid::new(course_w, 1.5 * s, 0.3)), wall_mat.clone(),
            "End Wall".into(), Transform::from_translation(Vec3::new(0.0, 0.75 * s, side * course_len * 0.5)),
            CollisionShapeData::cuboid(Vec3::new(course_w * 0.5, 0.75 * s, 0.15)));
    }
}

fn spawn_funnel_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let funnel_r = 20.0 * s;
    let funnel_mat = arena_mat(materials, Color::srgb(0.35, 0.3, 0.4));
    let floor_mat = arena_mat(materials, Color::srgb(0.3, 0.3, 0.35));

    let segments = 16u32;
    let funnel_height = funnel_r * 0.8;
    let inner_r = 2.0 * s;
    let segment_angle = std::f32::consts::TAU / segments as f32;
    let segment_w = funnel_r * segment_angle * 1.1;
    let slope_len = ((funnel_r - inner_r).powi(2) + funnel_height.powi(2)).sqrt();
    let slope_angle = (funnel_height / (funnel_r - inner_r)).atan();

    for i in 0..segments {
        let angle = i as f32 * segment_angle;
        let mid_r = (funnel_r + inner_r) * 0.5;
        let cx = angle.cos() * mid_r;
        let cz = angle.sin() * mid_r;
        let cy = funnel_height * 0.5;

        spawn_arena_static(commands, meshes.add(Cuboid::new(segment_w, 0.2, slope_len)), funnel_mat.clone(),
            format!("Funnel Segment {}", i),
            Transform::from_translation(Vec3::new(cx, cy, cz))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(slope_angle)),
            CollisionShapeData::cuboid(Vec3::new(segment_w * 0.5, 0.1, slope_len * 0.5)));
    }

    // Collection floor
    spawn_arena_static(commands, meshes.add(Cuboid::new(inner_r * 4.0, 0.5, inner_r * 4.0)), floor_mat,
        "Collection Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(inner_r * 2.0, 0.25, inner_r * 2.0)));

    // Rim
    for i in 0..segments {
        let angle = i as f32 * segment_angle;
        let cx = angle.cos() * (funnel_r + 0.5);
        let cz = angle.sin() * (funnel_r + 0.5);
        spawn_arena_static(commands, meshes.add(Cuboid::new(segment_w, 3.0 * s, 0.5)), funnel_mat.clone(),
            format!("Rim {}", i),
            Transform::from_translation(Vec3::new(cx, funnel_height + 1.5 * s, cz))
                .with_rotation(Quat::from_rotation_y(-angle)),
            CollisionShapeData::cuboid(Vec3::new(segment_w * 0.5, 1.5 * s, 0.25)));
    }
}

fn spawn_colosseum(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let wall_mat = arena_mat(materials, Color::srgb(0.5, 0.45, 0.35));
    let segments = 24u32;
    let angle_step = std::f32::consts::TAU / segments as f32;
    let outer_r = 15.0 * s;
    let inner_r = 10.0 * s;
    let seg_w = outer_r * angle_step * 1.1;

    // Outer wall
    for i in 0..segments {
        let angle = i as f32 * angle_step;
        let x = angle.cos() * outer_r;
        let z = angle.sin() * outer_r;
        let wall_mesh = meshes.add(Cuboid::new(seg_w, 6.0 * s, 0.5 * s));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
            format!("Outer Wall {}", i),
            Transform::from_translation(Vec3::new(x, 3.0 * s, z))
                .with_rotation(Quat::from_rotation_y(-angle)),
            CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 3.0 * s, 0.25)));
    }
    // Inner wall (lower)
    for i in 0..segments {
        let angle = i as f32 * angle_step;
        let x = angle.cos() * inner_r;
        let z = angle.sin() * inner_r;
        let seg_w2 = inner_r * angle_step * 1.1;
        let wall_mesh = meshes.add(Cuboid::new(seg_w2, 3.0 * s, 0.4 * s));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
            format!("Inner Wall {}", i),
            Transform::from_translation(Vec3::new(x, 1.5 * s, z))
                .with_rotation(Quat::from_rotation_y(-angle)),
            CollisionShapeData::cuboid(Vec3::new(seg_w2 * 0.5, 1.5 * s, 0.2)));
    }
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(outer_r * 2.2, 0.5, outer_r * 2.2));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.4, 0.35, 0.3)),
        "Arena Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(outer_r * 1.1, 0.25, outer_r * 1.1)));
}

fn spawn_maze(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let wall_mat = arena_mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let wall_h = 2.0 * s;
    let cell = 4.0 * s;
    // Simple maze layout: (start_x, start_z, end_x, end_z) in grid coords
    let walls: &[(i32, i32, i32, i32)] = &[
        (0,0,5,0), (0,0,0,5), (5,0,5,5), (0,5,5,5), // outer
        (1,0,1,3), (2,1,2,4), (3,0,3,2), (3,3,3,5),
        (4,1,4,4), (1,2,3,2), (0,3,2,3), (3,4,5,4),
        (1,1,2,1), (2,3,3,3),
    ];
    for (i, &(x1, z1, x2, z2)) in walls.iter().enumerate() {
        let sx = x1 as f32 * cell - 2.5 * cell;
        let sz = z1 as f32 * cell - 2.5 * cell;
        let ex = x2 as f32 * cell - 2.5 * cell;
        let ez = z2 as f32 * cell - 2.5 * cell;
        let dx = ex - sx;
        let dz = ez - sz;
        let len = (dx * dx + dz * dz).sqrt();
        let cx = (sx + ex) * 0.5;
        let cz = (sz + ez) * 0.5;
        let angle = dz.atan2(dx);
        let wall_mesh = meshes.add(Cuboid::new(len, wall_h, 0.2 * s));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
            format!("Maze Wall {}", i),
            Transform::from_translation(Vec3::new(cx, wall_h * 0.5, cz))
                .with_rotation(Quat::from_rotation_y(-angle + std::f32::consts::FRAC_PI_2)),
            CollisionShapeData::cuboid(Vec3::new(len * 0.5, wall_h * 0.5, 0.1 * s)));
    }
    // Floor
    let size = 5.0 * cell;
    let floor_mesh = meshes.add(Cuboid::new(size * 2.0, 0.5, size * 2.0));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.35)),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(size, 0.25, size)));
}

fn spawn_halfpipe(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let pipe_mat = arena_mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let segments = 12;
    let pipe_r = 6.0 * s;
    let pipe_len = 20.0 * s;
    // U-shape from flat segments
    for i in 0..segments {
        let angle = (i as f32 / (segments - 1) as f32) * std::f32::consts::PI - std::f32::consts::FRAC_PI_2;
        let x = angle.cos() * pipe_r;
        let y = angle.sin() * pipe_r + pipe_r;
        let seg_w = pipe_r * std::f32::consts::PI / segments as f32 * 1.1;
        let seg_mesh = meshes.add(Cuboid::new(seg_w, 0.2 * s, pipe_len));
        spawn_arena_static(commands, seg_mesh, pipe_mat.clone(),
            format!("Halfpipe Seg {}", i),
            Transform::from_translation(Vec3::new(x, y, 0.0))
                .with_rotation(Quat::from_rotation_z(angle)),
            CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.1 * s, pipe_len * 0.5)));
    }
    // End caps
    for side in [-1.0f32, 1.0] {
        let cap_mesh = meshes.add(Cuboid::new(pipe_r * 2.2, pipe_r * 2.0, 0.3 * s));
        spawn_arena_static(commands, cap_mesh, pipe_mat.clone(),
            "End Cap".into(),
            Transform::from_translation(Vec3::new(0.0, pipe_r, side * pipe_len * 0.5)),
            CollisionShapeData::cuboid(Vec3::new(pipe_r * 1.1, pipe_r, 0.15 * s)));
    }
}

fn spawn_tiered_platforms(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let plat_mat = arena_mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let tiers = 6;
    let plat_w = 20.0 * s;
    for i in 0..tiers {
        let y = i as f32 * 2.5 * s;
        let w = plat_w - i as f32 * 2.0 * s;
        let offset = (i as f32 - tiers as f32 / 2.0) * 1.5 * s;
        let plat_mesh = meshes.add(Cuboid::new(w, 0.3 * s, 6.0 * s));
        spawn_arena_static(commands, plat_mesh, plat_mat.clone(),
            format!("Tier {}", i),
            Transform::from_translation(Vec3::new(offset, y, 0.0)),
            CollisionShapeData::cuboid(Vec3::new(w * 0.5, 0.15 * s, 3.0 * s)));
        // Ramp connecting to next tier
        if i < tiers - 1 {
            let ramp_mesh = meshes.add(Cuboid::new(2.0 * s, 0.15 * s, 3.0 * s));
            spawn_arena_static(commands, ramp_mesh, plat_mat.clone(),
                format!("Ramp {}", i),
                Transform::from_translation(Vec3::new(offset + 1.0 * s, y + 1.25 * s, 0.0))
                    .with_rotation(Quat::from_rotation_z(-0.45)),
                CollisionShapeData::cuboid(Vec3::new(1.0 * s, 0.075 * s, 1.5 * s)));
        }
    }
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(plat_w * 1.5, 0.5, plat_w));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.35)),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(plat_w * 0.75, 0.25, plat_w * 0.5)));
}

fn spawn_canyon(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let wall_mat = arena_mat(materials, Color::srgb(0.5, 0.35, 0.25));
    let canyon_len = 30.0 * s;
    let wall_h = 10.0 * s;
    let gap = 4.0 * s;
    // Two canyon walls with varying width
    let segments = 8;
    for i in 0..segments {
        let z = (i as f32 / segments as f32 - 0.5) * canyon_len;
        let wobble = ((i as f32 * 1.7).sin()) * 2.0 * s;
        for side in [-1.0f32, 1.0] {
            let x = side * (gap + wobble.abs());
            let wall_mesh = meshes.add(Cuboid::new(3.0 * s, wall_h, canyon_len / segments as f32 * 1.1));
            spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
                format!("Canyon Wall {},{}", i, if side > 0.0 { "R" } else { "L" }),
                Transform::from_translation(Vec3::new(x, wall_h * 0.5, z)),
                CollisionShapeData::cuboid(Vec3::new(1.5 * s, wall_h * 0.5, canyon_len / segments as f32 * 0.55)));
        }
    }
    // Rocky floor
    let floor_mesh = meshes.add(Cuboid::new(gap * 4.0, 0.5, canyon_len));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.4, 0.3, 0.2)),
        "Canyon Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(gap * 2.0, 0.25, canyon_len * 0.5)));
    // Boulders
    for i in 0..6u32 {
        let z = (i as f32 - 2.5) * 5.0 * s;
        let x = ((i as f32 * 2.7).sin()) * gap * 0.5;
        let r = (0.5 + (i as f32 * 0.3).sin().abs()) * s;
        let boulder_mesh = meshes.add(Sphere::new(r));
        spawn_arena_static(commands, boulder_mesh, wall_mat.clone(),
            format!("Boulder {}", i),
            Transform::from_translation(Vec3::new(x, r, z)),
            CollisionShapeData::sphere(r));
    }
}

fn spawn_fortress(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let stone_mat = arena_mat(materials, Color::srgb(0.45, 0.42, 0.38));
    let fort_size = 10.0 * s;
    let wall_h = 5.0 * s;
    // Four walls
    for (pos, half) in [
        (Vec3::new(0.0, wall_h * 0.5, fort_size), Vec3::new(fort_size, wall_h * 0.5, 0.3 * s)),
        (Vec3::new(0.0, wall_h * 0.5, -fort_size), Vec3::new(fort_size, wall_h * 0.5, 0.3 * s)),
        (Vec3::new(fort_size, wall_h * 0.5, 0.0), Vec3::new(0.3 * s, wall_h * 0.5, fort_size)),
        (Vec3::new(-fort_size, wall_h * 0.5, 0.0), Vec3::new(0.3 * s, wall_h * 0.5, fort_size)),
    ] {
        let wall_mesh = meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0));
        spawn_arena_static(commands, wall_mesh, stone_mat.clone(), "Fort Wall".into(),
            Transform::from_translation(pos), CollisionShapeData::cuboid(half));
    }
    // Corner towers
    let tower_mat = arena_mat(materials, Color::srgb(0.5, 0.45, 0.4));
    for (cx, cz) in [(-1.0f32, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
        let tower_mesh = meshes.add(Cylinder::new(1.5 * s, wall_h * 1.5));
        spawn_arena_static(commands, tower_mesh, tower_mat.clone(), "Tower".into(),
            Transform::from_translation(Vec3::new(cx * fort_size, wall_h * 0.75, cz * fort_size)),
            CollisionShapeData::cylinder(1.5 * s, wall_h * 0.75));
    }
    // Interior ramp
    let ramp_mesh = meshes.add(Cuboid::new(4.0 * s, 0.2 * s, 6.0 * s));
    spawn_arena_static(commands, ramp_mesh, stone_mat.clone(), "Interior Ramp".into(),
        Transform::from_translation(Vec3::new(-5.0 * s, 2.0 * s, 0.0))
            .with_rotation(Quat::from_rotation_z(-0.3)),
        CollisionShapeData::cuboid(Vec3::new(2.0 * s, 0.1 * s, 3.0 * s)));
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(fort_size * 2.5, 0.5, fort_size * 2.5));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.35, 0.32, 0.3)),
        "Courtyard".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(fort_size * 1.25, 0.25, fort_size * 1.25)));
}

fn spawn_spiral_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let ramp_mat = arena_mat(materials, Color::srgb(0.4, 0.4, 0.45));
    let total_height = 15.0 * s;
    let outer_r = 8.0 * s;
    let segments = 24;
    for i in 0..segments {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let y = total_height - i as f32 * (total_height / segments as f32);
        let r = outer_r - i as f32 * 0.1 * s;
        let seg_mesh = meshes.add(Cuboid::new(4.0 * s, 0.15 * s, 2.0 * s));
        spawn_arena_static(commands, seg_mesh, ramp_mat.clone(),
            format!("Spiral {}", i),
            Transform::from_translation(Vec3::new(angle.cos() * r, y, angle.sin() * r))
                .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_z(-0.08)),
            CollisionShapeData::cuboid(Vec3::new(2.0 * s, 0.075 * s, 1.0 * s)));
    }
    // Central column
    let col_mesh = meshes.add(Cylinder::new(1.0 * s, total_height));
    spawn_arena_static(commands, col_mesh, arena_mat(materials, Color::srgb(0.35, 0.35, 0.4)),
        "Column".into(), Transform::from_translation(Vec3::new(0.0, total_height * 0.5, 0.0)),
        CollisionShapeData::cylinder(1.0 * s, total_height * 0.5));
    // Outer rail
    for i in 0..segments {
        let angle = i as f32 * std::f32::consts::TAU / 6.0;
        let y = total_height - i as f32 * (total_height / segments as f32) + 0.5 * s;
        let r = outer_r + 1.5 * s;
        let rail_mesh = meshes.add(Cuboid::new(0.2 * s, 1.0 * s, 2.0 * s));
        spawn_arena_static(commands, rail_mesh, ramp_mat.clone(),
            format!("Rail {}", i),
            Transform::from_translation(Vec3::new(angle.cos() * r, y, angle.sin() * r))
                .with_rotation(Quat::from_rotation_y(-angle)),
            CollisionShapeData::cuboid(Vec3::new(0.1 * s, 0.5 * s, 1.0 * s)));
    }
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(outer_r * 3.0, 0.5, outer_r * 3.0));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.35)),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(outer_r * 1.5, 0.25, outer_r * 1.5)));
}

fn spawn_bowl_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let bowl_mat = arena_mat(materials, Color::srgb(0.4, 0.38, 0.35));
    let bowl_r = 15.0 * s;
    let segments = 20;
    let rings = 6;
    let angle_step = std::f32::consts::TAU / segments as f32;
    for ring in 0..rings {
        let ring_r = (ring as f32 + 1.0) / rings as f32 * bowl_r;
        let ring_h = (ring as f32 / rings as f32).powi(2) * bowl_r * 0.4;
        let seg_w = ring_r * angle_step * 1.1;
        let tilt = (ring as f32 / rings as f32) * 0.5;
        for seg in 0..segments {
            let angle = seg as f32 * angle_step;
            let x = angle.cos() * ring_r;
            let z = angle.sin() * ring_r;
            let seg_mesh = meshes.add(Cuboid::new(seg_w, 0.2 * s, ring_r / rings as f32 * 1.2));
            spawn_arena_static(commands, seg_mesh, bowl_mat.clone(),
                format!("Bowl {},{}", ring, seg),
                Transform::from_translation(Vec3::new(x, ring_h, z))
                    .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(tilt)),
                CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.1 * s, ring_r / rings as f32 * 0.6)));
        }
    }
    // Center flat
    let center_mesh = meshes.add(Cuboid::new(4.0 * s, 0.3 * s, 4.0 * s));
    spawn_arena_static(commands, center_mesh, arena_mat(materials, Color::srgb(0.35, 0.35, 0.4)),
        "Bowl Center".into(), Transform::from_translation(Vec3::new(0.0, -0.15 * s, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(2.0 * s, 0.15 * s, 2.0 * s)));
}

fn spawn_pillars_arena(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let pillar_mat = arena_mat(materials, Color::srgb(0.45, 0.42, 0.38));
    let arena = 18.0 * s;
    // Random-ish pillar field
    let pillar_positions: &[(f32, f32, f32)] = &[
        (0.3, 0.2, 2.0), (-0.5, 0.6, 1.5), (0.7, -0.3, 2.5), (-0.2, -0.7, 1.8),
        (0.1, 0.8, 2.2), (-0.8, -0.1, 1.6), (0.6, 0.5, 2.8), (-0.4, 0.4, 1.4),
        (0.8, -0.6, 2.0), (-0.6, -0.5, 2.3), (0.4, -0.8, 1.7), (-0.7, 0.7, 2.1),
        (0.0, -0.4, 1.9), (0.5, 0.0, 2.4), (-0.3, -0.2, 1.6), (0.2, 0.6, 2.0),
    ];
    for (i, &(px, pz, h_factor)) in pillar_positions.iter().enumerate() {
        let x = px * arena * 0.8;
        let z = pz * arena * 0.8;
        let h = h_factor * 2.0 * s;
        let r = (0.6 + (i as f32 * 0.15).sin().abs() * 0.6) * s;
        let pillar_mesh = meshes.add(Cylinder::new(r, h));
        spawn_arena_static(commands, pillar_mesh, pillar_mat.clone(),
            format!("Pillar {}", i),
            Transform::from_translation(Vec3::new(x, h * 0.5, z)),
            CollisionShapeData::cylinder(r, h * 0.5));
    }
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.35)),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(arena, 0.25, arena)));
    // Outer walls
    let wall_mat = arena_mat(materials, Color::srgb(0.35, 0.35, 0.4));
    let wh = 4.0 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena, wh * 0.5, 0.25)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.25, wh * 0.5, arena)),
    ] {
        let wall_mesh = meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(), "Wall".into(),
            Transform::from_translation(pos), CollisionShapeData::cuboid(half));
    }
}

fn spawn_ice_rink(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let arena = 18.0 * s;
    // Low-friction floor
    let floor_mesh = meshes.add(Cuboid::new(arena * 2.0, 0.5, arena * 2.0));
    let mut floor_shape = CollisionShapeData::cuboid(Vec3::new(arena, 0.25, arena));
    floor_shape.friction = 0.02;
    floor_shape.restitution = 0.3;
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.7, 0.85, 0.95)),
        "Ice Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)), floor_shape);
    // Bumper walls
    let wall_mat = arena_mat(materials, Color::srgb(0.8, 0.2, 0.2));
    let wh = 1.5 * s;
    for (pos, half) in [
        (Vec3::new(0.0, wh * 0.5, arena), Vec3::new(arena, wh * 0.5, 0.3)),
        (Vec3::new(0.0, wh * 0.5, -arena), Vec3::new(arena, wh * 0.5, 0.3)),
        (Vec3::new(arena, wh * 0.5, 0.0), Vec3::new(0.3, wh * 0.5, arena)),
        (Vec3::new(-arena, wh * 0.5, 0.0), Vec3::new(0.3, wh * 0.5, arena)),
    ] {
        let wall_mesh = meshes.add(Cuboid::new(half.x * 2.0, half.y * 2.0, half.z * 2.0));
        let mut wall_shape = CollisionShapeData::cuboid(half);
        wall_shape.restitution = 0.8;
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(), "Bumper".into(),
            Transform::from_translation(pos), wall_shape);
    }
    // Center circle marker (cosmetic obstacle)
    let circle_mesh = meshes.add(Cylinder::new(3.0 * s, 0.1 * s));
    spawn_arena_static(commands, circle_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.8)),
        "Center Circle".into(), Transform::from_translation(Vec3::new(0.0, 0.05 * s, 0.0)),
        CollisionShapeData::cylinder(3.0 * s, 0.05 * s));
}

fn spawn_volcano(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let lava_mat = arena_mat(materials, Color::srgb(0.5, 0.4, 0.3));
    let base_r = 15.0 * s;
    let summit_r = 3.0 * s;
    let height = 12.0 * s;
    let rings = 8;
    let segments = 16;
    let angle_step = std::f32::consts::TAU / segments as f32;
    for ring in 0..rings {
        let t = ring as f32 / rings as f32;
        let r = base_r - t * (base_r - summit_r);
        let y = t * height;
        let slope_angle = ((base_r - summit_r) / height).atan();
        let seg_w = r * angle_step * 1.1;
        let ring_depth = height / rings as f32 / slope_angle.cos();
        for seg in 0..segments {
            let angle = seg as f32 * angle_step;
            let x = angle.cos() * r;
            let z = angle.sin() * r;
            let seg_mesh = meshes.add(Cuboid::new(seg_w, 0.2 * s, ring_depth.min(4.0 * s)));
            spawn_arena_static(commands, seg_mesh, lava_mat.clone(),
                format!("Volcano {},{}", ring, seg),
                Transform::from_translation(Vec3::new(x, y, z))
                    .with_rotation(Quat::from_rotation_y(-angle) * Quat::from_rotation_x(slope_angle)),
                CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.1 * s, ring_depth.min(4.0 * s) * 0.5)));
        }
    }
    // Crater rim
    for seg in 0..segments {
        let angle = seg as f32 * angle_step;
        let x = angle.cos() * summit_r;
        let z = angle.sin() * summit_r;
        let seg_w = summit_r * angle_step * 1.2;
        let rim_mesh = meshes.add(Cuboid::new(seg_w, 1.5 * s, 0.4 * s));
        spawn_arena_static(commands, rim_mesh, arena_mat(materials, Color::srgb(0.4, 0.3, 0.25)),
            format!("Rim {}", seg),
            Transform::from_translation(Vec3::new(x, height + 0.75 * s, z))
                .with_rotation(Quat::from_rotation_y(-angle)),
            CollisionShapeData::cuboid(Vec3::new(seg_w * 0.5, 0.75 * s, 0.2 * s)));
    }
    // Floor
    let floor_mesh = meshes.add(Cuboid::new(base_r * 2.5, 0.5, base_r * 2.5));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.28, 0.25)),
        "Ground".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(base_r * 1.25, 0.25, base_r * 1.25)));
}

fn spawn_labyrinth(commands: &mut Commands, meshes: &mut Assets<Mesh>, materials: &mut Assets<StandardMaterial>, s: f32) {
    let wall_mat = arena_mat(materials, Color::srgb(0.38, 0.38, 0.42));
    let wall_h = 2.5 * s;
    let cell = 3.0 * s;
    let grid = 7;
    let offset = grid as f32 * cell * 0.5;
    // Complex maze with more paths
    // Horizontal walls: (row, col_start, col_end)
    let h_walls: &[(i32, i32, i32)] = &[
        (0,0,7), (7,0,7), // top and bottom
        (1,0,2), (1,3,5), (1,6,7),
        (2,1,3), (2,4,6),
        (3,0,1), (3,2,4), (3,5,7),
        (4,1,2), (4,3,5), (4,6,7),
        (5,0,3), (5,4,6),
        (6,1,4), (6,5,7),
    ];
    for (i, &(row, c1, c2)) in h_walls.iter().enumerate() {
        let z = row as f32 * cell - offset;
        let x1 = c1 as f32 * cell - offset;
        let x2 = c2 as f32 * cell - offset;
        let len = x2 - x1;
        let cx = (x1 + x2) * 0.5;
        let wall_mesh = meshes.add(Cuboid::new(len, wall_h, 0.2 * s));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
            format!("HWall {}", i),
            Transform::from_translation(Vec3::new(cx, wall_h * 0.5, z)),
            CollisionShapeData::cuboid(Vec3::new(len * 0.5, wall_h * 0.5, 0.1 * s)));
    }
    // Vertical walls: (col, row_start, row_end)
    let v_walls: &[(i32, i32, i32)] = &[
        (0,0,7), (7,0,7), // left and right
        (1,0,2), (1,4,6),
        (2,1,3), (2,5,7),
        (3,0,2), (3,3,5),
        (4,2,4), (4,5,7),
        (5,0,3), (5,4,6),
        (6,1,5),
    ];
    for (i, &(col, r1, r2)) in v_walls.iter().enumerate() {
        let x = col as f32 * cell - offset;
        let z1 = r1 as f32 * cell - offset;
        let z2 = r2 as f32 * cell - offset;
        let len = z2 - z1;
        let cz = (z1 + z2) * 0.5;
        let wall_mesh = meshes.add(Cuboid::new(0.2 * s, wall_h, len));
        spawn_arena_static(commands, wall_mesh, wall_mat.clone(),
            format!("VWall {}", i),
            Transform::from_translation(Vec3::new(x, wall_h * 0.5, cz)),
            CollisionShapeData::cuboid(Vec3::new(0.1 * s, wall_h * 0.5, len * 0.5)));
    }
    // Floor
    let size = grid as f32 * cell;
    let floor_mesh = meshes.add(Cuboid::new(size * 1.2, 0.5, size * 1.2));
    spawn_arena_static(commands, floor_mesh, arena_mat(materials, Color::srgb(0.3, 0.3, 0.35)),
        "Floor".into(), Transform::from_translation(Vec3::new(0.0, -0.25, 0.0)),
        CollisionShapeData::cuboid(Vec3::new(size * 0.6, 0.25, size * 0.6)));
}

// ============================================================================
// Plugin
// ============================================================================

pub struct PhysicsPanelPlugin;

impl Plugin for PhysicsPanelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<PhysicsDebugState>()
            .init_resource::<PlaygroundState>()
            .init_resource::<ForcesState>()
            .init_resource::<MetricsState>()
            .init_resource::<ScenariosState>()
            .init_resource::<ArenaPresetsState>();

        let bridge = PhysicsBridge::default();
        let arc = bridge.pending.clone();
        app.insert_resource(bridge);

        use renzora_editor::SplashState;
        app.add_systems(
            Update,
            (
                sync_physics_bridge,
                process_playground_commands,
                process_scenario_commands,
                process_arena_commands,
            )
                .run_if(in_state(SplashState::Editor)),
        );

        app.register_panel(PhysicsDebugPanel::new(arc.clone()));
        app.register_panel(PhysicsPlaygroundPanel::new(arc.clone()));
        app.register_panel(PhysicsPropertiesPanel);
        app.register_panel(PhysicsForcesPanel::new(arc.clone()));
        app.register_panel(PhysicsMetricsPanel::new(arc.clone()));
        app.register_panel(PhysicsScenariosPanel::new(arc.clone()));
        app.register_panel(ArenaPresetsPanel::new(arc));
    }
}
