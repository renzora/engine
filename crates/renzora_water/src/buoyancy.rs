#![allow(unused_variables)]

use bevy::prelude::*;
use avian3d::prelude::*;
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

#[cfg(feature = "editor")]
pub fn buoyant_inspector_entry() -> renzora_editor::InspectorEntry {
    use renzora_editor::{InspectorEntry, FieldDef, FieldType, FieldValue};

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
                field_type: FieldType::Float { speed: 0.5, min: 0.0, max: 200.0 },
                get_fn: |world, entity| {
                    world.get::<Buoyant>(entity).map(|s| FieldValue::Float(s.force))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) { s.force = v; }
                    }
                },
            },
            FieldDef {
                name: "Damping",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world.get::<Buoyant>(entity).map(|s| FieldValue::Float(s.damping))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) { s.damping = v; }
                    }
                },
            },
            FieldDef {
                name: "Submerge Depth",
                field_type: FieldType::Float { speed: 0.05, min: 0.1, max: 5.0 },
                get_fn: |world, entity| {
                    world.get::<Buoyant>(entity).map(|s| FieldValue::Float(s.submerge_depth))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) { s.submerge_depth = v; }
                    }
                },
            },
            FieldDef {
                name: "Wave Push",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world.get::<Buoyant>(entity).map(|s| FieldValue::Float(s.wave_push))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) { s.wave_push = v; }
                    }
                },
            },
            FieldDef {
                name: "Drag",
                field_type: FieldType::Float { speed: 0.1, min: 0.0, max: 10.0 },
                get_fn: |world, entity| {
                    world.get::<Buoyant>(entity).map(|s| FieldValue::Float(s.drag))
                },
                set_fn: |world, entity, val| {
                    if let FieldValue::Float(v) = val {
                        if let Some(mut s) = world.get_mut::<Buoyant>(entity) { s.drag = v; }
                    }
                },
            },
        ],
    }
}
