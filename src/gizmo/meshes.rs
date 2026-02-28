#![allow(dead_code)]

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;

use super::modal_transform::ModalTransformState;
use super::{DragAxis, EditorTool, GizmoMode, GizmoState, GIZMO_RENDER_LAYER, GIZMO_SIZE};
use crate::core::{EditorSettings, SelectionHighlightMode, SelectionState, ViewportCamera};

/// Marker component for gizmo mesh entities
#[derive(Component)]
pub struct GizmoMesh;

/// Marker component for outline/border meshes (not affected by highlighting)
#[derive(Component)]
pub struct GizmoOutline;

/// Identifies which axis/part this gizmo mesh represents
#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub enum GizmoPart {
    XAxis,
    YAxis,
    ZAxis,
    Center,
}

/// Resource to track gizmo mesh entity handles
#[derive(Resource)]
pub struct GizmoMeshEntities {
    pub x_arrow_shaft: Entity,
    pub x_arrow_head: Entity,
    pub y_arrow_shaft: Entity,
    pub y_arrow_head: Entity,
    pub z_arrow_shaft: Entity,
    pub z_arrow_head: Entity,
    pub center_cube: Entity,
}

/// Resource to store gizmo materials for color changes
#[derive(Resource)]
pub struct GizmoMaterials {
    pub x_normal: Handle<StandardMaterial>,
    pub x_highlight: Handle<StandardMaterial>,
    pub y_normal: Handle<StandardMaterial>,
    pub y_highlight: Handle<StandardMaterial>,
    pub z_normal: Handle<StandardMaterial>,
    pub z_highlight: Handle<StandardMaterial>,
    pub center_normal: Handle<StandardMaterial>,
    pub center_highlight: Handle<StandardMaterial>,
    pub outline: Handle<StandardMaterial>,
}

/// System to spawn the gizmo mesh entities
pub fn setup_gizmo_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // Create materials - solid colors, unlit, with depth bias to render in front of scene
    let gizmo_depth_bias = -1.0;

    let x_normal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.9, 0.2, 0.2),
        emissive: LinearRgba::new(0.9, 0.2, 0.2, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });
    let x_highlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.3),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });

    let y_normal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.9, 0.2),
        emissive: LinearRgba::new(0.2, 0.9, 0.2, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });
    let y_highlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.3),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });

    let z_normal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.2, 0.2, 0.9),
        emissive: LinearRgba::new(0.2, 0.2, 0.9, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });
    let z_highlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.3),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });

    let center_normal = materials.add(StandardMaterial {
        base_color: Color::srgb(0.8, 0.8, 0.8),
        emissive: LinearRgba::new(0.8, 0.8, 0.8, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });
    let center_highlight = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 1.0, 0.3),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        unlit: true,
        depth_bias: gizmo_depth_bias,
        ..default()
    });

    // Outline material (not used but kept for resource)
    let outline = materials.add(StandardMaterial {
        base_color: Color::srgb(0.1, 0.1, 0.1),
        unlit: true,
        ..default()
    });

    // Create meshes - main geometry
    let shaft_mesh = meshes.add(Cylinder::new(0.05, GIZMO_SIZE - 0.4));
    let cone_mesh = meshes.add(Cone { radius: 0.15, height: 0.4 });
    let cube_mesh = meshes.add(Cuboid::new(0.25, 0.25, 0.25));

    let render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);

    // Spawn gizmo root entity (parent for all gizmo parts)
    let gizmo_root = commands.spawn((
        Transform::default(),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
        GizmoRoot,
        render_layers.clone(),
    )).id();

    // Helper to spawn a gizmo part as child of root
    let spawn_part = |commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<StandardMaterial>, transform: Transform, part: GizmoPart, render_layers: &RenderLayers, gizmo_root: Entity| -> Entity {
        commands.spawn((
            Mesh3d(mesh),
            MeshMaterial3d(material),
            transform,
            GlobalTransform::default(),
            Visibility::Inherited,
            InheritedVisibility::default(),
            ViewVisibility::default(),
            GizmoMesh,
            part,
            render_layers.clone(),
            ChildOf(gizmo_root),
        )).id()
    };

    // Spawn X axis arrow (no outline - cleaner look)
    let x_arrow_shaft = spawn_part(
        &mut commands,
        shaft_mesh.clone(),
        x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new((GIZMO_SIZE - 0.4) / 2.0, 0.0, 0.0)),
        GizmoPart::XAxis,
        &render_layers,
        gizmo_root,
    );

    let x_arrow_head = spawn_part(
        &mut commands,
        cone_mesh.clone(),
        x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(GIZMO_SIZE - 0.2, 0.0, 0.0)),
        GizmoPart::XAxis,
        &render_layers,
        gizmo_root,
    );

    // Spawn Y axis arrow
    let y_arrow_shaft = spawn_part(
        &mut commands,
        shaft_mesh.clone(),
        y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, (GIZMO_SIZE - 0.4) / 2.0, 0.0)),
        GizmoPart::YAxis,
        &render_layers,
        gizmo_root,
    );

    let y_arrow_head = spawn_part(
        &mut commands,
        cone_mesh.clone(),
        y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE - 0.2, 0.0)),
        GizmoPart::YAxis,
        &render_layers,
        gizmo_root,
    );

    // Spawn Z axis arrow
    let z_arrow_shaft = spawn_part(
        &mut commands,
        shaft_mesh.clone(),
        z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, (GIZMO_SIZE - 0.4) / 2.0)),
        GizmoPart::ZAxis,
        &render_layers,
        gizmo_root,
    );

    let z_arrow_head = spawn_part(
        &mut commands,
        cone_mesh.clone(),
        z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE - 0.2)),
        GizmoPart::ZAxis,
        &render_layers,
        gizmo_root,
    );

    // Spawn center cube
    let center_cube = spawn_part(
        &mut commands,
        cube_mesh,
        center_normal.clone(),
        Transform::default(),
        GizmoPart::Center,
        &render_layers,
        gizmo_root,
    );

    // Store entity handles (planes are now drawn with lines in drawing.rs)
    commands.insert_resource(GizmoMeshEntities {
        x_arrow_shaft,
        x_arrow_head,
        y_arrow_shaft,
        y_arrow_head,
        z_arrow_shaft,
        z_arrow_head,
        center_cube,
    });

    // Store materials
    commands.insert_resource(GizmoMaterials {
        x_normal,
        x_highlight,
        y_normal,
        y_highlight,
        z_normal,
        z_highlight,
        center_normal,
        center_highlight,
        outline,
    });
}

/// Marker for the gizmo root entity
#[derive(Component)]
pub struct GizmoRoot;

/// Distance at which gizmo_scale = 1.0 (gizmo appears at its base world size)
const GIZMO_SCALE_REF_DIST: f32 = 10.0;

/// System to update gizmo mesh positions based on selection
pub fn update_gizmo_mesh_transforms(
    selection: Res<SelectionState>,
    mut gizmo_state: ResMut<GizmoState>,
    modal: Res<ModalTransformState>,
    settings: Res<EditorSettings>,
    transforms: Query<&Transform, (Without<GizmoMesh>, Without<GizmoRoot>)>,
    mut gizmo_root: Query<(&mut Transform, &mut Visibility), With<GizmoRoot>>,
    camera_query: Query<&GlobalTransform, With<ViewportCamera>>,
) {
    let Ok((mut root_transform, mut root_visibility)) = gizmo_root.single_mut() else { return };

    let gizmo_tool_active = gizmo_state.tool == EditorTool::Transform
        || (gizmo_state.tool == EditorTool::Select
            && settings.selection_highlight_mode == SelectionHighlightMode::Gizmo);

    // Hide gizmo during modal transform (G/R/S mode)
    let show_gizmos = selection.selected_entity.is_some()
        && gizmo_tool_active
        && gizmo_state.mode == GizmoMode::Translate
        && !gizmo_state.collider_edit.is_active()
        && !modal.active;
    *root_visibility = if show_gizmos { Visibility::Visible } else { Visibility::Hidden };

    if let Some(selected) = selection.selected_entity {
        if let Ok(selected_transform) = transforms.get(selected) {
            let gizmo_pos = selected_transform.translation;
            root_transform.translation = gizmo_pos;

            // Scale gizmo so it occupies the same screen area regardless of camera distance
            if let Ok(cam_transform) = camera_query.single() {
                let dist = (cam_transform.translation() - gizmo_pos).length().max(0.1);
                let scale = dist / GIZMO_SCALE_REF_DIST;
                root_transform.scale = Vec3::splat(scale);
                gizmo_state.gizmo_scale = scale;
            }
        }
    }
}

/// System to update gizmo material colors based on hover state
pub fn update_gizmo_materials(
    gizmo_state: Res<GizmoState>,
    gizmo_materials: Option<Res<GizmoMaterials>>,
    mut gizmo_query: Query<(&GizmoPart, &mut MeshMaterial3d<StandardMaterial>), With<GizmoMesh>>,
) {
    let Some(materials) = gizmo_materials else { return };

    let active_axis = gizmo_state.drag_axis.or(gizmo_state.hovered_axis);

    for (part, mut material) in gizmo_query.iter_mut() {
        let (normal, highlight, is_highlighted) = match part {
            GizmoPart::XAxis => (
                materials.x_normal.clone(),
                materials.x_highlight.clone(),
                matches!(active_axis, Some(DragAxis::X) | Some(DragAxis::XY) | Some(DragAxis::XZ)),
            ),
            GizmoPart::YAxis => (
                materials.y_normal.clone(),
                materials.y_highlight.clone(),
                matches!(active_axis, Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ)),
            ),
            GizmoPart::ZAxis => (
                materials.z_normal.clone(),
                materials.z_highlight.clone(),
                matches!(active_axis, Some(DragAxis::Z) | Some(DragAxis::XZ) | Some(DragAxis::YZ)),
            ),
            GizmoPart::Center => (
                materials.center_normal.clone(),
                materials.center_highlight.clone(),
                active_axis == Some(DragAxis::Free),
            ),
        };

        material.0 = if is_highlighted { highlight } else { normal };
    }
}
