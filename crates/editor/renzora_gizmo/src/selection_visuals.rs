//! Selection outline (bevy_mod_outline) and bounding box gizmo.
//!
//! Two highlight modes controlled by `EditorSettings::selection_highlight_mode`:
//! - **Outline**: Mesh-based outline via bevy_mod_outline (orange stroke around selected meshes)
//! - **Gizmo**: Wireframe bounding box drawn with Bevy gizmos
//!
//! Terrain types are detected via reflection (component name checks) so this
//! crate does not depend on renzora_terrain.

use bevy::prelude::*;
use bevy::camera::primitives::Aabb;
use bevy::camera::visibility::RenderLayers;
use bevy::gizmos::config::GizmoConfigStore;
use bevy_mod_outline::{OutlineVolume, OutlineStencil, OutlineMode};

use renzora::editor::{EditorSelection, EditorSettings, SelectionHighlightMode, HideInHierarchy};
use crate::modal_transform::ModalTransformState;
use crate::OverlayGizmoGroup;

/// Marker component for entities that currently have a selection outline.
#[derive(Component)]
pub struct SelectionOutline;

/// Check if an entity has a component whose type name contains the given substring.
fn has_component_by_name(world: &World, entity: Entity, name: &str) -> bool {
    let Ok(er) = world.get_entity(entity) else { return false };
    for &component_id in er.archetype().components() {
        if let Some(info) = world.components().get_info(component_id) {
            if info.name().contains(name) {
                return true;
            }
        }
    }
    false
}

/// Check if an entity is a terrain chunk (has TerrainChunkData component).
fn is_terrain_chunk(world: &World, entity: Entity) -> bool {
    has_component_by_name(world, entity, "TerrainChunkData")
}

/// Check if an entity is a terrain parent (has TerrainData component).
fn is_terrain_parent(world: &World, entity: Entity) -> bool {
    has_component_by_name(world, entity, "TerrainData")
}

/// Read a u32 field from a reflected component.
fn get_reflected_u32(world: &World, entity: Entity, type_substr: &str, field: &str) -> Option<u32> {
    let pv = renzora::core::reflection::get_reflected_field(world, entity, type_substr, field)?;
    match pv {
        renzora::core::PropertyValue::Int(v) => Some(v as u32),
        renzora::core::PropertyValue::Float(v) => Some(v as u32),
        _ => None,
    }
}

/// Read an f32 field from a reflected component.
fn get_reflected_f32(world: &World, entity: Entity, type_substr: &str, field: &str) -> Option<f32> {
    let pv = renzora::core::reflection::get_reflected_field(world, entity, type_substr, field)?;
    match pv {
        renzora::core::PropertyValue::Float(v) => Some(v),
        renzora::core::PropertyValue::Int(v) => Some(v as f32),
        _ => None,
    }
}

/// Add/remove outline components based on selection state.
pub fn update_selection_outlines(
    mut commands: Commands,
    selection: Res<EditorSelection>,
    modal: Res<ModalTransformState>,
    settings: Res<EditorSettings>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mesh_entities: Query<Entity, With<Mesh3d>>,
    children_query: Query<&Children>,
    outlined_entities: Query<Entity, With<SelectionOutline>>,
    hidden: Query<(), With<HideInHierarchy>>,
    world: &World,
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
        if is_terrain_chunk(world, entity) || is_terrain_parent(world, entity) {
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
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    mesh_aabbs: Query<(Option<&Aabb>, &GlobalTransform), With<Mesh3d>>,
    children_query: Query<&Children>,
    hidden: Query<(), With<HideInHierarchy>>,
    world: &World,
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
        if is_terrain_chunk(world, entity) || is_terrain_parent(world, entity) {
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
///
/// Uses reflection to read terrain fields (chunk_x, chunk_z, chunks_x, chunks_z,
/// chunk_resolution, chunk_size, max_height, min_height) and heightmap data via
/// `get_reflected_field`.
pub fn terrain_chunk_selection_system(
    selection: Res<EditorSelection>,
    modal: Res<ModalTransformState>,
    play_mode: Option<Res<renzora::core::PlayModeState>>,
    global_transforms: Query<&GlobalTransform>,
    children_query: Query<&Children>,
    child_of_query: Query<&ChildOf>,
    mut gizmos: Gizmos<OverlayGizmoGroup>,
    world: &World,
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
    for &terrain_entity in &all_selected {
        if !is_terrain_parent(world, terrain_entity) {
            continue;
        }

        let is_primary = primary == Some(terrain_entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        // Read terrain data fields via reflection
        let Some(chunk_resolution) = get_reflected_u32(world, terrain_entity, "TerrainData", "chunk_resolution") else { continue };
        let Some(chunks_x) = get_reflected_u32(world, terrain_entity, "TerrainData", "chunks_x") else { continue };
        let Some(chunks_z) = get_reflected_u32(world, terrain_entity, "TerrainData", "chunks_z") else { continue };
        let Some(chunk_size) = get_reflected_f32(world, terrain_entity, "TerrainData", "chunk_size") else { continue };
        let Some(max_height) = get_reflected_f32(world, terrain_entity, "TerrainData", "max_height") else { continue };
        let Some(min_height) = get_reflected_f32(world, terrain_entity, "TerrainData", "min_height") else { continue };

        let height_range = max_height - min_height;
        let spacing = chunk_size / (chunk_resolution - 1).max(1) as f32;

        // Find all chunk children
        if let Ok(children) = children_query.get(terrain_entity) {
            for child in children.iter() {
                if !is_terrain_chunk(world, child) { continue; }

                let Some(cx) = get_reflected_u32(world, child, "TerrainChunkData", "chunk_x") else { continue };
                let Some(cz) = get_reflected_u32(world, child, "TerrainChunkData", "chunk_z") else { continue };
                let Ok(chunk_gt) = global_transforms.get(child) else { continue };
                let pos = chunk_gt.translation();
                let y_offset = 0.15;

                // Draw outer border edges
                if cz == 0 {
                    draw_flat_edge(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, 0, true, color);
                }
                if cz == chunks_z - 1 {
                    draw_flat_edge(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, chunk_resolution - 1, false, color);
                }
                if cx == 0 {
                    draw_flat_edge_z(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, 0, true, color);
                }
                if cx == chunks_x - 1 {
                    draw_flat_edge_z(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, chunk_resolution - 1, false, color);
                }
            }
        }
    }

    // Draw per-chunk borders for individually selected chunks
    for &entity in &all_selected {
        if !is_terrain_chunk(world, entity) {
            continue;
        }

        // Find parent terrain
        let Ok(child_of) = child_of_query.get(entity) else { continue };
        let terrain_entity = child_of.parent();

        // Skip if the parent terrain is already selected
        if all_selected.contains(&terrain_entity) {
            continue;
        }

        if !is_terrain_parent(world, terrain_entity) {
            continue;
        }

        let Some(chunk_resolution) = get_reflected_u32(world, terrain_entity, "TerrainData", "chunk_resolution") else { continue };
        let Some(chunk_size) = get_reflected_f32(world, terrain_entity, "TerrainData", "chunk_size") else { continue };
        let Some(max_height) = get_reflected_f32(world, terrain_entity, "TerrainData", "max_height") else { continue };
        let Some(min_height) = get_reflected_f32(world, terrain_entity, "TerrainData", "min_height") else { continue };

        let height_range = max_height - min_height;
        let spacing = chunk_size / (chunk_resolution - 1).max(1) as f32;

        let Ok(chunk_gt) = global_transforms.get(entity) else { continue };
        let pos = chunk_gt.translation();
        let y_offset = 0.15;

        let is_primary = primary == Some(entity);
        let color = if is_primary {
            Color::srgb(1.0, 1.0, 0.0)
        } else {
            Color::srgba(1.0, 1.0, 0.0, 0.8)
        };

        // Draw all 4 edges of the chunk border (flat — no heightmap sampling via reflection for simplicity)
        draw_flat_edge(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, 0, true, color);
        draw_flat_edge(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, chunk_resolution - 1, false, color);
        draw_flat_edge_z(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, 0, true, color);
        draw_flat_edge_z(&mut gizmos, pos, chunk_resolution, spacing, y_offset, min_height, height_range, chunk_resolution - 1, false, color);
    }
}

/// Draw a horizontal edge line along X at a fixed Z row.
/// Since we can't efficiently read per-vertex heightmap via reflection,
/// we draw a flat line at min_height + y_offset.
fn draw_flat_edge(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    chunk_pos: Vec3,
    resolution: u32,
    spacing: f32,
    y_offset: f32,
    _min_height: f32,
    _height_range: f32,
    vz: u32,
    _is_front: bool,
    color: Color,
) {
    let y = chunk_pos.y + y_offset;
    let z = chunk_pos.z + vz as f32 * spacing;
    let x_start = chunk_pos.x;
    let x_end = chunk_pos.x + (resolution - 1) as f32 * spacing;
    gizmos.line(Vec3::new(x_start, y, z), Vec3::new(x_end, y, z), color);
}

/// Draw a horizontal edge line along Z at a fixed X column.
fn draw_flat_edge_z(
    gizmos: &mut Gizmos<OverlayGizmoGroup>,
    chunk_pos: Vec3,
    resolution: u32,
    spacing: f32,
    y_offset: f32,
    _min_height: f32,
    _height_range: f32,
    vx: u32,
    _is_left: bool,
    color: Color,
) {
    let y = chunk_pos.y + y_offset;
    let x = chunk_pos.x + vx as f32 * spacing;
    let z_start = chunk_pos.z;
    let z_end = chunk_pos.z + (resolution - 1) as f32 * spacing;
    gizmos.line(Vec3::new(x, y, z_start), Vec3::new(x, y, z_end), color);
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
