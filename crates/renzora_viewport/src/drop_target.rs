//! Native (bevy_ui) viewport asset-drop dispatch for `AssetDragPayload` drops
//! that have no live preview entity of their own (material / scene / sprite).
//!
//! Under the egui backend the viewport panel's `ui()` calls the
//! `check_viewport_*_drop` helpers, which read the payload + pointer straight
//! from egui on the release frame. Under the bevy_ui shell that panel body is
//! replaced by [`crate::panel`], so those checks never run — and we
//! can't simply read the payload on release either: the native asset browser
//! removes it via a deferred command on mouse-up, and any intervening exclusive
//! system flushes that removal before a release-frame read would see it.
//!
//! So instead of reading the payload *at* release, [`arm_viewport_drop`] records
//! the drop candidate every frame *while* a compatible payload hovers the
//! focused viewport, and [`commit_viewport_drop`] consumes that armed snapshot
//! on the release edge. The model drop is handled separately in
//! [`crate::model_drop::native_model_drop`] (it drives off the preview ghost).
//!
//! All gated on the bevy_ui backend in `lib.rs`, so they never double-fire with
//! the egui drop checks.

use bevy::math::Rect;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::path::PathBuf;

use bevy::window::SystemCursorIcon;
use renzora::core::viewport_types::{DropCursorRequest, ViewportSettings, ViewportView};
use renzora::core::MaterialTarget;
use renzora_editor_framework::EditorCommands;
use renzora_ui::asset_drag::AssetDragPayload;

use crate::ViewportState;

/// Which native commit a hovering payload routes to.
#[derive(Clone, Copy)]
enum DropKind {
    /// A post-process effect shader, which joins the scene's Post Process list.
    Effect,
    Material,
    Scene,
    Sprite,
    Particle,
    Blueprint,
    Gaussian,
}

/// The last drop candidate seen hovering the focused viewport. Captured while
/// the drag is in flight so the release frame doesn't have to re-read the
/// (by-then-removed) payload.
struct Armed {
    path: PathBuf,
    cursor: Vec2,
    vp_rect: Rect,
    kind: DropKind,
}

#[derive(Resource, Default)]
pub(crate) struct ArmedViewportDrop(Option<Armed>);

/// Classify a payload extension into a [`DropKind`]. `None` if the payload isn't
/// a material/scene/sprite drop (or is a sprite drop outside 2D view).
///
/// `is_effect` says whether the payload is a post-process effect shader. It is
/// read from the file, so the caller caches it per drag rather than this
/// reading the disk every hovered frame. Checked before materials: an effect is
/// a `.wesl` too, and taken for a material it crashes wgpu.
fn classify(payload: &AssetDragPayload, view: ViewportView, is_effect: bool) -> Option<DropKind> {
    if is_effect {
        Some(DropKind::Effect)
    } else if payload.matches_extensions(crate::material_drop::MATERIAL_EXTENSIONS) {
        Some(DropKind::Material)
    } else if payload.matches_extensions(crate::particle_drop::PARTICLE_EXTENSIONS) {
        Some(DropKind::Particle)
    } else if payload.matches_extensions(crate::blueprint_drop::BLUEPRINT_EXTENSIONS) {
        Some(DropKind::Blueprint)
    } else if payload.matches_extensions(crate::scene_drop::SCENE_EXTENSIONS) {
        Some(DropKind::Scene)
    } else if view == ViewportView::Two
        && payload.matches_extensions(crate::sprite_drop::IMAGE_EXTENSIONS)
    {
        // Sprites only drop in 2D view — 3D is owned by the model/material drops.
        Some(DropKind::Sprite)
    } else if view != ViewportView::Two
        && payload.matches_extensions(crate::gaussian_drop::GAUSSIAN_EXTENSIONS)
    {
        // Splat clouds are 3D-only content — no 2D-view drop target.
        Some(DropKind::Gaussian)
    } else {
        None
    }
}

/// Every frame: arm the drop if a compatible, detached payload is hovering the
/// focused viewport; disarm if a payload is present but not a valid hover.
///
/// When no payload is present (e.g. the release frame, after the browser removed
/// it) the armed snapshot is left untouched so [`commit_viewport_drop`] can still
/// consume the value captured on the previous frame.
#[allow(clippy::too_many_arguments)]
pub fn arm_viewport_drop(
    payload: Option<Res<AssetDragPayload>>,
    settings: Option<Res<ViewportSettings>>,
    viewport: Option<Res<ViewportState>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut target_of: Local<Option<(PathBuf, Option<MaterialTarget>)>>,
    mut effect_of: Local<Option<(PathBuf, bool)>>,
    mut refusing: Local<bool>,
    mut cursor: ResMut<DropCursorRequest>,
    mut armed: ResMut<ArmedViewportDrop>,
) {
    // "Not allowed" is ours to clear only if we set it; see `DropCursorRequest`.
    let mut set_refusing = |refuse: bool, cursor: &mut DropCursorRequest| {
        if refuse {
            if cursor.0 != Some(SystemCursorIcon::NotAllowed) {
                cursor.0 = Some(SystemCursorIcon::NotAllowed);
            }
            *refusing = true;
        } else if std::mem::take(&mut *refusing) {
            cursor.0 = None;
        }
    };
    let Some(payload) = payload else {
        set_refusing(false, &mut cursor);
        return; // keep the last snapshot for the release frame
    };
    let (Some(viewport), Some(window)) = (viewport, window.single().ok()) else {
        set_refusing(false, &mut cursor);
        armed.0 = None;
        return;
    };
    let view = settings.map(|s| s.viewport_view).unwrap_or_default();

    // `hovered` is the panel-aware signal: it's the viewport node's own
    // `cursor_over` (clip- and stack-index-aware), already cleared when the
    // cursor is over another panel, an overlay, or a modal. Requiring it — on
    // top of the geometric rect — keeps a drop from arming the viewport while
    // the cursor is actually over the hierarchy/inspector (which otherwise lets
    // BOTH the viewport and that panel commit the same drop → a duplicate spawn).
    let valid = payload.is_detached
        && viewport.hovered
        && window.cursor_position().is_some_and(|c| {
            let min = viewport.screen_position;
            let max = min + viewport.screen_size;
            c.x >= min.x && c.y >= min.y && c.x <= max.x && c.y <= max.y
        });

    if !valid {
        set_refusing(false, &mut cursor);
        armed.0 = None;
        return;
    }
    if effect_of.as_ref().map(|(p, _)| p) != Some(&payload.path) {
        *effect_of = Some((payload.path.clone(), renzora::is_effect_file(&payload.path)));
    }
    let is_effect = effect_of.as_ref().is_some_and(|(_, e)| *e);
    let Some(kind) = classify(&payload, view, is_effect) else {
        set_refusing(false, &mut cursor);
        armed.0 = None;
        return;
    };

    // A Sprite material goes on a sprite, so only in the 2D view; any other
    // material goes on a mesh, so only in 3D. Refused before the drop, with
    // the cursor saying so, rather than after it with a Console line.
    if matches!(kind, DropKind::Material) {
        if target_of.as_ref().map(|(p, _)| p) != Some(&payload.path) {
            *target_of = Some((payload.path.clone(), MaterialTarget::of_file(&payload.path)));
        }
        let material = target_of.as_ref().and_then(|(_, t)| *t).unwrap_or(MaterialTarget::Mesh);
        let fits_view = match material {
            MaterialTarget::Sprite => view == ViewportView::Two,
            MaterialTarget::Mesh => view != ViewportView::Two,
        };
        if !fits_view {
            set_refusing(true, &mut cursor);
            armed.0 = None;
            return;
        }
    }
    set_refusing(false, &mut cursor);

    let cursor = window.cursor_position().unwrap();
    let min = viewport.screen_position;
    armed.0 = Some(Armed {
        path: payload.path.clone(),
        cursor,
        vp_rect: Rect::from_corners(min, min + viewport.screen_size),
        kind,
    });
}

/// On the left-mouse-release edge, commit the armed drop (if any) by queuing the
/// matching `commit_*_drop` through [`EditorCommands`] — same handler the egui
/// path runs, drained by `drain_editor_commands_native` under bevy_ui.
pub fn commit_viewport_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    mut armed: ResMut<ArmedViewportDrop>,
    cmds: Option<Res<EditorCommands>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let Some(Armed {
        path,
        cursor,
        vp_rect,
        kind,
    }) = armed.0.take()
    else {
        return;
    };
    let Some(cmds) = cmds else {
        return;
    };
    cmds.push(move |world: &mut World| match kind {
        DropKind::Effect => crate::effect_drop::commit_effect_drop(world, path),
        DropKind::Material => crate::material_drop::commit_material_drop(world, cursor, vp_rect, path),
        DropKind::Scene => crate::scene_drop::commit_scene_drop(world, cursor, vp_rect, path),
        DropKind::Sprite => crate::sprite_drop::commit_sprite_drop(world, cursor, vp_rect, path),
        DropKind::Particle => crate::particle_drop::commit_particle_drop(world, cursor, vp_rect, path),
        DropKind::Blueprint => crate::blueprint_drop::commit_blueprint_drop(world, cursor, vp_rect, path),
        DropKind::Gaussian => crate::gaussian_drop::commit_gaussian_drop(world, cursor, vp_rect, path),
    });
}
