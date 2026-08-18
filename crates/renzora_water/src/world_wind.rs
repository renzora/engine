//! Drive the sea state from the world wind.
//!
//! The naive version of this — copy `WindState::speed` into every cascade's
//! `wind_speed` — is wrong twice over. It flattens a hand-tuned ocean (a long
//! swell plus a short wind sea, at different bearings, is the whole reason
//! there is a cascade *list*) into one number, and it rebuilds the JONSWAP
//! spectrum textures on every frame the wind moves at all.
//!
//! So this scales instead of overwriting. The authored cascade values are
//! captured as a [`WaterWindBaseline`] and treated as the sea's *shape* at
//! reference wind; the world wind then scales every cascade's speed by one
//! ratio and rotates every bearing by one offset. Turn the wind up and the same
//! ocean gets harsher without becoming a different ocean.
//!
//! # Why the ratio is quantized
//!
//! [`CascadeSignature`](crate::systems) drives the spectrum rebuild, and
//! `wind_speed` is part of it — every distinct value costs a full cascade
//! re-bake. `WindState::sea_state_speed` is smoothed but still continuous, so
//! writing it raw would rebuild every frame during a change. Quantizing the
//! ratio to [`RATIO_STEP`] bounds that to one rebuild per step crossed, and
//! leaves a settled sea at exactly zero rebuilds.

use bevy::prelude::*;
use renzora::{WindState, REFERENCE_WIND_SPEED};

use crate::component::WaterSurface;

/// Quantum for the wind-speed ratio. 1/32 is fine enough that a rising wind
/// reads as continuous and coarse enough that a settling one stops rebuilding.
const RATIO_STEP: f32 = 1.0 / 32.0;

/// Quantum for the bearing offset, in degrees. Two degrees is below the angle
/// at which a change in swell heading is visible on open water.
const BEARING_STEP: f32 = 2.0;

/// The authored sea, captured so the world wind can scale it without
/// destroying it.
///
/// Not serialized — it is a cache of what the scene already stores, rebuilt on
/// load and re-captured whenever the author edits a cascade (see
/// [`apply_world_wind`] for how an external edit is told apart from our own
/// write).
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
pub struct WaterWindBaseline {
    /// Authored `(wind_speed, wind_direction)` per cascade.
    cascades: Vec<(f32, f32)>,
    /// What we last wrote, so an author's edit is distinguishable from ours.
    last_written: Vec<(f32, f32)>,
}

fn quantize(v: f32, step: f32) -> f32 {
    (v / step).round() * step
}

/// Scale each surface's cascades by the smoothed world wind.
pub fn apply_world_wind(
    mut commands: Commands,
    wind: Option<Res<WindState>>,
    mut surfaces: Query<(Entity, &mut WaterSurface, Option<&mut WaterWindBaseline>)>,
) {
    let wind = wind.as_deref().copied().unwrap_or_default();

    for (entity, mut surface, baseline) in surfaces.iter_mut() {
        if !surface.follow_world_wind {
            // Leaving the cascades wherever the last scaled write put them
            // would freeze the ocean at whatever wind happened to be blowing
            // when the flag was cleared. Restore the authored sea instead.
            if let Some(mut baseline) = baseline {
                restore(&mut surface, &baseline);
                baseline.last_written.clear();
                baseline.cascades.clear();
                commands.entity(entity).remove::<WaterWindBaseline>();
            }
            continue;
        }

        let current: Vec<(f32, f32)> = surface
            .cascades
            .iter()
            .map(|c| (c.wind_speed, c.wind_direction))
            .collect();

        let mut baseline = match baseline {
            // Re-capture when what's on the cascades isn't what we last wrote:
            // either the author edited them in the inspector, or a cascade was
            // added/removed. Either way the authored sea has changed and the
            // old baseline describes an ocean that no longer exists.
            Some(b) if b.last_written == current => b,
            Some(mut b) => {
                b.cascades = current.clone();
                b.last_written = current;
                b
            }
            None => {
                commands.entity(entity).insert(WaterWindBaseline {
                    cascades: current.clone(),
                    last_written: current,
                });
                continue;
            }
        };

        let response = surface.wind_response.max(0.0);
        let ratio = quantize(
            (wind.sea_state_speed / REFERENCE_WIND_SPEED * response).max(0.0),
            RATIO_STEP,
        );
        let bearing = quantize(wind.direction_degrees(), BEARING_STEP);

        let mut written = Vec::with_capacity(baseline.cascades.len());
        for (cascade, &(base_speed, base_dir)) in
            surface.cascades.iter_mut().zip(baseline.cascades.iter())
        {
            // Floored, not clamped to zero: a JONSWAP spectrum at literally
            // zero wind speed divides by it (see `jonswap_peak_frequency`), and
            // a dead-flat ocean should be flat because the waves are tiny, not
            // because the maths blew up.
            let speed = (base_speed * ratio).max(0.05);
            // The cascade keeps its authored bearing *relative* to the sea's
            // reference direction, so a cross-swell stays a cross-swell as the
            // wind veers.
            let direction = base_dir + bearing;
            cascade.wind_speed = speed;
            cascade.wind_direction = direction;
            written.push((speed, direction));
        }
        baseline.last_written = written;
    }
}

fn restore(surface: &mut WaterSurface, baseline: &WaterWindBaseline) {
    for (cascade, &(speed, direction)) in
        surface.cascades.iter_mut().zip(baseline.cascades.iter())
    {
        cascade.wind_speed = speed;
        cascade.wind_direction = direction;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_surface(surface: WaterSurface) -> (App, Entity) {
        let mut app = App::new();
        app.add_systems(Update, apply_world_wind);
        app.init_resource::<WindState>();
        let entity = app.world_mut().spawn(surface).id();
        (app, entity)
    }

    /// First pass only captures. Scaling on the same frame we captured would
    /// bake the current wind into the baseline itself.
    #[test]
    fn first_pass_captures_the_authored_sea() {
        let surface = WaterSurface::default();
        let authored: Vec<f32> = surface.cascades.iter().map(|c| c.wind_speed).collect();
        let (mut app, entity) = app_with_surface(surface);
        app.update();
        let after: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        assert_eq!(authored, after);
        assert!(app.world().get::<WaterWindBaseline>(entity).is_some());
    }

    /// A harder wind gives a harsher sea, and the swell-to-wind-sea ratio the
    /// author set survives it.
    #[test]
    fn stronger_wind_scales_every_cascade_by_the_same_factor() {
        let surface = WaterSurface::default();
        let authored: Vec<f32> = surface.cascades.iter().map(|c| c.wind_speed).collect();
        let (mut app, entity) = app_with_surface(surface);
        app.update(); // capture
        app.world_mut().resource_mut::<WindState>().sea_state_speed = REFERENCE_WIND_SPEED;
        app.update();

        let scaled: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        for (a, s) in authored.iter().zip(scaled.iter()) {
            assert!(
                (a - s).abs() < 1e-3,
                "ratio 1.0 should reproduce the authored sea: {a} vs {s}"
            );
        }

        // Half the wind, half the sea — uniformly.
        app.world_mut().resource_mut::<WindState>().sea_state_speed =
            REFERENCE_WIND_SPEED * 0.5;
        app.update();
        let halved: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        for (a, h) in authored.iter().zip(halved.iter()) {
            assert!((a * 0.5 - h).abs() < 1e-2, "{a} * 0.5 != {h}");
        }
    }

    /// A settled wind must not keep rewriting the cascades — each distinct
    /// `wind_speed` costs a full spectrum re-bake.
    #[test]
    fn a_settled_wind_stops_writing() {
        let (mut app, entity) = app_with_surface(WaterSurface::default());
        app.update();
        app.world_mut().resource_mut::<WindState>().sea_state_speed = 7.0;
        app.update();
        let settled: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        // A wind drift far below one quantum must not move the sea at all.
        app.world_mut().resource_mut::<WindState>().sea_state_speed = 7.0 + 1e-4;
        app.update();
        let again: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        assert_eq!(settled, again);
    }

    /// Turning the flag off puts the authored sea back rather than freezing
    /// whatever the wind last scaled it to.
    #[test]
    fn opting_out_restores_the_authored_sea() {
        let surface = WaterSurface::default();
        let authored: Vec<f32> = surface.cascades.iter().map(|c| c.wind_speed).collect();
        let (mut app, entity) = app_with_surface(surface);
        app.update();
        app.world_mut().resource_mut::<WindState>().sea_state_speed = 2.0;
        app.update();

        app.world_mut()
            .get_mut::<WaterSurface>(entity)
            .unwrap()
            .follow_world_wind = false;
        app.update();

        let restored: Vec<f32> = app
            .world()
            .get::<WaterSurface>(entity)
            .unwrap()
            .cascades
            .iter()
            .map(|c| c.wind_speed)
            .collect();
        assert_eq!(authored, restored);
        assert!(app.world().get::<WaterWindBaseline>(entity).is_none());
    }
}
