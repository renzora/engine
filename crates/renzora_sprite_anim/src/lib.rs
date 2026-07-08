//! Renzora Sprite Animation (runtime) — resolves [`SpriteImages`] to the sprite.
//!
//! 2D sprite animation reuses the engine's existing systems: [`SpriteSheet`]
//! cell-cropping + the property timeline keyframing `SpriteSheet.frame` (which
//! cell) and [`SpriteImages::active`] (which sheet). The only runtime piece
//! *this* feature adds is the one system below: when `SpriteImages.active`
//! changes (a keyframed sheet-switch, or the editor picking a sheet), it copies
//! the active path into the entity's [`SpriteImagePath`], so the ordinary sprite
//! pipeline binds the texture and the inspector's "Sprite Image" slot shows it.
//! `SpriteImagePath` stays the single rendered-image source of truth;
//! `SpriteImages` is the switchable library driving it.
//!
//! Runs in both the editor and the shipped game so a keyframed sheet-switch
//! plays back identically.

use bevy::prelude::*;

use renzora::core::{SpriteImagePath, SpriteImages};

#[derive(Default)]
pub struct SpriteImagesPlugin;

impl Plugin for SpriteImagesPlugin {
    fn build(&self, app: &mut App) {
        info!("[runtime] SpriteImagesPlugin");
        app.add_systems(Update, sync_sprite_images_path);
    }
}

renzora::add!(SpriteImagesPlugin);

/// When [`SpriteImages`] changes — the property timeline keyframing `active`, a
/// scene-load insert, or the editor appending/switching sheets — mirror the
/// active image into [`SpriteImagePath`]. Inserting (not mutating) it fires the
/// engine's sprite-image lifecycle observer, which binds `Sprite.image` (and
/// spawns a `Sprite` if the entity doesn't have one yet). The compare-first
/// guard means an unchanged `active` doesn't re-insert every frame.
fn sync_sprite_images_path(
    changed: Query<(Entity, &SpriteImages, Option<&SpriteImagePath>), Changed<SpriteImages>>,
    mut commands: Commands,
) {
    for (entity, images, current) in &changed {
        let Some(path) = images.active_path() else { continue };
        if path.is_empty() {
            continue;
        }
        if current.map(|p| p.0.as_str()) != Some(path) {
            commands.entity(entity).insert(SpriteImagePath(path.to_string()));
        }
    }
}
