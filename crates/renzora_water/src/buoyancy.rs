#![allow(unused_variables)]

use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::component::{GerstnerWave, WaterSurface};

/// Attach to any entity with a RigidBody to make it float on water.
#[derive(Component, Clone, Debug, Reflect, Serialize, Deserialize)]
#[reflect(Component, Default)]
pub struct Buoyant {
    /// Buoyancy force multiplier. Higher = floats higher above surface.
    pub force: f32,
    /// Damping applied to velocity when in water. Reduces bobbing and sliding.
    pub damping: f32,
    /// How deep (below surface) the object must be to receive full buoyancy force.
    pub submerge_depth: f32,
    /// How strongly waves push the object horizontally.
    pub wave_push: f32,
    /// Water drag coefficient. Slows objects moving through water.
    pub drag: f32,
}

impl Default for Buoyant {
    fn default() -> Self {
        Self {
            force: 20.0,
            damping: 2.0,
            submerge_depth: 1.0,
            wave_push: 1.0,
            drag: 1.5,
        }
    }
}

// ── CPU-side Gerstner wave evaluation ────────────────────────────────────────

const PI: f32 = std::f32::consts::PI;
const GRAVITY_CONST: f32 = 9.81;

/// Evaluate a single Gerstner wave height at a given XZ position.
fn gerstner_wave_height(pos: Vec2, wave: &GerstnerWave, time: f32) -> f32 {
    let dir = wave.direction.normalize_or_zero();
    let wavelength = wave.wavelength;
    let amplitude = wave.amplitude;

    if wavelength < 0.01 || amplitude < 0.001 {
        return 0.0;
    }

    let w = 2.0 * PI / wavelength;
    let phi = (GRAVITY_CONST / w).sqrt() * w;
    let d = dir.dot(pos) * w + time * phi;

    amplitude * d.sin()
}

/// Evaluate the horizontal velocity of a single Gerstner wave at a position.
/// This is the time derivative of the Gerstner horizontal displacement.
fn gerstner_wave_velocity(pos: Vec2, wave: &GerstnerWave, time: f32) -> Vec2 {
    let dir = wave.direction.normalize_or_zero();
    let wavelength = wave.wavelength;
    let amplitude = wave.amplitude;
    let steepness = wave.steepness;

    if wavelength < 0.01 || amplitude < 0.001 {
        return Vec2::ZERO;
    }

    let w = 2.0 * PI / wavelength;
    let phi = (GRAVITY_CONST / w).sqrt() * w;
    let d = dir.dot(pos) * w + time * phi;
    let wave_count = 4.0; // approximate
    let q = steepness / (w * amplitude * wave_count + 0.001);

    // d/dt of Q*A*dir*cos(d) = -Q*A*dir*phi*sin(d)
    let speed = -q * amplitude * phi * d.sin();
    dir * speed
}

/// Sample the total water surface height at a world XZ position.
pub fn sample_water_height(xz: Vec2, surface: &WaterSurface, time: f32) -> f32 {
    let mut height = 0.0;
    for wave in &surface.waves {
        height += gerstner_wave_height(xz, wave, time);
    }
    height
}

/// Sample the total horizontal wave velocity at a world XZ position.
pub fn sample_wave_velocity(xz: Vec2, surface: &WaterSurface, time: f32) -> Vec2 {
    let mut vel = Vec2::ZERO;
    for wave in &surface.waves {
        vel += gerstner_wave_velocity(xz, wave, time);
    }
    vel
}

// ── Buoyancy system ──────────────────────────────────────────────────────────

pub fn apply_buoyancy(
    time: Res<Time>,
    water_query: Query<(&WaterSurface, &GlobalTransform)>,
    mut buoyant_query: Query<(&Buoyant, &GlobalTransform, Forces)>,
) {
    let t = time.elapsed_secs();
    let dt = time.delta_secs();

    let Some((surface, water_transform)) = water_query.iter().next() else {
        return;
    };
    let water_y = water_transform.translation().y;

    for (buoyant, transform, mut forces) in buoyant_query.iter_mut() {
        let pos = transform.translation();
        let xz = Vec2::new(pos.x, pos.z);

        let wave_height = sample_water_height(xz, surface, t);
        let surface_y = water_y + wave_height;
        let depth = surface_y - pos.y;

        if depth > 0.0 {
            let submerge_factor = (depth / buoyant.submerge_depth).min(1.0);

            // Vertical buoyancy
            let buoyancy_force = buoyant.force * submerge_factor;
            forces.apply_force(Vec3::new(0.0, buoyancy_force, 0.0));

            // Horizontal wave push — waves carry floating objects along
            let wave_vel = sample_wave_velocity(xz, surface, t);
            let push = Vec3::new(wave_vel.x, 0.0, wave_vel.y) * buoyant.wave_push * submerge_factor;
            forces.apply_force(push);

            // Water drag — opposes velocity, stronger when deeper
            let vel = forces.linear_velocity();
            let drag_force = -vel * buoyant.drag * submerge_factor;
            forces.apply_force(drag_force);

            // Extra vertical damping to settle bobbing
            if vel.y.abs() > 0.01 {
                let vert_damp = -vel.y * buoyant.damping * submerge_factor;
                forces.apply_force(Vec3::new(0.0, vert_damp, 0.0));
            }
        }
    }
}

// ── Inspector entry ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::{GerstnerWave, WaterSurface};

    /// Build a minimal WaterSurface with a hand-rolled wave list. We want
    /// deterministic inputs, so we override the heavy default surface.
    fn surface_with(waves: Vec<GerstnerWave>) -> WaterSurface {
        WaterSurface {
            waves,
            ..WaterSurface::default()
        }
    }

    #[test]
    fn sample_height_empty_wave_list_is_zero() {
        let surface = surface_with(Vec::new());
        let h = sample_water_height(Vec2::new(5.0, 7.0), &surface, 1.23);
        assert_eq!(h, 0.0);
    }

    #[test]
    fn sample_height_zero_amplitude_wave_is_zero() {
        // Amplitude under the 0.001 cutoff in gerstner_wave_height bypasses
        // the trig entirely. Same behaviour we ship in production so the
        // CPU path doesn't burn cycles on dead waves.
        let wave = GerstnerWave {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 10.0,
            amplitude: 0.0,
        };
        let surface = surface_with(vec![wave]);
        assert_eq!(sample_water_height(Vec2::ZERO, &surface, 0.0), 0.0);
    }

    #[test]
    fn sample_height_zero_wavelength_wave_is_zero() {
        // Same cutoff guard for degenerate wavelength.
        let wave = GerstnerWave {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 0.0,
            amplitude: 1.0,
        };
        let surface = surface_with(vec![wave]);
        assert_eq!(sample_water_height(Vec2::ZERO, &surface, 0.0), 0.0);
    }

    #[test]
    fn sample_height_bounded_by_amplitude() {
        // A single sine wave never exceeds its amplitude. This catches
        // formula mistakes that would inflate the displacement.
        let amp = 0.7;
        let wave = GerstnerWave {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 12.0,
            amplitude: amp,
        };
        let surface = surface_with(vec![wave]);
        // Sweep position + time so we hit a peak.
        for i in 0..200 {
            let t = i as f32 * 0.05;
            for x in [-3.0, -1.5, 0.0, 1.5, 3.0] {
                let h = sample_water_height(Vec2::new(x, 0.0), &surface, t);
                assert!(
                    h.abs() <= amp + 1e-4,
                    "height {} exceeded amplitude {} at x={} t={}",
                    h,
                    amp,
                    x,
                    t,
                );
            }
        }
    }

    #[test]
    fn sample_height_periodic_in_position() {
        // Stepping along the wave direction by exactly one wavelength
        // returns to the same height — a basic invariant of the Gerstner
        // formulation.
        let wavelength = 10.0;
        let dir = Vec2::new(1.0, 0.0);
        let wave = GerstnerWave {
            direction: dir,
            steepness: 0.5,
            wavelength,
            amplitude: 0.5,
        };
        let surface = surface_with(vec![wave]);
        let pos = Vec2::new(2.5, 0.0);
        let pos_shifted = pos + dir * wavelength;
        let t = 0.42;
        let h0 = sample_water_height(pos, &surface, t);
        let h1 = sample_water_height(pos_shifted, &surface, t);
        assert!(
            (h0 - h1).abs() < 1e-4,
            "expected periodic, got {} vs {}",
            h0,
            h1,
        );
    }

    #[test]
    fn sample_height_sums_multiple_waves() {
        // Sample at the origin where both waves contribute their peak
        // through the time term — the total has to equal the sum of
        // individual contributions, never less.
        let waves = vec![
            GerstnerWave {
                direction: Vec2::X,
                steepness: 0.5,
                wavelength: 10.0,
                amplitude: 0.4,
            },
            GerstnerWave {
                direction: Vec2::Y,
                steepness: 0.5,
                wavelength: 14.0,
                amplitude: 0.3,
            },
        ];
        let surface = surface_with(waves.clone());
        let pos = Vec2::new(0.0, 0.0);
        let t = 0.7;
        let total = sample_water_height(pos, &surface, t);
        // Reproduce by querying each wave alone.
        let alone_a = sample_water_height(pos, &surface_with(vec![waves[0].clone()]), t);
        let alone_b = sample_water_height(pos, &surface_with(vec![waves[1].clone()]), t);
        assert!(
            (total - (alone_a + alone_b)).abs() < 1e-5,
            "{} vs {} + {}",
            total,
            alone_a,
            alone_b,
        );
    }

    #[test]
    fn sample_velocity_zero_amplitude_is_zero() {
        let wave = GerstnerWave {
            direction: Vec2::new(1.0, 0.0),
            steepness: 0.5,
            wavelength: 10.0,
            amplitude: 0.0,
        };
        let surface = surface_with(vec![wave]);
        let v = sample_wave_velocity(Vec2::ZERO, &surface, 0.0);
        assert_eq!(v, Vec2::ZERO);
    }

    #[test]
    fn sample_velocity_aligns_with_wave_direction() {
        // Single 1D wave — the resulting horizontal velocity must lie
        // along the wave's direction (or be zero at a node), never
        // perpendicular to it.
        let dir = Vec2::new(1.0, 0.0);
        let wave = GerstnerWave {
            direction: dir,
            steepness: 0.5,
            wavelength: 8.0,
            amplitude: 0.4,
        };
        let surface = surface_with(vec![wave]);
        for t in [0.1, 0.4, 0.9, 1.7] {
            let v = sample_wave_velocity(Vec2::new(0.5, 0.0), &surface, t);
            // Component perpendicular to dir should be ~0.
            let perp = Vec2::new(-dir.y, dir.x);
            assert!(
                v.dot(perp).abs() < 1e-5,
                "velocity {:?} not aligned with direction {:?} at t={}",
                v,
                dir,
                t,
            );
        }
    }

    #[test]
    fn buoyant_default_force_is_positive() {
        // Sanity: a Buoyant with default values must push UP, otherwise
        // the inspector spawns a sinker-by-default.
        let b = Buoyant::default();
        assert!(b.force > 0.0);
        assert!(b.submerge_depth > 0.0);
        assert!(b.damping >= 0.0);
        assert!(b.drag >= 0.0);
    }
}

#[cfg(feature = "editor")]
pub fn buoyant_inspector_entry() -> renzora_editor::InspectorEntry {
    use renzora_editor::{FieldDef, FieldType, FieldValue, InspectorEntry};

    InspectorEntry {
        type_id: "buoyant",
        display_name: "Buoyant",
        icon: egui_phosphor::regular::LIFEBUOY,
        category: "physics",
        has_fn: |world, entity| world.get::<Buoyant>(entity).is_some(),
        add_fn: Some(|world, entity| {
            world.entity_mut(entity).insert(Buoyant::default());
        }),
        remove_fn: Some(|world, entity| {
            world.entity_mut(entity).remove::<Buoyant>();
        }),
        is_enabled_fn: None,
        set_enabled_fn: None,
        custom_ui_fn: None,
        fields: vec![
            FieldDef {
                name: "Force",
                field_type: FieldType::Float {
                    speed: 0.5,
                    min: 0.0,
                    max: 200.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.force))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.force = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Damping",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.damping))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.damping = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Submerge Depth",
                field_type: FieldType::Float {
                    speed: 0.05,
                    min: 0.1,
                    max: 5.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.submerge_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.submerge_depth = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Wave Push",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.wave_push))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.wave_push = v;
                        }
                    }
                },
            },
            FieldDef {
                name: "Drag",
                field_type: FieldType::Float {
                    speed: 0.1,
                    min: 0.0,
                    max: 10.0,
                },
                get_fn: |world, entity| {
                    world
                        .get::<Buoyant>(entity)
                        .map(|s| FieldValue::Float(s.drag))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) {
                            s.drag = v;
                        }
                    }
                },
            },
        ],
    }
}
