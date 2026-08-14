//! Renzora NavMesh — navigation meshes and pathfinding built on `vleue_navigator`.
//!
//! Phase 1: a single `NavMeshVolume` component defines a ground-plane walkable
//! region. Entities with `Collider + NavMeshObstacle` carve holes in the mesh.
//! Set `debug_draw = true` on the volume to see red wireframe triangles in the
//! editor viewport.

use std::f32::consts::FRAC_PI_2;

use bevy::ecs::entity::EntityHashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use vleue_navigator::{
    prelude::{
        ManagedNavMesh, NavMeshAgentExclusion, NavMeshDebug, NavMeshSettings, NavMeshUpdateMode,
        Triangulation, VleueNavigatorPlugin,
    },
    NavMesh,
};

pub mod persistence;
#[cfg(feature = "scripting")]
pub mod script_extension;
#[cfg(feature = "scripting")]
pub use script_extension::NavScriptExtension;

#[cfg(feature = "terrain")]
use renzora_terrain::data::{TerrainChunkData, TerrainChunkOf, TerrainData};
// avian `Collider` drives the obstacle auto-updater. Gated with `physics` so the
// lean export drops avian for a no-physics game — navmeshes then build from the
// volume outline (+ terrain) only.
#[cfg(feature = "physics")]
use avian3d::prelude::Collider;
#[cfg(feature = "physics")]
use vleue_navigator::prelude::NavmeshUpdaterPlugin;

/// Optional editor override for agent-path gizmo visibility. The NavMesh editor
/// panel (in `renzora_navmesh_editor`) inits + drives this from its "Show Agent
/// Paths" toggle; it is absent in a shipped game, where [`draw_agent_paths`]
/// falls back to per-volume `debug_draw`.
#[derive(Resource, Clone, Copy)]
pub struct ShowAgentPathsOverride(pub bool);

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
    mut volumes: Query<(Entity, &NavMeshVolume, Option<&mut Transform>), Added<NavMeshVolume>>,
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
#[cfg(feature = "terrain")]
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
#[cfg(feature = "terrain")]
fn sync_terrain_obstacles(
    mut volumes: Query<
        (
            Entity,
            &NavMeshVolume,
            &GlobalTransform,
            &mut NavMeshSettings,
        ),
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
            let Ok(terrain) = terrain_data_q.get(chunk_of.0) else {
                continue;
            };
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
    let Some(managed) = navmesh_q.iter().next() else {
        return;
    };
    let Some(navmesh) = navmeshes.get(managed) else {
        return;
    };

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
                            dest.x,
                            dest.y,
                            dest.z,
                            wps.len()
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
        let threshold = if is_final {
            agent.stopping_distance.max(0.01)
        } else {
            0.15
        };

        if dist < threshold {
            path.waypoints.remove(0);
            if path.waypoints.is_empty() {
                let dest = agent.target.unwrap_or(wp);
                agent.target = None;
                info!(
                    "[nav] Agent {entity:?} arrived at ({:.1}, {:.1}, {:.1})",
                    dest.x, dest.y, dest.z
                );
                renzora::clog_success!(
                    "NavMesh",
                    "Agent {entity:?} arrived at ({:.1}, {:.1}, {:.1})",
                    dest.x,
                    dest.y,
                    dest.z
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
    show_override: Option<Res<ShowAgentPathsOverride>>,
    mut gizmos: Gizmos,
) {
    // The editor's nav panel sets `ShowAgentPathsOverride` from its "Show Agent
    // Paths" toggle (see renzora_navmesh_editor). In a shipped game it's absent,
    // so fall back to the per-volume `debug_draw` flag — games can still opt into
    // path gizmos for on-screen debugging.
    let show = show_override
        .map(|o| o.0)
        .unwrap_or_else(|| volumes.iter().any(|v| v.debug_draw));

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

fn update_nav_read_state(mut q: Query<(&NavAgent, &NavPath, &GlobalTransform, &mut NavReadState)>) {
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

fn handle_nav_script_actions(trigger: On<renzora::ScriptAction>, mut agents: Query<&mut NavAgent>) {
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
            .add_systems(
                Update,
                (
                    on_volume_added,
                    sync_volume_changes,
                    update_agent_paths,
                    advance_agents,
                    auto_init_nav_read_state,
                    update_nav_read_state,
                    draw_agent_paths,
                ),
            )
            .add_observer(handle_nav_script_actions);

        // Auto-rebuild navmeshes from avian `Collider` obstacles — only when the
        // `physics` subsystem is built (added after VleueNavigatorPlugin above).
        #[cfg(feature = "physics")]
        app.add_plugins(NavmeshUpdaterPlugin::<Collider, NavMeshObstacle>::default());

        // Terrain-aware obstacle injection — only when the `terrain` subsystem is
        // built (the tuple above is unordered, so a separate add doesn't change
        // scheduling). Stripped together with `renzora_terrain` in a lean export.
        #[cfg(feature = "terrain")]
        app.add_systems(Update, sync_terrain_obstacles);

        // Script functions owned by the navmesh crate.
        #[cfg(feature = "scripting")]
        {
            let mut extensions = app.world_mut().get_resource_or_insert_with(
                renzora_scripting::extension::ScriptExtensions::default,
            );
            extensions.register(NavScriptExtension);
        }
    }
}

// Runtime-scope: the core navmesh systems run in both the editor viewport and
// the shipped game. The editor-only panel/inspectors live in the separate
// `renzora_navmesh_editor` crate (Editor scope). Previously NavMeshPlugin was
// never registered at all (no `add!`), so navmesh was dormant — this activates
// it. See [`ShowAgentPathsOverride`] for the editor↔runtime gizmo seam.
renzora::add!(NavMeshPlugin, Runtime);

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    use renzora::{ScriptAction, ScriptActionValue};
    use renzora_test_harness::{minimal_app, pump, with_manual_time};

    // ── settings derived from the volume ─────────────────────────────────────

    #[test]
    fn settings_carry_the_volumes_tuning_across() {
        let volume = NavMeshVolume {
            agent_radius: 1.25,
            simplify: 0.02,
            merge_steps: 3,
            upward_shift: 0.75,
            ..default()
        };
        let settings = build_settings(&volume);
        assert_eq!(settings.agent_radius, 1.25);
        assert_eq!(settings.simplify, 0.02);
        assert_eq!(settings.merge_steps, 3);
        assert_eq!(settings.upward_shift, 0.75);
        // Without a timeout a degenerate outline can hang the build thread.
        assert!(settings.build_timeout.is_some());
    }

    /// The volume is authored as half-extents in XZ but meshed as a 2D outline.
    /// Getting the sign or the axis wrong yields a mesh the agents cannot stand
    /// on, which surfaces much later as "start is not on the navmesh".
    #[test]
    fn the_outline_spans_the_full_extent_on_x_and_z() {
        let volume = NavMeshVolume {
            half_extents: Vec3::new(10.0, 5.0, 4.0),
            ..default()
        };
        let settings = build_settings(&volume);
        let outline = settings.fixed.as_navmesh();
        // The triangulation is meshed into a single polyanya layer; its vertices
        // are the 2D (x, z) outline the agents will be confined to.
        let vertices = &outline.layers[0].vertices;
        let xs: Vec<f32> = vertices.iter().map(|v| v.coords.x).collect();
        let zs: Vec<f32> = vertices.iter().map(|v| v.coords.y).collect();
        let span = |v: &[f32]| {
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!((span(&xs) - 20.0).abs() < 1e-3, "x span should be 2*10");
        assert!((span(&zs) - 8.0).abs() < 1e-3, "z span should be 2*4 — the Y half-extent must not leak in");
    }

    // ── agent movement ───────────────────────────────────────────────────────

    fn agent_app() -> App {
        let mut app = minimal_app();
        with_manual_time(&mut app, 60.0);
        app.add_message::<NavAgentArrived>()
            .add_systems(Update, advance_agents);
        app
    }

    fn spawn_walker(app: &mut App, at: Vec3, path: Vec<Vec3>, target: Option<Vec3>) -> Entity {
        app.world_mut()
            .spawn((
                NavAgent {
                    speed: 6.0,
                    target,
                    ..default()
                },
                NavPath { waypoints: path },
                Transform::from_translation(at),
            ))
            .id()
    }

    fn pos(app: &App, e: Entity) -> Vec3 {
        app.world().get::<Transform>(e).unwrap().translation
    }

    #[test]
    fn an_agent_walks_toward_its_next_waypoint() {
        let mut app = agent_app();
        let e = spawn_walker(&mut app, Vec3::ZERO, vec![Vec3::new(10.0, 0.0, 0.0)], Some(Vec3::new(10.0, 0.0, 0.0)));
        // Two frames, not one: Bevy's first update has a zero delta, so a
        // dt-scaled mover legitimately stands still on frame one.
        pump(&mut app, 2);
        let p = pos(&app, e);
        assert!(p.x > 0.0 && p.x < 10.0, "expected partial progress, got {p:?}");
    }

    /// Agents walk on the XZ plane; their height is owned by whatever placed
    /// them (terrain sampling, a physics capsule). If the mover wrote Y from the
    /// waypoint, every agent would sink to the navmesh plane.
    #[test]
    fn walking_preserves_the_agents_height() {
        let mut app = agent_app();
        let e = spawn_walker(
            &mut app,
            Vec3::new(0.0, 3.5, 0.0),
            vec![Vec3::new(10.0, 0.0, 0.0)],
            Some(Vec3::new(10.0, 0.0, 0.0)),
        );
        pump(&mut app, 5);
        assert_eq!(pos(&app, e).y, 3.5);
    }

    #[test]
    fn intermediate_waypoints_are_popped_as_they_are_reached() {
        let mut app = agent_app();
        let e = spawn_walker(
            &mut app,
            Vec3::ZERO,
            vec![Vec3::new(0.1, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0)],
            Some(Vec3::new(10.0, 0.0, 0.0)),
        );
        pump(&mut app, 1);
        assert_eq!(
            app.world().get::<NavPath>(e).unwrap().waypoints.len(),
            1,
            "the waypoint within the 0.15 threshold should have been consumed"
        );
    }

    #[test]
    fn arriving_clears_the_target_and_announces_it() {
        let mut app = agent_app();
        let dest = Vec3::new(0.05, 0.0, 0.0);
        let e = spawn_walker(&mut app, Vec3::ZERO, vec![dest], Some(dest));

        pump(&mut app, 1);

        assert!(app.world().get::<NavPath>(e).unwrap().waypoints.is_empty());
        assert!(
            app.world().get::<NavAgent>(e).unwrap().target.is_none(),
            "target must be cleared so the agent does not immediately repath"
        );

        let messages = app
            .world()
            .resource::<bevy::ecs::message::Messages<NavAgentArrived>>();
        let mut cursor = messages.get_cursor();
        let fired: Vec<_> = cursor.read(messages).collect();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].entity, e);
    }

    #[test]
    fn an_agent_with_no_path_does_not_move() {
        let mut app = agent_app();
        let e = spawn_walker(&mut app, Vec3::new(1.0, 2.0, 3.0), vec![], None);
        pump(&mut app, 5);
        assert_eq!(pos(&app, e), Vec3::new(1.0, 2.0, 3.0));
    }

    /// A step longer than the remaining distance would overshoot and oscillate
    /// around the waypoint forever.
    #[test]
    fn a_fast_agent_does_not_overshoot_its_waypoint() {
        let mut app = minimal_app();
        with_manual_time(&mut app, 1.0); // a full second per frame
        app.add_message::<NavAgentArrived>()
            .add_systems(Update, advance_agents);
        let dest = Vec3::new(1.0, 0.0, 0.0);
        let e = app
            .world_mut()
            .spawn((
                NavAgent { speed: 1000.0, target: Some(dest), ..default() },
                NavPath { waypoints: vec![dest] },
                Transform::default(),
            ))
            .id();
        pump(&mut app, 1);
        assert!(pos(&app, e).x <= 1.0 + 1e-3, "agent overshot: {:?}", pos(&app, e));
    }

    // ── the script-readable mirror ───────────────────────────────────────────

    #[test]
    fn read_state_is_auto_inserted_for_agents() {
        let mut app = minimal_app();
        app.add_systems(Update, auto_init_nav_read_state);
        let e = app.world_mut().spawn((NavAgent::default(), NavPath::default())).id();
        pump(&mut app, 1);
        assert!(app.world().get::<NavReadState>(e).is_some());
    }

    #[test]
    fn read_state_mirrors_target_path_and_distance() {
        let mut app = minimal_app();
        app.add_systems(Update, update_nav_read_state);
        let e = app
            .world_mut()
            .spawn((
                NavAgent { target: Some(Vec3::new(3.0, 99.0, 4.0)), ..default() },
                NavPath { waypoints: vec![Vec3::new(3.0, 0.0, 4.0)] },
                Transform::default(),
                NavReadState::default(),
            ))
            .id();
        pump(&mut app, 1);

        let read = *app.world().get::<NavReadState>(e).unwrap();
        assert!(read.has_target);
        assert!(read.has_path);
        assert!(!read.is_at_destination);
        // Distance is measured on the XZ plane, so the target's Y is ignored:
        // 3-4-5 triangle.
        assert!((read.distance_to_destination - 5.0).abs() < 1e-3);
    }

    #[test]
    fn read_state_reports_arrival_once_the_target_clears() {
        let mut app = minimal_app();
        app.add_systems(Update, update_nav_read_state);
        let e = app
            .world_mut()
            .spawn((
                NavAgent { target: None, ..default() },
                NavPath::default(),
                Transform::from_xyz(7.0, 0.0, 7.0),
                NavReadState::default(),
            ))
            .id();
        pump(&mut app, 1);

        let read = *app.world().get::<NavReadState>(e).unwrap();
        assert!(read.is_at_destination);
        assert!(!read.has_target);
        assert_eq!(read.distance_to_destination, 0.0);
    }

    // ── the script action surface ────────────────────────────────────────────

    fn action_app() -> App {
        let mut app = minimal_app();
        app.add_observer(handle_nav_script_actions);
        app
    }

    fn fire(app: &mut App, entity: Entity, name: &str, args: HashMap<String, ScriptActionValue>) {
        app.world_mut().trigger(ScriptAction {
            name: name.to_string(),
            entity,
            target_entity: None,
            args,
        });
    }

    #[test]
    fn nav_set_destination_accepts_a_vec3_argument() {
        let mut app = action_app();
        let e = app.world_mut().spawn((NavAgent::default(), NavPath::default())).id();

        let mut args = HashMap::new();
        args.insert("target".to_string(), ScriptActionValue::Vec3([1.0, 2.0, 3.0]));
        fire(&mut app, e, "nav_set_destination", args);

        assert_eq!(
            app.world().get::<NavAgent>(e).unwrap().target,
            Some(Vec3::new(1.0, 2.0, 3.0))
        );
    }

    /// The x/y/z fallback is what a script written as
    /// `nav_set_destination(x = 1, y = 0, z = 2)` produces, and integers are what
    /// a Lua number literal arrives as. Both paths matter.
    #[test]
    fn nav_set_destination_falls_back_to_scalar_components() {
        let mut app = action_app();
        let e = app.world_mut().spawn((NavAgent::default(), NavPath::default())).id();

        let mut args = HashMap::new();
        args.insert("x".to_string(), ScriptActionValue::Float(1.5));
        args.insert("y".to_string(), ScriptActionValue::Int(2));
        // `z` deliberately omitted — a missing component reads as 0.0.
        fire(&mut app, e, "nav_set_destination", args);

        assert_eq!(
            app.world().get::<NavAgent>(e).unwrap().target,
            Some(Vec3::new(1.5, 2.0, 0.0))
        );
    }

    #[test]
    fn nav_clear_destination_clears_the_target() {
        let mut app = action_app();
        let e = app
            .world_mut()
            .spawn((
                NavAgent { target: Some(Vec3::ONE), ..default() },
                NavPath::default(),
            ))
            .id();
        fire(&mut app, e, "nav_clear_destination", HashMap::new());
        assert!(app.world().get::<NavAgent>(e).unwrap().target.is_none());
    }

    /// The observer sees every `ScriptAction` in the app, including ones aimed at
    /// other crates' extensions. Reacting to an unknown name would make two
    /// crates fight over the same call.
    #[test]
    fn an_unrelated_action_is_ignored() {
        let mut app = action_app();
        let e = app
            .world_mut()
            .spawn((
                NavAgent { target: Some(Vec3::ONE), ..default() },
                NavPath::default(),
            ))
            .id();
        fire(&mut app, e, "play_sound", HashMap::new());
        assert_eq!(app.world().get::<NavAgent>(e).unwrap().target, Some(Vec3::ONE));
    }

    #[test]
    fn an_action_aimed_at_a_non_agent_is_a_no_op() {
        let mut app = action_app();
        let e = app.world_mut().spawn_empty().id();
        let mut args = HashMap::new();
        args.insert("target".to_string(), ScriptActionValue::Vec3([1.0, 2.0, 3.0]));
        fire(&mut app, e, "nav_set_destination", args);
        assert!(app.world().get::<NavAgent>(e).is_none());
    }

    // ── defaults ─────────────────────────────────────────────────────────────

    #[test]
    fn volume_defaults_are_a_usable_starting_region() {
        let v = NavMeshVolume::default();
        assert!(v.half_extents.x > 0.0 && v.half_extents.z > 0.0);
        assert!(v.agent_radius > 0.0);
        assert!(v.terrain_sample_step >= 1, "a step of 0 would divide by zero");
        assert!(v.max_slope_degrees > 0.0 && v.max_slope_degrees < 90.0);
    }

    #[test]
    fn agent_defaults_move_and_stop() {
        let a = NavAgent::default();
        assert!(a.speed > 0.0);
        assert!(a.turn_speed > 0.0);
        assert!(a.stopping_distance > 0.0);
        assert!(a.target.is_none());
    }
}
