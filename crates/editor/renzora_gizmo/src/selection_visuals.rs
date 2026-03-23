//! Selection outline (bevy_mod_outline) and bounding box gizmo.
//!
//! Two highlight modes controlled by `EditorSettings::selection_highlight_mode`:
//! - **Outline**: Mesh-based outline via bevy_mod_outline (orange stroke around selected meshes)
//! - **Gizmo**: Wireframe bounding box drawn with Bevy gizmos

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::gizmos::config::GizmoConfigStore;
use bevy_mod_outline::{OutlineVolume, OutlineStencil, OutlineMode};

use renzora_editor::{EditorSelection, EditorSettings, SelectionHighlightMode, HideInHierarchy};
use renzora_terrain::data::{TerrainData, TerrainChunkData, TerrainChunkOf};
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
    terrain_chunks: Query<(), With<TerrainChunkData>>,
    terrain_parents: Query<(), With<TerrainData>>,
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

        // Skip terrain chunks and terrain parents — they use border highlight instead
        if terrain_chunks.get(entity).is_ok() || terrain_parents.get(entity).is_ok() {
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
    terrain_chunks: Query<(), With<TerrainChunkData>>,
    terrain_parents: Query<(), With<TerrainData>>,
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

        // Skip terrain — uses dedicated border highlight
        if terrain_chunks.get(entity).is_ok() || terrain_parents.get(entity).is_ok() {
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

/// Draw yellow border around selected terrain chunks or entire terrain.
pub fn terrain_chunk_selection_system(
    selection: Res<EditorSelection>,
    modal: Res<ModalTransformState>,
    play_mode: Option<Res<renzora_core::PlayModeState>>,
    terrain_chunks: Query<(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)>,
    terrain_query: Query<(Entity, &TerrainData, &GlobalTransform)>,
    mut gizmos: Gizmos<OverlayGizmoGroup>,
) {
    let in_play = play_mode
        .as_ref()
        .map_or(false, |pm| pm.is_in_play_mode());
    if modal.active || in_play {
        return;
    }

    let all_selected = selection.get_all();
    let primary = selection.get();

    // Draw outer border when the parent terrain entity is selected
    for (terrain_entity, terrain_data, terrain_transform) in terrain_query.iter() {
        if !all_selected.contains(&terrain_entity) {
            continue;
        }

        let is_primary = primary == Some(terrain_entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        let chunks: Vec<_> = terrain_chunks
            .iter()
            .filter(|(_, _, chunk_of, _)| chunk_of.0 == terrain_entity)
            .collect();

        draw_terrain_outer_border(&mut gizmos, terrain_data, &chunks, color);
    }

    // Draw per-chunk borders for individually selected chunks
    for (entity, chunk_data, chunk_of, global_transform) in terrain_chunks.iter() {
        if !all_selected.contains(&entity) {
            continue;
        }

        // Skip if the parent terrain is already selected (full border drawn above)
        if all_selected.contains(&chunk_of.0) {
            continue;
        }

        let Ok((_, terrain_data, _)) = terrain_query.get(chunk_of.0) else {
            continue;
        };

        let is_primary = primary == Some(entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        draw_terrain_chunk_border(&mut gizmos, chunk_data, terrain_data, global_transform, color);
    }
}

/// Draw the outer border of the entire terrain (only exterior edges of boundary chunks).
fn draw_terrain_outer_border(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    terrain_data: &TerrainData,
    chunks: &[(Entity, &TerrainChunkData, &TerrainChunkOf, &GlobalTransform)],
    color: Color,
) {
    let resolution = terrain_data.chunk_resolution;
    let spacing = terrain_data.vertex_spacing();
    let height_range = terrain_data.height_range();
    let min_height = terrain_data.min_height;
    let y_offset = 0.15;

    let get_vertex_pos = |chunk: &TerrainChunkData, chunk_transform: &GlobalTransform, vx: u32, vz: u32| -> Vec3 {
        let height_normalized = chunk.get_height(vx, vz, resolution);
        let height = min_height + height_normalized * height_range;
        let pos = chunk_transform.translation();
        Vec3::new(
            pos.x + vx as f32 * spacing,
            pos.y + height + y_offset,
            pos.z + vz as f32 * spacing,
        )
    };

    for &(_, chunk_data, _, chunk_transform) in chunks {
        let cx = chunk_data.chunk_x;
        let cz = chunk_data.chunk_z;

        // Front edge (chunk_z == 0)
        if cz == 0 {
            for vx in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, vx, 0);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, vx + 1, 0);
                gizmos.line(p1, p2, color);
            }
        }

        // Back edge (chunk_z == chunks_z-1)
        if cz == terrain_data.chunks_z - 1 {
            for vx in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, vx, resolution - 1);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, vx + 1, resolution - 1);
                gizmos.line(p1, p2, color);
            }
        }

        // Left edge (chunk_x == 0)
        if cx == 0 {
            for vz in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, 0, vz);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, 0, vz + 1);
                gizmos.line(p1, p2, color);
            }
        }

        // Right edge (chunk_x == chunks_x-1)
        if cx == terrain_data.chunks_x - 1 {
            for vz in 0..(resolution - 1) {
                let p1 = get_vertex_pos(chunk_data, chunk_transform, resolution - 1, vz);
                let p2 = get_vertex_pos(chunk_data, chunk_transform, resolution - 1, vz + 1);
                gizmos.line(p1, p2, color);
            }
        }
    }
}

/// Draw a border around a single terrain chunk.
fn draw_terrain_chunk_border(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    chunk_data: &TerrainChunkData,
    terrain_data: &TerrainData,
    global_transform: &GlobalTransform,
    color: Color,
) {
    let resolution = terrain_data.chunk_resolution;
    let spacing = terrain_data.vertex_spacing();
    let height_range = terrain_data.height_range();
    let min_height = terrain_data.min_height;
    let pos = global_transform.translation();
    let y_offset = 0.15;

    let get_vertex_pos = |vx: u32, vz: u32| -> Vec3 {
        let height_normalized = chunk_data.get_height(vx, vz, resolution);
        let height = min_height + height_normalized * height_range;
        Vec3::new(
            pos.x + vx as f32 * spacing,
            pos.y + height + y_offset,
            pos.z + vz as f32 * spacing,
        )
    };

    // Front edge (z = 0)
    for vx in 0..(resolution - 1) {
        gizmos.line(get_vertex_pos(vx, 0), get_vertex_pos(vx + 1, 0), color);
    }
    // Back edge (z = max)
    for vx in 0..(resolution - 1) {
        gizmos.line(get_vertex_pos(vx, resolution - 1), get_vertex_pos(vx + 1, resolution - 1), color);
    }
    // Left edge (x = 0)
    for vz in 0..(resolution - 1) {
        gizmos.line(get_vertex_pos(0, vz), get_vertex_pos(0, vz + 1), color);
    }
    // Right edge (x = max)
    for vz in 0..(resolution - 1) {
        gizmos.line(get_vertex_pos(resolution - 1, vz), get_vertex_pos(resolution - 1, vz + 1), color);
    }
}

/// Dynamically switch OverlayGizmoGroup between on-top (render layer 1) and
/// depth-tested (render layer 0) based on `selection_boundary_on_top`.
pub fn update_selection_gizmo_depth(
    settings: Res<EditorSettings>,
    mut config_store: ResMut<GizmoConfigStore>,
) {
    if !settings.is_changed() {
        return;
    }
    let (config, _) = config_store.config_mut::<OverlayGizmoGroup>();
    if settings.selection_boundary_on_top {
        config.render_layers = RenderLayers::layer(1);
        config.depth_bias = -1.0;
    } else {
        config.render_layers = RenderLayers::layer(0);
        config.depth_bias = 0.0;
    }
}
