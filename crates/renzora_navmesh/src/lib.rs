//! Renzora NavMesh — navigation meshes and pathfinding built on `vleue_navigator`.
//!
//! Phase 1: a single `NavMeshVolume` component defines a ground-plane walkable
//! region. Entities with `Collider + NavMeshObstacle` carve holes in the mesh.
//! Set `debug_draw = true` on the volume to see red wireframe triangles in the
//! editor viewport.

use std::f32::consts::FRAC_PI_2;

use avian3d::prelude::Collider;
use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use vleue_navigator::{
    NavMesh,
    prelude::{
        ManagedNavMesh, NavMeshAgentExclusion, NavMeshDebug, NavMeshSettings, NavMeshUpdateMode,
        NavmeshUpdaterPlugin, Triangulation, VleueNavigatorPlugin,
    },
};

#[cfg(feature = "editor")]
use renzora_editor::{
    AppEditorExt, FieldDef, FieldType, FieldValue, InspectorEntry,
};

pub mod persistence;
pub mod script_extension;
pub use script_extension::NavScriptExtension;

use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};

#[cfg(feature = "editor")]
pub mod editor_panel;

/// Defines a navigable region of the world. The volume is an axis-aligned box
/// in local space (its center is the entity's `Transform` translation) that
/// gets meshed on the ground plane. Obstacles with `NavMeshObstacle` inside
/// the volume carve holes.
///
/// Spawning a `NavMeshVolume` rotates the owning entity to ground-plane
/// orientation. Only one volume per scene is supported in Phase 1.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NavMeshVolume {
    pub half_extents: Vec3,
    pub agent_radius: f32,
    pub upward_shift: f32,
    pub simplify: f32,
    pub merge_steps: u32,
    pub debug_draw: bool,
    /// When true, samples terrain heightmaps within the volume and
    /// generates simplified obstacles for steep slopes. Agents will
    /// walk around hills instead of through them.
    pub include_terrain: bool,
    /// Slopes steeper than this angle (degrees) become obstacles.
    pub max_slope_degrees: f32,
    /// Sample every Nth terrain vertex. Higher = faster but less precise.
    /// 1 = full resolution (slow), 4 = every 4th vertex (recommended),
    /// 8 = very coarse.
    pub terrain_sample_step: u32,
}

impl Default for NavMeshVolume {
    fn default() -> Self {
        Self {
            half_extents: Vec3::new(25.0, 5.0, 25.0),
            agent_radius: 0.5,
            upward_shift: 0.2,
            simplify: 0.005,
            merge_steps: 0,
            debug_draw: true,
            include_terrain: false,
            max_slope_degrees: 45.0,
            terrain_sample_step: 4,
        }
    }
}

/// Marker: entities with both [`Collider`] and [`NavMeshObstacle`] become
/// holes in the navmesh. Useful so the ground collider itself is *not*
/// treated as an obstacle — only explicit blockers are.
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NavMeshObstacle;

fn build_settings(volume: &NavMeshVolume) -> NavMeshSettings {
    let (hx, hz) = (volume.half_extents.x, volume.half_extents.z);
    NavMeshSettings {
        fixed: Triangulation::from_outer_edges(&[
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ]),
        agent_radius: volume.agent_radius,
        simplify: volume.simplify,
        merge_steps: volume.merge_steps as usize,
        upward_shift: volume.upward_shift,
        build_timeout: Some(5.0),
        ..default()
    }
}

fn debug_color() -> Color {
    Color::srgb(1.0, 0.25, 0.25)
}

fn on_volume_added(
    mut commands: Commands,
    mut volumes: Query<
        (Entity, &NavMeshVolume, Option<&mut Transform>),
        Added<NavMeshVolume>,
    >,
) {
    for (entity, volume, transform) in &mut volumes {
        if let Some(mut t) = transform {
            t.rotation = Quat::from_rotation_x(FRAC_PI_2);
        }
        let mut e = commands.entity(entity);
        e.insert((
            ManagedNavMesh::single(),
            build_settings(volume),
            NavMeshUpdateMode::Direct,
        ));
        if volume.debug_draw {
            e.insert(NavMeshDebug(debug_color()));
        }
    }
}

fn sync_volume_changes(
    mut commands: Commands,
    changed: Query<(Entity, &NavMeshVolume), Changed<NavMeshVolume>>,
    mut settings_q: Query<&mut NavMeshSettings>,
) {
    for (entity, volume) in &changed {
        if let Ok(mut settings) = settings_q.get_mut(entity) {
            let new_settings = build_settings(volume);
            settings.fixed = new_settings.fixed;
            settings.agent_radius = new_settings.agent_radius;
            settings.simplify = new_settings.simplify;
            settings.merge_steps = new_settings.merge_steps;
            settings.upward_shift = new_settings.upward_shift;
        }
        if volume.debug_draw {
            commands.entity(entity).insert(NavMeshDebug(debug_color()));
        } else {
            commands.entity(entity).remove::<NavMeshDebug>();
        }
    }
}

#[cfg(feature = "editor")]
fn inspector_entry() -> InspectorEntry {
    use egui_phosphor::regular;
    InspectorEntry {
        type_id: "nav_mesh_volume",
        display_name: "NavMesh Volume",
        icon: regular::POLYGON,
        category: "navigation",
        has_fn: |world, entity| world.get::<NavMeshVolume>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavMeshVolume::default());
        }),
        remove_fn: Some(|world, entity| {
            let mut e = world.entity_mut(entity);
            e.remove::<NavMeshVolume>();
            e.remove::<ManagedNavMesh>();
            e.remove::<NavMeshSettings>();
            e.remove::<NavMeshUpdateMode>();
            e.remove::<NavMeshDebug>();
        }),
        is_enabled_fn: Some(|world, entity| {
            world
                .get::<NavMeshVolume>(entity)
                .map(|v| v.debug_draw)
                .unwrap_or(false)
        }),
        set_enabled_fn: Some(|world, entity, val| {
            if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                v.debug_draw = val;
            }
        }),
        fields: vec![
            FieldDef {
                name: "Half Extents",
                field_type: FieldType::Vec3 { speed: 0.25 },
                get_fn: |world, entity| {
                    world.get::<NavMeshVolume>(entity).map(|v| {
                        FieldValue::Vec3([v.half_extents.x, v.half_extents.y, v.half_extents.z])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3([x, y, z]) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.half_extents = Vec3::new(x, y, z);
                        }
                    }
                },
            },
            FieldDef {
                name: "Agent Radius",
                field_type: FieldType::Float { speed: 0.05, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.agent_radius))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.agent_radius = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Upward Shift",
                field_type: FieldType::Float { speed: 0.01, min: 0.0, max: 5.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.upward_shift))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.upward_shift = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Simplify",
                field_type: FieldType::Float { speed: 0.001, min: 0.0, max: 1.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.simplify))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.simplify = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Merge Steps",
                field_type: FieldType::Float { speed: 1.0, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.merge_steps as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.merge_steps = x.round().clamp(0.0, 10.0) as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Include Terrain",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Bool(v.include_terrain))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.include_terrain = b;
                        }
                    }
                },
            },
            FieldDef {
                name: "Max Slope (degrees)",
                field_type: FieldType::Float { speed: 1.0, min: 5.0, max: 89.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.max_slope_degrees))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.max_slope_degrees = x;
                        }
                    }
                },
            },
            FieldDef {
                name: "Terrain Sample Step",
                field_type: FieldType::Float { speed: 1.0, min: 1.0, max: 32.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Float(v.terrain_sample_step as f32))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.terrain_sample_step = x.round().clamp(1.0, 32.0) as u32;
                        }
                    }
                },
            },
            FieldDef {
                name: "Debug Draw",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavMeshVolume>(entity)
                        .map(|v| FieldValue::Bool(v.debug_draw))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut v) = world.get_mut::<NavMeshVolume>(entity) {
                            v.debug_draw = b;
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

#[cfg(feature = "editor")]
fn obstacle_inspector_entry() -> InspectorEntry {
    use egui_phosphor::regular;
    InspectorEntry {
        type_id: "nav_mesh_obstacle",
        display_name: "NavMesh Obstacle",
        icon: regular::CUBE,
        category: "navigation",
        has_fn: |world, entity| world.get::<NavMeshObstacle>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavMeshObstacle);
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<NavMeshObstacle>();
        }),
        is_enabled_fn: Some(|world, entity| world.get::<NavMeshObstacle>(entity).is_some()),
        set_enabled_fn: Some(|world, entity, val| {
            if val {
                world.entity_mut(entity).insert(NavMeshObstacle);
            } else {
                world.entity_mut(entity).remove::<NavMeshObstacle>();
            }
        }),
        fields: vec![],
        custom_ui_fn: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Phase 5: Terrain auto-obstacle
// ─────────────────────────────────────────────────────────────────────────

/// When any `NavMeshVolume` has `include_terrain = true`, auto-insert
/// `NavMeshObstacle` on every terrain chunk entity. When all volumes have
/// it off, auto-remove. This lets the heightmap colliders carve into the
/// navmesh so agents respect hills and valleys.
/// Generate simple rectangular obstacles from terrain heightmap where
/// slopes exceed the threshold. Samples every `step`-th vertex to keep
/// polygon count low. Returns polygons in navmesh-local 2D space
/// (relative to the volume's XZ center).
fn terrain_slope_obstacles(
    volume: &NavMeshVolume,
    vol_pos: Vec3,
    terrain: &TerrainData,
    chunk: &TerrainChunkData,
) -> Vec<Vec<Vec2>> {
    let step = volume.terrain_sample_step.max(1);
    let res = terrain.chunk_resolution;
    let spacing = terrain.vertex_spacing();
    let height_range = terrain.height_range();
    let origin = terrain.chunk_world_origin(chunk.chunk_x, chunk.chunk_z);
    let slope_threshold = volume.max_slope_degrees.to_radians().tan();
    let cell_size = spacing * step as f32;

    let vol_min_x = vol_pos.x - volume.half_extents.x;
    let vol_max_x = vol_pos.x + volume.half_extents.x;
    let vol_min_z = vol_pos.z - volume.half_extents.z;
    let vol_max_z = vol_pos.z + volume.half_extents.z;

    let mut obstacles = Vec::new();
    let mut x = 0u32;
    while x + step < res {
        let mut z = 0u32;
        while z + step < res {
            let world_x = origin.x + x as f32 * spacing;
            let world_z = origin.z + z as f32 * spacing;

            // Skip cells outside the volume
            if world_x + cell_size < vol_min_x
                || world_x > vol_max_x
                || world_z + cell_size < vol_min_z
                || world_z > vol_max_z
            {
                z += step;
                continue;
            }

            let h00 = chunk.get_height(x, z, res) * height_range + terrain.min_height;
            let h10 = chunk.get_height((x + step).min(res - 1), z, res) * height_range
                + terrain.min_height;
            let h01 = chunk.get_height(x, (z + step).min(res - 1), res) * height_range
                + terrain.min_height;

            let dx_slope = ((h10 - h00) / cell_size).abs();
            let dz_slope = ((h01 - h00) / cell_size).abs();

            if dx_slope > slope_threshold || dz_slope > slope_threshold {
                let lx = world_x - vol_pos.x;
                let lz = world_z - vol_pos.z;
                obstacles.push(vec![
                    Vec2::new(lx, lz),
                    Vec2::new(lx + cell_size, lz),
                    Vec2::new(lx + cell_size, lz + cell_size),
                    Vec2::new(lx, lz + cell_size),
                ]);
            }
            z += step;
        }
        x += step;
    }
    obstacles
}

/// When `include_terrain` is on, sample terrain heightmaps and inject
/// slope obstacles into `NavMeshSettings.fixed`. Runs only when the
/// volume or terrain data changes.
fn sync_terrain_obstacles(
    mut volumes: Query<
        (Entity, &NavMeshVolume, &GlobalTransform, &mut NavMeshSettings),
        Changed<NavMeshVolume>,
    >,
    terrain_data_q: Query<&TerrainData>,
    chunks: Query<(&TerrainChunkOf, &TerrainChunkData)>,
) {
    for (_entity, volume, gt, mut settings) in &mut volumes {
        if !volume.include_terrain {
            continue;
        }

        let vol_pos = gt.translation();
        let (hx, hz) = (volume.half_extents.x, volume.half_extents.z);

        // Rebuild fixed triangulation from scratch (outer edges + terrain obstacles).
        let mut tri = Triangulation::from_outer_edges(&[
            Vec2::new(-hx, -hz),
            Vec2::new(hx, -hz),
            Vec2::new(hx, hz),
            Vec2::new(-hx, hz),
        ]);

        let mut obstacle_count = 0usize;
        for (chunk_of, chunk_data) in &chunks {
            let Ok(terrain) = terrain_data_q.get(chunk_of.0) else { continue };
            let obs = terrain_slope_obstacles(volume, vol_pos, terrain, chunk_data);
            obstacle_count += obs.len();
            tri.add_obstacles(obs);
        }

        if obstacle_count > 0 {
            renzora::clog_info!(
                "NavMesh",
                "Injected {obstacle_count} terrain slope obstacles (step={}, max_slope={}deg)",
                volume.terrain_sample_step,
                volume.max_slope_degrees
            );
        }

        settings.fixed = tri;
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Phase 2: NavAgent + pathfinding
// ─────────────────────────────────────────────────────────────────────────

/// A moving entity that walks along the navmesh. Set `target` to something
/// `Some(world_pos)` and the agent will compute a path and follow it. When
/// it arrives within `stopping_distance`, `target` is cleared and a
/// [`NavAgentArrived`] message fires.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
#[require(NavPath, NavMeshAgentExclusion)]
pub struct NavAgent {
    /// World-units per second.
    pub speed: f32,
    /// Radians per second (how fast the agent rotates to face the path).
    pub turn_speed: f32,
    /// The agent considers itself arrived when within this distance of the
    /// final waypoint.
    pub stopping_distance: f32,
    /// Current destination. Setting this (re-assign, not field tweak) triggers
    /// a repath on the next frame.
    pub target: Option<Vec3>,
}

impl Default for NavAgent {
    fn default() -> Self {
        Self {
            speed: 5.0,
            turn_speed: 8.0,
            stopping_distance: 0.2,
            target: None,
        }
    }
}

/// Internal: the current list of waypoints the agent is walking along. The
/// first element is the next target; popped on arrival.
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct NavPath {
    pub waypoints: Vec<Vec3>,
}

/// Message fired when a [`NavAgent`] reaches its destination.
#[derive(Message, Clone, Copy, Debug)]
pub struct NavAgentArrived {
    pub entity: Entity,
}

/// Find a path across a single navmesh asset. Returns waypoints in world
/// space including the destination as the last point, or `None` if no path
/// exists (e.g. target outside the mesh).
pub fn find_path(navmesh: &NavMesh, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
    navmesh.transformed_path(from, to).map(|p| p.path)
}

fn update_agent_paths(
    mut agents: Query<(Entity, &NavAgent, &GlobalTransform, &mut NavPath)>,
    navmesh_q: Query<&ManagedNavMesh>,
    navmeshes: Res<Assets<NavMesh>>,
    mut last_target: Local<EntityHashMap<Option<Vec3>>>,
) {
    let Some(managed) = navmesh_q.iter().next() else { return };
    let Some(navmesh) = navmeshes.get(managed) else { return };

    for (entity, agent, gt, mut path) in &mut agents {
        let prev = last_target.get(&entity).copied().flatten();
        if prev == agent.target {
            continue;
        }
        last_target.insert(entity, agent.target);

        match agent.target {
            Some(dest) => {
                let from = gt.translation();
                let start_ok = navmesh.transformed_is_in_mesh(from);
                let end_ok = navmesh.transformed_is_in_mesh(dest);
                if !start_ok {
                    let msg = format!(
                        "Agent start {from:?} is not on the navmesh — \
                         capsule may be inside/on top of an obstacle, or \
                         outside the volume bounds"
                    );
                    warn!("[nav] {msg}");
                    renzora::clog_warn!("NavMesh", "{msg}");
                    path.waypoints.clear();
                    continue;
                }
                if !end_ok {
                    let msg = format!(
                        "Target {dest:?} is not on the navmesh — likely \
                         inside a wall, or outside the volume bounds"
                    );
                    warn!("[nav] {msg}");
                    renzora::clog_warn!("NavMesh", "{msg}");
                    path.waypoints.clear();
                    continue;
                }
                match find_path(navmesh, from, dest) {
                    Some(wps) => {
                        renzora::clog_info!(
                            "NavMesh",
                            "Agent {entity:?} heading to ({:.1}, {:.1}, {:.1}) — {} waypoints",
                            dest.x, dest.y, dest.z, wps.len()
                        );
                        path.waypoints = wps;
                    }
                    None => {
                        let msg = format!(
                            "No path from {from:?} to {dest:?} — points \
                             are on the mesh but no connected route \
                             (corridor may be narrower than agent_radius)"
                        );
                        warn!("[nav] {msg}");
                        renzora::clog_warn!("NavMesh", "{msg}");
                        path.waypoints.clear();
                    }
                }
            }
            None => path.waypoints.clear(),
        }
    }
}

fn advance_agents(
    time: Res<Time>,
    mut agents: Query<(Entity, &mut NavAgent, &mut NavPath, &mut Transform)>,
    mut arrived: MessageWriter<NavAgentArrived>,
) {
    let dt = time.delta_secs();
    for (entity, mut agent, mut path, mut transform) in &mut agents {
        if path.waypoints.is_empty() {
            continue;
        }

        let keep_y = transform.translation.y;
        let pos = Vec3::new(transform.translation.x, 0.0, transform.translation.z);
        let wp = path.waypoints[0];
        let wp_flat = Vec3::new(wp.x, 0.0, wp.z);
        let delta = wp_flat - pos;
        let dist = delta.length();

        let is_final = path.waypoints.len() == 1;
        let threshold = if is_final { agent.stopping_distance.max(0.01) } else { 0.15 };

        if dist < threshold {
            path.waypoints.remove(0);
            if path.waypoints.is_empty() {
                let dest = agent.target.unwrap_or(wp);
                agent.target = None;
                info!("[nav] Agent {entity:?} arrived at ({:.1}, {:.1}, {:.1})", dest.x, dest.y, dest.z);
                renzora::clog_success!(
                    "NavMesh",
                    "Agent {entity:?} arrived at ({:.1}, {:.1}, {:.1})",
                    dest.x, dest.y, dest.z
                );
                arrived.write(NavAgentArrived { entity });
            }
            continue;
        }

        let dir = delta / dist;
        let step = (agent.speed * dt).min(dist);
        transform.translation.x += dir.x * step;
        transform.translation.z += dir.z * step;
        transform.translation.y = keep_y;

        if dir.length_squared() > 1e-4 {
            let target_rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
            let t = (agent.turn_speed * dt).clamp(0.0, 1.0);
            transform.rotation = transform.rotation.slerp(target_rot, t);
        }
    }
}

fn draw_agent_paths(
    agents: Query<(&NavPath, &GlobalTransform)>,
    volumes: Query<&NavMeshVolume>,
    #[cfg(feature = "editor")] panel: Option<Res<editor_panel::NavMeshPanelState>>,
    mut gizmos: Gizmos,
) {
    // In editor builds, the panel's "Show Agent Paths" toggle overrides
    // the per-volume debug_draw flag. In runtime-only builds, fall back
    // to the volume flag so shipped games can still opt into paths for
    // on-screen debugging.
    #[cfg(feature = "editor")]
    let show = panel
        .as_ref()
        .map(|p| p.show_agent_paths())
        .unwrap_or_else(|| volumes.iter().any(|v| v.debug_draw));
    #[cfg(not(feature = "editor"))]
    let show = volumes.iter().any(|v| v.debug_draw);

    if !show {
        return;
    }
    let color = Color::srgb(0.2, 1.0, 0.4);
    for (path, gt) in &agents {
        if path.waypoints.is_empty() {
            continue;
        }
        let mut prev = gt.translation();
        for wp in &path.waypoints {
            gizmos.line(prev, *wp, color);
            gizmos.sphere(*wp, 0.15, color);
            prev = *wp;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Phase 3: scripting — NavReadState mirror + ScriptAction observer
// ─────────────────────────────────────────────────────────────────────────

/// Per-entity nav state, refreshed each frame. Scripts and blueprints read
/// this via the reflect path dispatcher:
/// - `get("NavReadState.has_path")`
/// - `get("NavReadState.distance_to_destination")`
/// - `get("NavReadState.is_at_destination")`
#[derive(Component, Clone, Copy, Debug, Default, Reflect, Serialize, Deserialize)]
#[reflect(Component, Serialize, Deserialize)]
pub struct NavReadState {
    pub has_target: bool,
    pub has_path: bool,
    pub is_at_destination: bool,
    pub distance_to_destination: f32,
}

fn auto_init_nav_read_state(
    mut commands: Commands,
    q: Query<Entity, (With<NavAgent>, Without<NavReadState>)>,
) {
    for entity in &q {
        commands.entity(entity).try_insert(NavReadState::default());
    }
}

fn update_nav_read_state(
    mut q: Query<(&NavAgent, &NavPath, &GlobalTransform, &mut NavReadState)>,
) {
    for (agent, path, gt, mut read) in &mut q {
        read.has_target = agent.target.is_some();
        read.has_path = !path.waypoints.is_empty();
        read.is_at_destination = agent.target.is_none();
        read.distance_to_destination = match agent.target {
            Some(dest) => {
                let pos = Vec3::new(gt.translation().x, 0.0, gt.translation().z);
                let d = Vec3::new(dest.x, 0.0, dest.z);
                (d - pos).length()
            }
            None => 0.0,
        };
    }
}

fn handle_nav_script_actions(
    trigger: On<renzora::ScriptAction>,
    mut agents: Query<&mut NavAgent>,
) {
    use renzora::ScriptActionValue;
    let action = trigger.event();
    match action.name.as_str() {
        "nav_set_destination" => {
            let dest = match action.args.get("target") {
                Some(ScriptActionValue::Vec3(v)) => Vec3::from(*v),
                _ => {
                    let read = |k: &str| -> f32 {
                        match action.args.get(k) {
                            Some(ScriptActionValue::Float(f)) => *f,
                            Some(ScriptActionValue::Int(i)) => *i as f32,
                            _ => 0.0,
                        }
                    };
                    Vec3::new(read("x"), read("y"), read("z"))
                }
            };
            if let Ok(mut agent) = agents.get_mut(action.entity) {
                agent.target = Some(dest);
            }
        }
        "nav_clear_destination" => {
            if let Ok(mut agent) = agents.get_mut(action.entity) {
                agent.target = None;
            }
        }
        _ => {}
    }
}

#[cfg(feature = "editor")]
fn agent_inspector_entry() -> InspectorEntry {
    use egui_phosphor::regular;
    InspectorEntry {
        type_id: "nav_agent",
        display_name: "NavMesh Agent",
        icon: regular::PERSON_SIMPLE_WALK,
        category: "navigation",
        has_fn: |world, entity| world.get::<NavAgent>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(NavAgent::default());
        }),
        remove_fn: Some(|world, entity| {
            let mut e = world.entity_mut(entity);
            e.remove::<NavAgent>();
            e.remove::<NavPath>();
            e.remove::<NavMeshAgentExclusion>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        fields: vec![
            FieldDef {
                name: "Has Target",
                field_type: FieldType::Bool,
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Bool(a.target.is_some()))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Bool(b) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.target = if b { Some(a.target.unwrap_or(Vec3::ZERO)) } else { None };
                        }
                    }
                },
            },
            FieldDef {
                name: "Speed",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 100.0 },
                get_fn: |world, entity| {
                    world.get::<NavAgent>(entity).map(|a| FieldValue::Float(a.speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.speed = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Turn Speed",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 50.0 },
                get_fn: |world, entity| {
                    world.get::<NavAgent>(entity).map(|a| FieldValue::Float(a.turn_speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.turn_speed = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Stopping Distance",
                field_type: FieldType::Float { speed: 0.05, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world
                        .get::<NavAgent>(entity)
                        .map(|a| FieldValue::Float(a.stopping_distance))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(x) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.stopping_distance = x.max(0.0);
                        }
                    }
                },
            },
            FieldDef {
                name: "Target",
                field_type: FieldType::Vec3 { speed: 0.25 },
                get_fn: |world, entity| {
                    world.get::<NavAgent>(entity).map(|a| {
                        let t = a.target.unwrap_or(Vec3::ZERO);
                        FieldValue::Vec3([t.x, t.y, t.z])
                    })
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Vec3([x, y, z]) = val {
                        if let Some(mut a) = world.get_mut::<NavAgent>(entity) {
                            a.target = Some(Vec3::new(x, y, z));
                        }
                    }
                },
            },
        ],
        custom_ui_fn: None,
    }
}

#[derive(Default)]
pub struct NavMeshPlugin;

impl Plugin for NavMeshPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] NavMeshPlugin");
        renzora::clog_info!("NavMesh", "NavMeshPlugin loaded");
        app.register_type::<NavMeshVolume>()
            .register_type::<NavMeshObstacle>()
            .register_type::<NavAgent>()
            .register_type::<NavPath>()
            .register_type::<NavReadState>()
            .add_message::<NavAgentArrived>()
            .add_plugins(VleueNavigatorPlugin)
            .add_plugins(NavmeshUpdaterPlugin::<Collider, NavMeshObstacle>::default())
            .add_systems(
                Update,
                (
                    on_volume_added,
                    sync_volume_changes,
                    sync_terrain_obstacles,
                    update_agent_paths,
                    advance_agents,
                    auto_init_nav_read_state,
                    update_nav_read_state,
                    draw_agent_paths,
                ),
            )
            .add_observer(handle_nav_script_actions);

        {
            let mut extensions = app
                .world_mut()
                .get_resource_or_insert_with(
                    renzora_scripting::extension::ScriptExtensions::default,
                );
            extensions.register(NavScriptExtension);
        }

        #[cfg(feature = "editor")]
        {
            app.register_inspector(inspector_entry());
            app.register_inspector(obstacle_inspector_entry());
            app.register_inspector(agent_inspector_entry());

            {
                use renzora_editor::{EntityPreset, SpawnRegistry};
                let mut registry = app
                    .world_mut()
                    .get_resource_or_insert_with(SpawnRegistry::default);
                registry.register(EntityPreset {
                    id: "navmesh_volume",
                    display_name: "NavMesh Volume",
                    icon: egui_phosphor::regular::POLYGON,
                    category: "Navigation",
                    spawn_fn: |world: &mut World| {
                        world
                            .spawn((
                                Name::new("NavMesh Volume"),
                                NavMeshVolume::default(),
                                Transform::default(),
                            ))
                            .id()
                    },
                });
            }

            app.init_resource::<editor_panel::NavMeshPanelState>();
            app.init_resource::<editor_panel::NavMeshPanelMirror>();
            app.init_resource::<editor_panel::NavMeshBakeRequest>();
            app.register_panel(editor_panel::NavMeshPanel);
            app.add_systems(
                Update,
                (
                    editor_panel::refresh_panel_mirror,
                    editor_panel::drain_panel_actions,
                    editor_panel::apply_auto_rebuild_setting,
                ),
            );
            app.add_systems(Update, editor_panel::flush_bake_request);
        }
    }
}
