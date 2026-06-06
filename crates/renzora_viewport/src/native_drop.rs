//! Native (bevy_ui) viewport asset-drop dispatch.
//!
//! Under the egui backend the viewport panel's `ui()` calls the
//! `check_viewport_*_drop` helpers directly, reading egui pointer state. Under
//! the bevy_ui shell that panel body is replaced by [`crate::native_viewport`],
//! so those checks never run. These systems are the native equivalents: each
//! one, on left-mouse release of a matching [`AssetDragPayload`] over the
//! focused viewport, commits the same drop the egui path would have.
//!
//! They are gated on the bevy_ui backend in `lib.rs`, so they never double-fire
//! with the egui drop checks.

use bevy::math::Rect;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::path::PathBuf;

use renzora::core::viewport_types::{ViewportSettings, ViewportView};
use renzora_ui::asset_drag::AssetDragPayload;

use crate::ViewportState;

/// If the left mouse button was just released this frame with a detached
/// [`AssetDragPayload`] whose extension matches `exts` and the cursor over the
/// focused viewport, return `(cursor, viewport_rect, abs_path, name)` — all in
/// window logical pixels. Otherwise `None`.
///
/// Mirrors the egui drop-check guards (detached + extension + pointer-in-rect +
/// released) but sources the geometry from [`ViewportState`] and the window
/// cursor instead of egui.
pub(crate) fn released_drop_on_viewport(
    world: &mut World,
    exts: &[&str],
) -> Option<(Vec2, Rect, PathBuf, String)> {
    // Released this frame?
    if !world
        .get_resource::<ButtonInput<MouseButton>>()
        .is_some_and(|m| m.just_released(MouseButton::Left))
    {
        return None;
    }

    // A detached, extension-matching payload?
    let (path, name) = {
        let payload = world.get_resource::<AssetDragPayload>()?;
        if !payload.is_detached || !payload.matches_extensions(exts) {
            return None;
        }
        (payload.path.clone(), payload.name.clone())
    };

    // Focused viewport rect in window logical pixels.
    let (min, size) = {
        let vp = world.get_resource::<ViewportState>()?;
        (vp.screen_position, vp.screen_size)
    };
    let max = min + size;

    // Cursor over that rect?
    let mut q = world.query_filtered::<&Window, With<PrimaryWindow>>();
    let cursor = q.iter(world).next().and_then(|w| w.cursor_position())?;
    if cursor.x < min.x || cursor.y < min.y || cursor.x > max.x || cursor.y > max.y {
        return None;
    }

    Some((cursor, Rect::from_corners(min, max), path, name))
}

/// Native (bevy_ui) counterpart of `material_drop::check_viewport_material_drop`.
pub fn native_material_drop(world: &mut World) {
    let Some((cursor, vp_rect, path, _)) =
        released_drop_on_viewport(world, crate::material_drop::MATERIAL_EXTENSIONS)
    else {
        return;
    };
    crate::material_drop::commit_material_drop(world, cursor, vp_rect, path);
}

/// Native (bevy_ui) counterpart of `scene_drop::check_viewport_scene_drop`.
pub fn native_scene_drop(world: &mut World) {
    let Some((cursor, vp_rect, path, _)) =
        released_drop_on_viewport(world, crate::scene_drop::SCENE_EXTENSIONS)
    else {
        return;
    };
    crate::scene_drop::commit_scene_drop(world, cursor, vp_rect, path);
}

/// Native (bevy_ui) counterpart of `sprite_drop::check_viewport_sprite_drop`.
/// Only fires in 2D view — 3D mode is owned by the model/material drops.
pub fn native_sprite_drop(world: &mut World) {
    if world
        .get_resource::<ViewportSettings>()
        .map(|s| s.viewport_view)
        .unwrap_or_default()
        != ViewportView::Two
    {
        return;
    }
    let Some((cursor, vp_rect, path, _)) =
        released_drop_on_viewport(world, crate::sprite_drop::IMAGE_EXTENSIONS)
    else {
        return;
    };
    crate::sprite_drop::commit_sprite_drop(world, cursor, vp_rect, path);
}
