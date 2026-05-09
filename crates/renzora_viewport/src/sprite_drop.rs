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
use bevy_egui::egui;

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora::core::{CurrentProject, EditorCamera2d, SpriteImagePath};
use renzora_editor::EditorCommands;
use renzora_ui::asset_drag::AssetDragPayload;

use crate::ViewportState;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "ktx2", "rmip"];

/// Called from the viewport panel's `ui()` each frame. On release of an
/// image drag-drop payload over the 2D viewport, queues a deferred
/// command that decides whether to retarget an existing sprite or spawn
/// a new one.
pub fn check_viewport_sprite_drop(ui: &mut egui::Ui, world: &World, viewport_rect: egui::Rect) {
    // Only fire in 2D view — 3D mode is owned by model/material/shape drops.
    let view = world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default();
    if view != ViewportView::Two {
        return;
    }

    let Some(payload) = world.get_resource::<AssetDragPayload>() else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(IMAGE_EXTENSIONS) {
        return;
    }

    let pointer_pos = ui.ctx().pointer_latest_pos();
    let pointer_in_viewport = pointer_pos.map_or(false, |p| viewport_rect.contains(p));
    if !pointer_in_viewport {
        return;
    }

    let pointer_released = !ui.ctx().input(|i| i.pointer.any_down());
    if !pointer_released {
        return;
    }

    let path = payload.path.clone();
    let screen_pos = pointer_pos.unwrap_or(viewport_rect.center());
    let vp_rect = viewport_rect;

    if let Some(commands) = world.get_resource::<EditorCommands>() {
        commands.push(move |world: &mut World| {
            handle_sprite_drop(world, path, screen_pos, vp_rect);
        });
    }
}

fn handle_sprite_drop(
    world: &mut World,
    abs_path: PathBuf,
    screen_pos: egui::Pos2,
    vp_rect: egui::Rect,
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
        let (camera_q, vp_state) = state.get(world);

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
        let sprites = state.get(world);

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
