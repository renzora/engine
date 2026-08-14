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

#[cfg(test)]
mod tests {
    use super::*;
    use renzora_test_harness::{minimal_app, pump};

    fn app() -> App {
        let mut app = minimal_app();
        app.add_systems(Update, sync_sprite_images_path);
        app
    }

    fn images(paths: &[&str], index: u32) -> SpriteImages {
        SpriteImages {
            images: paths.iter().map(|s| s.to_string()).collect(),
            index,
        }
    }

    fn path_of(app: &App, e: Entity) -> Option<String> {
        app.world().get::<SpriteImagePath>(e).map(|p| p.0.clone())
    }

    #[test]
    fn the_active_sheet_is_mirrored_into_the_rendered_path() {
        let mut app = app();
        let e = app
            .world_mut()
            .spawn(images(&["sprites/idle.png", "sprites/run.png"], 1))
            .id();
        pump(&mut app, 1);
        assert_eq!(path_of(&app, e).as_deref(), Some("sprites/run.png"));
    }

    /// A keyframed sheet-switch is just a mutation of `index`. If the `Changed`
    /// filter or the compare-first guard broke, the sprite would keep drawing
    /// the old sheet.
    #[test]
    fn switching_the_index_rebinds_the_path() {
        let mut app = app();
        let e = app
            .world_mut()
            .spawn(images(&["a.png", "b.png"], 0))
            .id();
        pump(&mut app, 1);
        assert_eq!(path_of(&app, e).as_deref(), Some("a.png"));

        app.world_mut().get_mut::<SpriteImages>(e).unwrap().index = 1;
        pump(&mut app, 1);
        assert_eq!(path_of(&app, e).as_deref(), Some("b.png"));
    }

    /// Inserting `SpriteImagePath` fires a lifecycle observer that rebinds the
    /// texture, so re-inserting an unchanged value every frame would rebind the
    /// sprite every frame. The guard is the whole reason the compare exists.
    #[test]
    fn an_unchanged_active_sheet_is_not_re_inserted() {
        let mut app = app();
        let e = app.world_mut().spawn(images(&["a.png"], 0)).id();
        pump(&mut app, 1);

        let first = app
            .world()
            .entity(e)
            .get_change_ticks::<SpriteImagePath>()
            .expect("path inserted")
            .changed;

        // Touch `SpriteImages` without changing which sheet is active: the
        // `Changed` filter fires, but the guard must stop the insert.
        app.world_mut().get_mut::<SpriteImages>(e).unwrap().index = 0;
        pump(&mut app, 2);

        let later = app
            .world()
            .entity(e)
            .get_change_ticks::<SpriteImagePath>()
            .unwrap()
            .changed;
        assert_eq!(first, later, "SpriteImagePath was re-inserted needlessly");
    }

    #[test]
    fn an_index_past_the_end_wraps_rather_than_blanking_the_sprite() {
        let mut app = app();
        let e = app.world_mut().spawn(images(&["a.png", "b.png"], 5)).id();
        pump(&mut app, 1);
        // 5 % 2 == 1
        assert_eq!(path_of(&app, e).as_deref(), Some("b.png"));
    }

    #[test]
    fn an_empty_library_leaves_the_path_alone() {
        let mut app = app();
        let e = app.world_mut().spawn(images(&[], 0)).id();
        pump(&mut app, 1);
        assert!(path_of(&app, e).is_none());
    }

    /// An empty string would bind a texture handle to nothing and blank the
    /// sprite; the guard skips it and keeps whatever was already drawn.
    #[test]
    fn an_empty_path_is_skipped() {
        let mut app = app();
        let e = app.world_mut().spawn(images(&[""], 0)).id();
        pump(&mut app, 1);
        assert!(path_of(&app, e).is_none());
    }

    #[test]
    fn the_plugin_installs_the_sync_system() {
        let mut app = minimal_app();
        app.add_plugins(SpriteImagesPlugin);
        let e = app.world_mut().spawn(images(&["via_plugin.png"], 0)).id();
        pump(&mut app, 1);
        assert_eq!(path_of(&app, e).as_deref(), Some("via_plugin.png"));
    }
}
