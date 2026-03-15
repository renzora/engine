//! Selection outline (bevy_mod_outline) and bounding box gizmo.
//!
//! Two highlight modes controlled by `EditorSettings::selection_highlight_mode`:
//! - **Outline**: Mesh-based outline via bevy_mod_outline (orange stroke around selected meshes)
//! - **Gizmo**: Wireframe bounding box drawn with Bevy gizmos

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy_mod_outline::{OutlineVolume, OutlineStencil, OutlineMode};

use renzora_editor::{EditorSelection, EditorSettings, SelectionHighlightMode, HideInHierarchy};
use crate::modal_transform::ModalTransformState;
use crate::OverlayGizmoGroup;

/// Marker component for entities that currently have a selection outline.
#[derive(Component)]
pub struct SelectionOutline;

/// Add/remove outline components based on selection state.
pub fn update_selection_outlines(
    mut commands: Commands,
    selection: Res<EditorSelection>,
    modal: Res<ModalTransformState>,
    settings: Res<EditorSettings>,
    play_mode: Option<Res<renzora_core::PlayModeState>>,
    mesh_entities: Query<Entity, With<Mesh3d>>,
    children_query: Query<&Children>,
    outlined_entities: Query<Entity, With<SelectionOutline>>,
    hidden: Query<(), With<HideInHierarchy>>,
) {
    let primary_color = Color::srgb(1.0, 0.5, 0.0);
    let secondary_color = Color::srgba(1.0, 0.5, 0.0, 0.8);
    let outline_width = 3.0;

    let in_play = play_mode
        .as_ref()
        .map_or(false, |pm| pm.is_in_play_mode());

    let should_show = !modal.active
        && !in_play
        && settings.selection_highlight_mode != SelectionHighlightMode::Gizmo;

    // Remove all existing outlines
    for entity in outlined_entities.iter() {
        if let Ok(mut ec) = commands.get_entity(entity) {
            ec.remove::<(OutlineVolume, OutlineStencil, OutlineMode, SelectionOutline)>();
        }
    }

    if !should_show {
        return;
    }

    let all_selected = selection.get_all();
    let primary = selection.get();

    let outline_mode = if settings.selection_boundary_on_top {
        OutlineMode::ExtrudeFlat
    } else {
        OutlineMode::ExtrudeReal
    };

    for &entity in &all_selected {
        if hidden.get(entity).is_ok() {
            continue;
        }

        let is_primary = primary == Some(entity);
        let color = if is_primary { primary_color } else { secondary_color };

        // Add outline to the entity itself if it has a mesh
        if mesh_entities.get(entity).is_ok() {
            if let Ok(mut ec) = commands.get_entity(entity) {
                ec.insert((
                    OutlineVolume {
                        visible: true,
                        width: outline_width,
                        colour: color,
                    },
                    OutlineStencil::default(),
                    outline_mode.clone(),
                    SelectionOutline,
                ));
            }
        }

        // Also add outlines to child meshes
        add_outline_to_children(
            &mut commands,
            entity,
            color,
            outline_width,
            outline_mode.clone(),
            &mesh_entities,
            &children_query,
        );
    }
}

fn add_outline_to_children(
    commands: &mut Commands,
    entity: Entity,
    color: Color,
    width: f32,
    outline_mode: OutlineMode,
    mesh_entities: &Query<Entity, With<Mesh3d>>,
    children_query: &Query<&Children>,
) {
    let Ok(children) = children_query.get(entity) else { return };
    for child in children.iter() {
        if mesh_entities.get(child).is_ok() {
            if let Ok(mut ec) = commands.get_entity(child) {
                ec.insert((
                    OutlineVolume {
                        visible: true,
                        width,
                        colour: color,
                    },
                    OutlineStencil::default(),
                    outline_mode.clone(),
                    SelectionOutline,
                ));
            }
        }
        add_outline_to_children(commands, child, color, width, outline_mode.clone(), mesh_entities, children_query);
    }
}

/// Draw wireframe bounding box around selected entities when in Gizmo highlight mode.
pub fn draw_selection_bounding_box(
    selection: Res<EditorSelection>,
    modal: Res<ModalTransformState>,
    settings: Res<EditorSettings>,
    play_mode: Option<Res<renzora_core::PlayModeState>>,
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    mesh_aabbs: Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_query: Query<&Children>,
    hidden: Query<(), With<HideInHierarchy>>,
) {
    if modal.active {
        return;
    }

    let in_play = play_mode
        .as_ref()
        .map_or(false, |pm| pm.is_in_play_mode());
    if in_play {
        return;
    }

    if settings.selection_highlight_mode != SelectionHighlightMode::Gizmo {
        return;
    }

    let primary_color = Color::srgb(1.0, 0.5, 0.0);
    let secondary_color = Color::srgba(1.0, 0.5, 0.0, 0.8);

    let all_selected = selection.get_all();
    let primary = selection.get();

    for &entity in &all_selected {
        if hidden.get(entity).is_ok() {
            continue;
        }

        let is_primary = primary == Some(entity);
        let color = if is_primary { primary_color } else { secondary_color };

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        collect_world_aabb(entity, &mesh_aabbs, &children_query, &mut min, &mut max);

        if min.x <= max.x {
            let center = (min + max) * 0.5;
            let size = max - min;
            draw_wireframe_box(&mut gizmos, center, size, color);
        }
    }
}

/// Recursively expand min/max with world-space corners of every mesh in the hierarchy.
fn collect_world_aabb(
    entity: Entity,
    mesh_aabbs: &Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_query: &Query<&Children>,
    min: &mut Vec3,
    max: &mut Vec3,
) {
    if let Ok((Some(aabb), global_transform)) = mesh_aabbs.get(entity) {
        let c = Vec3::from(aabb.center);
        let h = Vec3::from(aabb.half_extents);
        for sx in [-1.0_f32, 1.0] {
            for sy in [-1.0_f32, 1.0] {
                for sz in [-1.0_f32, 1.0] {
                    let corner = global_transform.transform_point(c + h * Vec3::new(sx, sy, sz));
                    *min = min.min(corner);
                    *max = max.max(corner);
                }
            }
        }
    }
    if let Ok(children) = children_query.get(entity) {
        for child in children.iter() {
            collect_world_aabb(child, mesh_aabbs, children_query, min, max);
        }
    }
}

/// Draw a wireframe box from center + size using 12 edge lines.
fn draw_wireframe_box(gizmos: &mut Gizmos<OverlayGizmoGroup>, center: Vec3, size: Vec3, color: Color) {
    let h = size * 0.5;

    let corners = [
        center + Vec3::new(-h.x, -h.y, -h.z),
        center + Vec3::new( h.x, -h.y, -h.z),
        center + Vec3::new( h.x,  h.y, -h.z),
        center + Vec3::new(-h.x,  h.y, -h.z),
        center + Vec3::new(-h.x, -h.y,  h.z),
        center + Vec3::new( h.x, -h.y,  h.z),
        center + Vec3::new( h.x,  h.y,  h.z),
        center + Vec3::new(-h.x,  h.y,  h.z),
    ];

    // Bottom face
    gizmos.line(corners[0], corners[1], color);
    gizmos.line(corners[1], corners[2], color);
    gizmos.line(corners[2], corners[3], color);
    gizmos.line(corners[3], corners[0], color);
    // Top face
    gizmos.line(corners[4], corners[5], color);
    gizmos.line(corners[5], corners[6], color);
    gizmos.line(corners[6], corners[7], color);
    gizmos.line(corners[7], corners[4], color);
    // Verticals
    gizmos.line(corners[0], corners[4], color);
    gizmos.line(corners[1], corners[5], color);
    gizmos.line(corners[2], corners[6], color);
    gizmos.line(corners[3], corners[7], color);
}
