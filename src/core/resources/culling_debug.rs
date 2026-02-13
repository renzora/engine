//! Culling debug state resource for frustum/distance culling inspection

use bevy::prelude::*;

use crate::core::{MainCamera, SceneNode};

/// Marker component inserted on entities hidden by distance culling.
/// Prevents the system from fighting with user-set visibility.
#[derive(Component)]
pub struct DistanceCulled;

/// Culling debug state for monitoring frustum and distance culling
#[derive(Resource)]
pub struct CullingDebugState {
    /// Master toggle for distance culling
    pub enabled: bool,
    /// Global default cull distance
    pub max_distance: f32,
    /// Fraction of max_distance where fade begins (unused visually for now, but tracked)
    pub fade_start_fraction: f32,

    // Stats (updated by system)
    pub total_entities: u32,
    pub frustum_visible: u32,
    pub frustum_culled: u32,
    pub distance_culled: u32,
    pub distance_faded: u32,

    /// Entity counts in distance ranges: 0-50, 50-100, 100-200, 200-500, 500+
    pub distance_buckets: [u32; 5],

    /// Update interval in seconds
    pub update_interval: f32,
    /// Time since last update
    pub time_since_update: f32,
}

impl Default for CullingDebugState {
    fn default() -> Self {
        Self {
            enabled: false,
            max_distance: 500.0,
            fade_start_fraction: 0.8,
            total_entities: 0,
            frustum_visible: 0,
            frustum_culled: 0,
            distance_culled: 0,
            distance_faded: 0,
            distance_buckets: [0; 5],
            update_interval: 0.2,
            time_since_update: 0.0,
        }
    }
}

/// System to update culling debug statistics
pub fn update_culling_debug_state(
    mut state: ResMut<CullingDebugState>,
    time: Res<Time>,
    mesh_entities: Query<(&GlobalTransform, &ViewVisibility), With<Mesh3d>>,
    camera_q: Query<&GlobalTransform, With<MainCamera>>,
) {
    state.time_since_update += time.delta_secs();

    if state.time_since_update < state.update_interval {
        return;
    }
    state.time_since_update = 0.0;

    let camera_pos = camera_q.iter().next().map(|t| t.translation()).unwrap_or(Vec3::ZERO);

    let mut total = 0u32;
    let mut frustum_visible = 0u32;
    let mut frustum_culled = 0u32;
    let mut distance_culled_count = 0u32;
    let mut distance_faded_count = 0u32;
    let mut buckets = [0u32; 5];

    let max_dist = state.max_distance;
    let fade_start = max_dist * state.fade_start_fraction;

    for (transform, view_vis) in mesh_entities.iter() {
        total += 1;

        let dist = transform.translation().distance(camera_pos);

        // Distance bucket
        let bucket_idx = if dist < 50.0 {
            0
        } else if dist < 100.0 {
            1
        } else if dist < 200.0 {
            2
        } else if dist < 500.0 {
            3
        } else {
            4
        };
        buckets[bucket_idx] += 1;

        if view_vis.get() {
            frustum_visible += 1;

            // Check distance culling stats (only when enabled)
            if state.enabled {
                if dist > max_dist {
                    distance_culled_count += 1;
                } else if dist > fade_start {
                    distance_faded_count += 1;
                }
            }
        } else {
            frustum_culled += 1;
        }
    }

    state.total_entities = total;
    state.frustum_visible = frustum_visible;
    state.frustum_culled = frustum_culled;
    state.distance_culled = distance_culled_count;
    state.distance_faded = distance_faded_count;
    state.distance_buckets = buckets;
}

/// System to hide entities beyond max_distance from camera.
/// Only affects SceneNode entities (not editor infrastructure).
pub fn distance_culling_system(
    state: Res<CullingDebugState>,
    camera_q: Query<&GlobalTransform, With<MainCamera>>,
    mut scene_entities: Query<(Entity, &GlobalTransform, &mut Visibility), With<SceneNode>>,
    mut commands: Commands,
    culled_entities: Query<Entity, With<DistanceCulled>>,
) {
    if !state.enabled {
        // Restore all previously culled entities
        for entity in culled_entities.iter() {
            if let Ok(mut cmds) = commands.get_entity(entity) {
                cmds.remove::<DistanceCulled>();
            }
            if let Ok((_, _, mut vis)) = scene_entities.get_mut(entity) {
                *vis = Visibility::Inherited;
            }
        }
        return;
    }

    let camera_pos = camera_q.iter().next().map(|t| t.translation()).unwrap_or(Vec3::ZERO);
    let max_dist = state.max_distance;

    for (entity, transform, mut vis) in scene_entities.iter_mut() {
        let dist = transform.translation().distance(camera_pos);

        if dist > max_dist {
            if *vis != Visibility::Hidden {
                *vis = Visibility::Hidden;
                commands.entity(entity).insert(DistanceCulled);
            }
        } else if culled_entities.contains(entity) {
            *vis = Visibility::Inherited;
            commands.entity(entity).remove::<DistanceCulled>();
        }
    }
}
