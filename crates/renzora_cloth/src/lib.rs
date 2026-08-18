//! Cloth physics distribution plugin.
//!
//! Wraps the vendored `bevy_silk` verlet cloth engine and registers it with the
//! Renzora runtime via `renzora::add!`. Built as a `cdylib` and dlopen'd from
//! `plugins/` at startup — the same distribution-plugin model the postprocess
//! effects use, so a shipped game gets cloth only if this plugin sits in
//! `plugins/`.
//!
//! Add a `bevy_silk::prelude::ClothBuilder` to any entity with a mesh to turn
//! it into cloth (see the `bevy_silk` docs for pinning / stick-generation).
//!
//! Cloth follows the world wind (`renzora::WindState`), so a flag and the grass
//! under it move together. `bevy_silk` has had a `Winds` resource all along;
//! nothing had ever written to it, which is why cloth used to hang dead still
//! in a scene full of moving foliage.

use bevy::prelude::*;
use bevy_silk::prelude::{Wind, Winds};
use renzora::WindState;

/// Index in [`Winds::wind_forces`] that the world wind owns.
///
/// Slot 0 rather than "the whole list", so a scene can still push extra
/// hand-authored forces (a scripted downdraft, a fan) and have them add on top
/// instead of being clobbered every frame.
const WORLD_WIND_SLOT: usize = 0;

/// Mirror [`WindState`] into `bevy_silk`'s wind resource.
///
/// A `ConstantWind` refreshed per frame, not a `SinWave`: the gust envelope is
/// already evaluated in `WindState`, and letting silk apply its own sine on top
/// would beat against it at some unrelated frequency — cloth would gust when
/// the grass beside it did not.
fn sync_cloth_wind(wind: Option<Res<WindState>>, mut winds: ResMut<Winds>) {
    let velocity = wind.as_deref().copied().unwrap_or_default().velocity();
    let world = Wind::ConstantWind { velocity };
    match winds.wind_forces.get_mut(WORLD_WIND_SLOT) {
        Some(slot) => *slot = world,
        None => winds.wind_forces.push(world),
    }
}

/// Runtime-scope plugin that installs `bevy_silk`'s cloth simulation.
#[derive(Default)]
pub struct ClothPlugin;

impl Plugin for ClothPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] ClothPlugin (bevy_silk verlet cloth)");
        app.add_plugins(bevy_silk::prelude::ClothPlugin)
            .init_resource::<Winds>()
            .add_systems(Update, sync_cloth_wind);
    }
}

renzora::add!(ClothPlugin);
