// The `Buoyant` marker is a plain component, so this module stays compiled even
// in a no-physics build. Only `apply_buoyancy` touches avian, and it is gated
// behind `physics` so a lean export drops `avian3d` entirely.
#[cfg(feature = "physics")]
use avian3d::prelude::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::heightfield::WaterHeightField;

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

// ── Surface queries ──────────────────────────────────────────────────────────
//
// These wrap [`WaterHeightField`], the CPU mirror of the GPU cascades. They
// keep the signatures the old Gerstner helpers had so callers outside this
// crate need not care that the waves are now spectral.

/// Water surface height at a world XZ position, relative to the water plane.
/// Returns 0 (flat water) until the first field has been built.
pub fn sample_water_height(xz: Vec2, field: &WaterHeightField) -> f32 {
    if !field.ready {
        return 0.0;
    }
    field.sample_height(xz)
}

/// Horizontal force direction waves exert on a floating body.
///
/// The old Gerstner path differentiated the horizontal displacement in time.
/// The spectral surface has no closed form for that, so this uses the surface
/// slope instead: water runs down a wave face, and a body on a face gets pushed
/// the same way. Same qualitative behaviour — bodies get carried along the
/// waves rather than sitting in one spot — without keeping a second copy of the
/// field around to difference against.
pub fn sample_wave_velocity(xz: Vec2, field: &WaterHeightField) -> Vec2 {
    if !field.ready {
        return Vec2::ZERO;
    }
    // 1 m is comfortably above the CPU field's cell size for any sane tile
    // length, so this measures the swell's slope rather than sampling noise.
    const EPS: f32 = 1.0;
    let dx = field.sample_height(xz + Vec2::new(EPS, 0.0))
        - field.sample_height(xz - Vec2::new(EPS, 0.0));
    let dz = field.sample_height(xz + Vec2::new(0.0, EPS))
        - field.sample_height(xz - Vec2::new(0.0, EPS));
    -Vec2::new(dx, dz) / (2.0 * EPS)
}

// ── Buoyancy system ──────────────────────────────────────────────────────────

#[cfg(feature = "physics")]
pub fn apply_buoyancy(
    field: Res<WaterHeightField>,
    water_query: Query<&GlobalTransform, With<crate::component::WaterSurface>>,
    mut buoyant_query: Query<(&Buoyant, &GlobalTransform, Forces)>,
) {
    let Some(water_transform) = water_query.iter().next() else {
        return;
    };
    let water_y = water_transform.translation().y;

    for (buoyant, transform, mut forces) in buoyant_query.iter_mut() {
        let pos = transform.translation();
        let xz = Vec2::new(pos.x, pos.z);

        let wave_height = sample_water_height(xz, &field);
        let surface_y = water_y + wave_height;
        let depth = surface_y - pos.y;

        if depth > 0.0 {
            let submerge_factor = (depth / buoyant.submerge_depth.max(1e-3)).min(1.0);

            // Vertical buoyancy
            let buoyancy_force = buoyant.force * submerge_factor;
            forces.apply_force(Vec3::new(0.0, buoyancy_force, 0.0));

            // Horizontal wave push — waves carry floating objects along
            let wave_vel = sample_wave_velocity(xz, &field);
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

// The `Buoyant` inspector entry is editor-only and lives in the
// `renzora_water_editor` crate (crates/renzora_water/editor).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::WaterSurface;

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

    #[test]
    fn queries_are_flat_before_the_field_exists() {
        // The first frames run before any simulation has been built. Sampling
        // then must read as "flat water", not as garbage that launches every
        // floating body.
        let field = WaterHeightField::default();
        assert_eq!(sample_water_height(Vec2::new(5.0, 7.0), &field), 0.0);
        assert_eq!(sample_wave_velocity(Vec2::new(5.0, 7.0), &field), Vec2::ZERO);
    }

    #[test]
    fn wave_push_points_downhill() {
        // The push is minus the slope measured over a 1 m stencil, while the
        // field also carries structure finer than that stencil — so the check
        // is statistical: stepping along the push must descend for the large
        // majority of samples, not for every one.
        let surface = WaterSurface::default();
        let mut field = WaterHeightField::default();
        field.update(&surface, &[120.0, 123.1, 126.3]);

        let (mut descended, mut checked) = (0, 0);
        for i in 0..128 {
            let xz = Vec2::new(i as f32 * 5.3 - 340.0, i as f32 * -3.1 + 190.0);
            let push = sample_wave_velocity(xz, &field);
            if push.length() < 1e-3 {
                continue;
            }
            let here = field.sample_height(xz);
            let ahead = field.sample_height(xz + push.normalize());
            checked += 1;
            if ahead <= here {
                descended += 1;
            }
        }
        assert!(checked > 32, "too few sloped samples ({checked})");
        assert!(
            descended * 4 >= checked * 3,
            "push climbed the wave in {}/{checked} samples",
            checked - descended
        );
    }
}
