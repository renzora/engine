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
use renzora::core::viewport_types::{LabelScope, ViewportSettings};
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};

use crate::LabelGizmoGroup;

pub fn draw_entity_labels(
    mut gizmos: Gizmos<LabelGizmoGroup>,
    settings: Res<ViewportSettings>,
    selection: Res<EditorSelection>,
    camera: Query<&GlobalTransform, With<EditorCamera>>,
    // `Without<EditorCamera>` keeps this disjoint from `camera` (both read
    // `GlobalTransform`); `Without<HideInHierarchy>` skips editor chrome,
    // gizmo meshes, and light/camera icon helpers. `Has<Mesh3d>`/`Has<ChildOf>`
    // drive the `LabelScope` filter (mesh vs top-level).
    labeled: Query<
        (Entity, &GlobalTransform, &Name, Has<Mesh3d>, Has<ChildOf>),
        (Without<HideInHierarchy>, Without<EditorCamera>),
    >,
    // Dedicated top-level query so `LabelScope::TopLevel` reaches only root
    // entities instead of scanning the whole scene and discarding children.
    top_level: Query<
        (Entity, &GlobalTransform, &Name),
        (Without<ChildOf>, Without<HideInHierarchy>, Without<EditorCamera>),
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

    // Draw one entity's label. Shared by every scope so the scope only decides
    // *which* entities are visited, not how the label is rendered.
    let mut draw = |entity: Entity, gt: &GlobalTransform, name: &Name| {
        let pos = gt.translation();
        let dist = cam_pos.distance(pos);
        if dist > max_dist {
            return;
        }

        // Billboard: the stroke text lies in the local XY plane with the glyph
        // normal along local +Z, so orient +Z toward the camera while keeping
        // local +Y aligned with world up — labels stay upright and readable
        // from any viewing angle.
        let forward = (cam_pos - pos).normalize_or_zero();
        let right = Vec3::Y.cross(forward).normalize_or_zero();
        if right == Vec3::ZERO {
            // Camera directly above/below the label — degenerate basis, skip.
            return;
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
    };

    // Visit only the entities the scope needs — `Selected` and `TopLevel` reach
    // their entities directly instead of scanning every named entity.
    match settings.label_scope {
        LabelScope::Selected => {
            for e in selection.get_all() {
                if let Ok((entity, gt, name, _, _)) = labeled.get(e) {
                    draw(entity, gt, name);
                }
            }
        }
        LabelScope::TopLevel => {
            for (entity, gt, name) in top_level.iter() {
                draw(entity, gt, name);
            }
        }
        LabelScope::Meshes => {
            for (entity, gt, name, has_mesh, _) in labeled.iter() {
                if has_mesh {
                    draw(entity, gt, name);
                }
            }
        }
        LabelScope::All => {
            for (entity, gt, name, _, _) in labeled.iter() {
                draw(entity, gt, name);
            }
        }
    }
}
