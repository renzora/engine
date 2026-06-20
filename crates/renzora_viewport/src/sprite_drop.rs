//! Drag-and-drop image-to-sprite — when the user drags an image asset
//! from the asset browser onto the 2D viewport, either assign it to the
//! sprite under the pointer or spawn a new Sprite at the drop point.
//!
//! Sibling of `material_drop` for 3D meshes; gated on
//! `viewport_view == Two` so dropping an image in 3D doesn't try to
//! spawn a sprite where a model would be expected.

use std::path::PathBuf;

use bevy::ecs::system::SystemState;
use bevy::prelude::*;
use bevy::sprite::Sprite;

use renzora::core::{CurrentProject, EditorCamera2d, SpriteImagePath};

use crate::ViewportState;

pub(crate) const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "ktx2", "rmip"];

/// Commit an image-to-sprite drop at the given viewport-space pointer. Shared by
/// the egui drop check and the native bevy_ui drop
/// (`native_drop::commit_viewport_drop`). `screen_pos` / `vp_rect` are in window
/// logical pixels.
pub(crate) fn commit_sprite_drop(
    world: &mut World,
    screen_pos: Vec2,
    vp_rect: Rect,
    abs_path: PathBuf,
) {
    // Asset path stored in components stays project-relative so it
    // survives moving the project to a different machine.
    let path_str = if let Some(project) = world.get_resource::<CurrentProject>() {
        project.make_asset_relative(&abs_path)
    } else {
        abs_path.to_string_lossy().to_string()
    };

    // Project the drop point through the 2D editor camera to world space.
    let world_pos_2d = {
        let mut state: SystemState<(
            Query<(&Camera, &GlobalTransform), With<EditorCamera2d>>,
            Res<ViewportState>,
        )> = SystemState::new(world);
        let (camera_q, vp_state) = state.get(world).unwrap();

        let Ok((camera, cam_gt)) = camera_q.single() else {
            return;
        };
        let vp_x = screen_pos.x - vp_rect.min.x;
        let vp_y = screen_pos.y - vp_rect.min.y;
        let image_size = vp_state.current_size.as_vec2();
        if image_size.x <= 0.0 || image_size.y <= 0.0 || vp_rect.width() <= 0.0 {
            return;
        }
        let render_pos = Vec2::new(
            vp_x * image_size.x / vp_rect.width(),
            vp_y * image_size.y / vp_rect.height(),
        );
        match camera.viewport_to_world_2d(cam_gt, render_pos) {
            Ok(p) => p,
            Err(_) => return,
        }
    };

    // AABB hit test against existing sprites — topmost-Z wins.
    let hit_entity = {
        let mut state: SystemState<Query<(Entity, &Sprite, &GlobalTransform)>> =
            SystemState::new(world);
        let sprites = state.get(world).unwrap();

        let mut best: Option<(Entity, f32)> = None;
        for (entity, sprite, gt) in &sprites {
            let Some(size) = sprite.custom_size else {
                continue;
            };
            if size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let pos = gt.translation();
            let half = size * 0.5;
            if (world_pos_2d.x - pos.x).abs() <= half.x && (world_pos_2d.y - pos.y).abs() <= half.y
            {
                match best {
                    None => best = Some((entity, pos.z)),
                    Some((_, z)) if pos.z > z => best = Some((entity, pos.z)),
                    _ => {}
                }
            }
        }
        best.map(|(e, _)| e)
    };

    if let Some(entity) = hit_entity {
        // Retarget the existing sprite's image — keeps the user's
        // chosen size/position so they can iterate quickly. Always
        // `insert` (replace) so the SpriteImagePath lifecycle observer
        // fires; in-place mutation via `Mut<>` doesn't trigger
        // observers.
        if let Ok(mut em) = world.get_entity_mut(entity) {
            em.insert(SpriteImagePath(path_str));
        }
    } else {
        // Spawn a fresh sprite at the drop point. Name from the file
        // stem so the hierarchy reads naturally; default size is a
        // visible square the user can resize via the corner handles.
        let name = abs_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "Sprite".to_string());
        let world_pos = Vec3::new(world_pos_2d.x, world_pos_2d.y, 0.0);
        // `custom_size: None` → Bevy uses the source image's native
        // pixel dimensions for the sprite's world-space size. The
        // observer in `scene_io::on_sprite_image_path_inserted` also
        // sets this on path bind, so this is mostly defensive.
        world.spawn((
            Name::new(name),
            Transform::from_translation(world_pos),
            Sprite {
                color: Color::WHITE,
                custom_size: None,
                ..default()
            },
            SpriteImagePath(path_str),
        ));
    }
}
