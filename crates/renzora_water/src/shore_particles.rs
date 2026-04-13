use bevy::prelude::*;
use avian3d::prelude::*;
use serde::{Deserialize, Serialize};

use crate::buoyancy::sample_water_height;
use crate::component::WaterSurface;
use renzora_hanabi::data::*;

/// Attach to a `WaterSurface` entity to enable shore splash particles
/// where waves hit terrain/objects.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct WaterShoreParticles {
    /// Radius around the camera to sample shore points (world units).
    pub sample_radius: f32,
    /// Grid resolution — number of sample points per axis within the radius.
    pub grid_resolution: u32,
    /// Minimum time between splash spawns at the same grid cell (seconds).
    pub cooldown: f32,
    /// How close terrain must be to wave height to trigger splash (world units).
    pub contact_threshold: f32,
    /// Particle spawn rate per splash burst.
    pub burst_count: u32,
    /// Splash particle lifetime range.
    pub lifetime_min: f32,
    pub lifetime_max: f32,
    /// Splash upward velocity.
    pub splash_speed: f32,
    /// Splash particle size.
    pub particle_size: f32,
}

impl Default for WaterShoreParticles {
    fn default() -> Self {
        Self {
            sample_radius: 40.0,
            grid_resolution: 12,
            cooldown: 0.4,
            contact_threshold: 0.5,
            burst_count: 8,
            lifetime_min: 0.3,
            lifetime_max: 0.8,
            splash_speed: 3.0,
            particle_size: 0.08,
        }
    }
}

/// Internal tracking for splash cooldowns at grid positions.
#[derive(Component, Default)]
pub struct ShoreParticleState {
    /// Cooldown timers keyed by grid cell (ix, iz).
    cooldowns: Vec<f32>,
    /// Previous wave heights for crest detection.
    prev_heights: Vec<f32>,
}

fn splash_effect_definition(config: &WaterShoreParticles) -> HanabiEffectDefinition {
    HanabiEffectDefinition {
        name: "Water Shore Splash".to_string(),
        capacity: 64,
        spawn_mode: SpawnMode::Burst,
        spawn_rate: 0.0,
        spawn_count: config.burst_count,
        spawn_starts_active: true,
        lifetime_min: config.lifetime_min,
        lifetime_max: config.lifetime_max,
        emit_shape: HanabiEmitShape::Circle {
            radius: 0.3,
            dimension: ShapeDimension::Surface,
        },
        velocity_mode: VelocityMode::Random,
        velocity_magnitude: config.splash_speed,
        velocity_spread: 0.8,
        velocity_direction: [0.0, 1.0, 0.0],
        acceleration: [0.0, -9.81, 0.0],
        linear_drag: 1.0,
        size_start: config.particle_size,
        size_end: 0.0,
        use_flat_color: true,
        flat_color: [0.85, 0.9, 0.95, 0.8],
        color_gradient: vec![
            GradientStop { position: 0.0, color: [0.9, 0.93, 0.96, 0.9] },
            GradientStop { position: 0.5, color: [0.85, 0.9, 0.95, 0.6] },
            GradientStop { position: 1.0, color: [0.8, 0.85, 0.9, 0.0] },
        ],
        alpha_mode: ParticleAlphaMode::Blend,
        simulation_space: SimulationSpace::World,
        ..Default::default()
    }
}

/// Setup the shore particle state when the component is added.
pub fn setup_shore_particles(
    mut commands: Commands,
    query: Query<(Entity, &WaterShoreParticles), Added<WaterShoreParticles>>,
) {
    for (entity, config) in query.iter() {
        let cells = (config.grid_resolution * config.grid_resolution) as usize;
        commands.entity(entity).insert(ShoreParticleState {
            cooldowns: vec![0.0; cells],
            prev_heights: vec![0.0; cells],
        });
    }
}

/// Core system: sample wave heights near the camera, detect shore contact,
/// spawn splash particle bursts.
pub fn update_shore_particles(
    mut commands: Commands,
    time: Res<Time>,
    camera_query: Query<&GlobalTransform, With<Camera3d>>,
    mut water_query: Query<(
        &WaterSurface,
        &GlobalTransform,
        &WaterShoreParticles,
        &mut ShoreParticleState,
    )>,
    spatial_query: SpatialQuery,
) {
    let dt = time.delta_secs();
    let t = time.elapsed_secs();

    let Some(camera_transform) = camera_query.iter().next() else { return };
    let camera_pos = camera_transform.translation();

    for (surface, water_transform, config, mut state) in water_query.iter_mut() {
        let water_y = water_transform.translation().y;
        let res = config.grid_resolution;
        let cell_size = (config.sample_radius * 2.0) / res as f32;

        // Grid centered on camera XZ
        let grid_origin_x = camera_pos.x - config.sample_radius;
        let grid_origin_z = camera_pos.z - config.sample_radius;

        // Ensure state arrays are correct size
        let total_cells = (res * res) as usize;
        if state.cooldowns.len() != total_cells {
            state.cooldowns.resize(total_cells, 0.0);
            state.prev_heights.resize(total_cells, 0.0);
        }

        // Tick cooldowns
        for cd in state.cooldowns.iter_mut() {
            *cd = (*cd - dt).max(0.0);
        }

        for iz in 0..res {
            for ix in 0..res {
                let idx = (iz * res + ix) as usize;

                // Skip if on cooldown
                if state.cooldowns[idx] > 0.0 {
                    continue;
                }

                let sample_x = grid_origin_x + (ix as f32 + 0.5) * cell_size;
                let sample_z = grid_origin_z + (iz as f32 + 0.5) * cell_size;
                let xz = Vec2::new(sample_x, sample_z);

                // Current wave height at this XZ
                let wave_h = sample_water_height(xz, surface, t);
                let water_surface_y = water_y + wave_h;

                let prev_h = state.prev_heights[idx];
                state.prev_heights[idx] = wave_h;

                // Raycast down from above the wave to find terrain
                let ray_origin = Vec3::new(sample_x, water_surface_y + 2.0, sample_z);
                let ray_dir = Dir3::NEG_Y;
                let max_dist = 4.0 + config.contact_threshold;

                let hit = spatial_query.cast_ray(
                    ray_origin,
                    ray_dir,
                    max_dist,
                    true,
                    &SpatialQueryFilter::default(),
                );

                if let Some(ray_hit) = hit {
                    let terrain_y = ray_origin.y - ray_hit.distance;
                    let gap = water_surface_y - terrain_y;

                    // Wave crest is at or above terrain, and wave is rising
                    if gap.abs() < config.contact_threshold && wave_h > prev_h {
                        // Spawn splash
                        let spawn_pos = Vec3::new(sample_x, terrain_y + 0.05, sample_z);
                        let max_life = config.lifetime_max;
                        commands.spawn((
                            Name::new("Shore Splash"),
                            Transform::from_translation(spawn_pos),
                            HanabiEffect {
                                source: EffectSource::Inline {
                                    definition: splash_effect_definition(config),
                                },
                                playing: true,
                                ..Default::default()
                            },
                            ShoreSplashMarker { despawn_at: t + max_life + 0.5 },
                        ));

                        state.cooldowns[idx] = config.cooldown;
                    }
                }
            }
        }
    }
}

/// Marker for shore splash entities with a despawn timer.
#[derive(Component)]
pub struct ShoreSplashMarker {
    pub despawn_at: f32,
}

/// Despawn splash particle entities after their lifetime expires.
pub fn cleanup_shore_splashes(
    mut commands: Commands,
    time: Res<Time>,
    query: Query<(Entity, &ShoreSplashMarker)>,
) {
    let t = time.elapsed_secs();
    for (entity, marker) in query.iter() {
        if t >= marker.despawn_at {
            commands.entity(entity).despawn();
        }
    }
}

// ── Inspector entry ─────────────────────────────────────────────────────────

#[cfg(feature = "editor")]
pub fn shore_particles_inspector_entry() -> renzora_editor_framework::InspectorEntry {
    use renzora_editor_framework::{InspectorEntry, FieldDef, FieldType, FieldValue};

    InspectorEntry {
        type_id: "water_shore_particles",
        display_name: "Shore Particles",
        icon: egui_phosphor::regular::WAVES,
        category: "rendering",
        has_fn: |world, entity| world.get::<WaterShoreParticles>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(WaterShoreParticles::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<WaterShoreParticles>();
            world.entity_mut(entity).remove::<ShoreParticleState>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        custom_ui_fn: None,
        fields: vec![
            FieldDef {
                name: "Sample Radius",
                field_type: FieldType::Float { speed: 1.0, min: 10.0, max: 100.0 },
                get_fn: |world, entity| {
                    world.get::<WaterShoreParticles>(entity).map(|s| FieldValue::Float(s.sample_radius))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterShoreParticles>(entity) { s.sample_radius = v; }
                    }
                },
            },
            FieldDef {
                name: "Cooldown",
                field_type: FieldType::Float { speed: 0.05, min: 0.1, max: 2.0 },
                get_fn: |world, entity| {
                    world.get::<WaterShoreParticles>(entity).map(|s| FieldValue::Float(s.cooldown))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterShoreParticles>(entity) { s.cooldown = v; }
                    }
                },
            },
            FieldDef {
                name: "Contact Threshold",
                field_type: FieldType::Float { speed: 0.05, min: 0.1, max: 3.0 },
                get_fn: |world, entity| {
                    world.get::<WaterShoreParticles>(entity).map(|s| FieldValue::Float(s.contact_threshold))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterShoreParticles>(entity) { s.contact_threshold = v; }
                    }
                },
            },
            FieldDef {
                name: "Splash Speed",
                field_type: FieldType::Float { speed: 0.1, min: 0.5, max: 10.0 },
                get_fn: |world, entity| {
                    world.get::<WaterShoreParticles>(entity).map(|s| FieldValue::Float(s.splash_speed))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterShoreParticles>(entity) { s.splash_speed = v; }
                    }
                },
            },
            FieldDef {
                name: "Particle Size",
                field_type: FieldType::Float { speed: 0.01, min: 0.01, max: 0.5 },
                get_fn: |world, entity| {
                    world.get::<WaterShoreParticles>(entity).map(|s| FieldValue::Float(s.particle_size))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<WaterShoreParticles>(entity) { s.particle_size = v; }
                    }
                },
            },
        ],
    }
}
