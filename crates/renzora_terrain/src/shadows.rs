//! Sun shadows that reach across the terrain.
//!
//! Bevy's default cascades end 150 m from the camera, which suits a room or a
//! street and not a landscape: pull back to see a 512 m terrain whole and
//! every shadow on it vanished at once, since all of it sat past the last
//! cascade. A sun still on those defaults gets its cascades stretched to the
//! largest terrain's span, plus room to pull back from it, keeping Bevy's 10 m
//! first cascade so shadows up close are as sharp as before.
//!
//! Only defaults are touched. Once someone sets a sun's cascades by hand (the
//! inspector's Cascades section), they differ from both Bevy's defaults and
//! the bounds written here, and this leaves that sun alone from then on.

use bevy::light::{CascadeShadowConfig, CascadeShadowConfigBuilder};
use bevy::prelude::*;

use crate::data::TerrainData;

/// The cascade bounds this module last gave a sun, to tell its own writes
/// from a person's.
#[derive(Component)]
pub struct TerrainFittedCascades(Vec<f32>);

/// The farthest shadow reach this will set, in metres. Past this a shadow map
/// texel covers metres of ground and the shadows it draws are mush anyway.
const MAX_REACH: f32 = 3000.0;

/// How far the cascades should reach for terrains this size: the largest
/// diagonal, half again so the far side still has shadows from beyond the
/// near edge. Never less than Bevy's 150 m.
pub fn shadow_reach<'a>(terrains: impl IntoIterator<Item = &'a TerrainData>) -> f32 {
    let diagonal = terrains
        .into_iter()
        .map(|t| Vec2::new(t.total_width(), t.total_depth()).length())
        .fold(0.0, f32::max);
    (diagonal * 1.5).clamp(CascadeShadowConfigBuilder::default().maximum_distance, MAX_REACH)
}

fn cascades_reaching(reach: f32) -> CascadeShadowConfig {
    CascadeShadowConfigBuilder { maximum_distance: reach, ..default() }.build()
}

pub fn fit_sun_cascades_to_terrain(
    mut commands: Commands,
    terrains: Query<&TerrainData>,
    mut suns: Query<(Entity, &mut CascadeShadowConfig, Option<&TerrainFittedCascades>), With<DirectionalLight>>,
    mut last_reach: Local<f32>,
) {
    let reach = if terrains.is_empty() {
        CascadeShadowConfigBuilder::default().maximum_distance
    } else {
        shadow_reach(terrains.iter())
    };
    let defaults = CascadeShadowConfig::default();
    let reach_moved = reach != *last_reach;
    *last_reach = reach;
    for (sun, mut config, fitted) in &mut suns {
        let ours = match fitted {
            Some(fitted) => fitted.0 == config.bounds,
            None => config.bounds == defaults.bounds,
        };
        if !ours {
            if fitted.is_some() {
                commands.entity(sun).remove::<TerrainFittedCascades>();
            }
            continue;
        }
        if fitted.is_some() && !reach_moved {
            continue;
        }
        let wanted = cascades_reaching(reach);
        if wanted.bounds != config.bounds {
            *config = wanted;
        }
        commands.entity(sun).insert(TerrainFittedCascades(config.bounds.clone()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terrain(chunks: u32) -> TerrainData {
        TerrainData { chunks_x: chunks, chunks_z: chunks, ..default() }
    }

    #[test]
    fn reach_covers_the_terrain_and_never_shrinks_below_bevys() {
        assert_eq!(shadow_reach([&terrain(1)]), 150.0, "a 64 m terrain keeps the default");
        let big = terrain(8);
        let span = Vec2::new(big.total_width(), big.total_depth()).length();
        assert!((shadow_reach([&big]) - span * 1.5).abs() < 0.01);
        assert_eq!(shadow_reach([&terrain(200)]), MAX_REACH);
    }

    #[test]
    fn a_default_sun_is_fitted_and_a_hand_set_one_is_left_alone() {
        let mut app = App::new();
        app.add_systems(Update, fit_sun_cascades_to_terrain);
        app.world_mut().spawn(terrain(8));
        let default_sun = app.world_mut().spawn((DirectionalLight::default(), CascadeShadowConfig::default())).id();
        let hand_set = cascades_reaching(42.0);
        let custom_sun = app.world_mut().spawn((DirectionalLight::default(), hand_set.clone())).id();
        app.update();
        let far = |app: &App, e| *app.world().get::<CascadeShadowConfig>(e).unwrap().bounds.last().unwrap();
        assert!(far(&app, default_sun) > 1000.0);
        assert_eq!(far(&app, custom_sun), 42.0);

        // Edited by hand after being fitted: left as the person set it.
        *app.world_mut().get_mut::<CascadeShadowConfig>(default_sun).unwrap() = cascades_reaching(80.0);
        app.update();
        app.update();
        assert_eq!(far(&app, default_sun), 80.0);
    }
}
