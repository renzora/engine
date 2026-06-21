//! In-viewport entity name labels, drawn with **Bevy 0.19's stroke-font text
//! gizmos** (`Gizmos::text`). Toggled by the Overlays → "Labels" menu item
//! (`ViewportSettings::show_labels`, off by default).
//!
//! Each named, non-chrome scene entity gets its `Name` rendered as a small
//! camera-facing billboard floating just above its origin. The stroke font is
//! vector line geometry (ASCII 32–126; other glyphs are silently skipped by
//! Bevy), so labels share the always-on-top `OverlayGizmoGroup` the light/camera
//! gizmos already use.

use bevy::prelude::*;
use renzora::core::viewport_types::ViewportSettings;
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::OverlayGizmoGroup;

pub fn draw_entity_labels(
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    settings: Res<ViewportSettings>,
    selection: Res<EditorSelection>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    // `Without<EditorCamera>` keeps this disjoint from `camera` (both read
    // `GlobalTransform`); `Without<HideInHierarchy>` skips editor chrome,
    // gizmo meshes, and light/camera icon helpers.
    labeled: Query<
        (Entity, &GlobalTransform, &Name),
        (Without<HideInHierarchy>, Without<EditorCamera>),
    >,
) {
    if !settings.show_labels {
        return;
    }
    let Ok(cam_gt) = camera.single() else {
        return;
    };
    let cam_pos = cam_gt.translation();
    let selected = selection.get();
    let max_dist = settings.label_max_distance.max(0.0);
    let base_color = Color::srgb(
        settings.label_color[0] as f32 / 255.0,
        settings.label_color[1] as f32 / 255.0,
        settings.label_color[2] as f32 / 255.0,
    );

    for (entity, gt, name) in labeled.iter() {
        let pos = gt.translation();
        let dist = cam_pos.distance(pos);
        if dist > max_dist {
            continue;
        }

        // Billboard: the stroke text lies in the local XY plane with the glyph
        // normal along local +Z, so orient +Z toward the camera while keeping
        // local +Y aligned with world up — labels stay upright and readable
        // from any viewing angle.
        let forward = (cam_pos - pos).normalize_or_zero();
        let right = Vec3::Y.cross(forward).normalize_or_zero();
        if right == Vec3::ZERO {
            // Camera directly above/below the label — degenerate basis, skip.
            continue;
        }
        let up = forward.cross(right);
        let rot = Quat::from_mat3(&Mat3::from_cols(right, up, forward));

        // Distance-proportional size keeps the on-screen height roughly
        // constant rather than shrinking to nothing far from the camera; the
        // user's `label_size` then scales that baseline.
        let size = (dist * 0.03).clamp(0.12, 1.5) * settings.label_size.max(0.01);
        // Anchor at the text's bottom-centre and lift by one line so the label
        // floats just above the entity origin instead of through it.
        let label_pos = pos + Vec3::Y * size;
        // Selected entity is always gold as a selection cue; everything else
        // uses the configured base colour.
        let color = if Some(entity) == selected {
            Color::srgb(1.0, 0.85, 0.3)
        } else {
            base_color
        };

        gizmos.text(
            Isometry3d::new(label_pos, rot),
            name.as_str(),
            size,
            Vec2::new(0.0, -0.5),
            color,
        );
    }
}
