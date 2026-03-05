#![allow(dead_code)]

use bevy::prelude::*;
use bevy::camera::visibility::RenderLayers;
use bevy::pbr::{Material, MaterialPipeline, MaterialPipelineKey};
use bevy::render::render_resource::{
    AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::mesh::MeshVertexBufferLayoutRef;

use crate::modal_transform::ModalTransformState;
use crate::state::{DragAxis, EditorTool, GizmoMode, GizmoState};
use crate::{GIZMO_RENDER_LAYER, GIZMO_SIZE};
use renzora_editor::{EditorSelection, HideInHierarchy};
use renzora_runtime::EditorCamera;

// ============================================================================
// GizmoMaterial — always renders on top via depth_compare: Always
// ============================================================================

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct GizmoMaterial {
    #[uniform(0)]
    pub base_color: LinearRgba,
    #[uniform(0)]
    pub emissive: LinearRgba,
}

impl Material for GizmoMaterial {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path("shaders/gizmo_material.wgsl".into())
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Opaque
    }

    fn specialize(
        _pipeline: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        _key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(ref mut depth_stencil) = descriptor.depth_stencil {
            depth_stencil.depth_compare = CompareFunction::Always;
            depth_stencil.depth_write_enabled = false;
        }
        Ok(())
    }
}

// ============================================================================
// Components
// ============================================================================

/// Marker component for gizmo mesh entities
#[derive(Component)]
pub struct GizmoMesh;

/// Identifies which axis/part this gizmo mesh represents
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GizmoPartKind {
    XAxis,
    YAxis,
    ZAxis,
    Center,
}

#[derive(Component, Clone, Copy, PartialEq, Eq)]
pub struct GizmoPart(pub GizmoPartKind);

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
    pub x_normal: Handle<GizmoMaterial>,
    pub x_highlight: Handle<GizmoMaterial>,
    pub y_normal: Handle<GizmoMaterial>,
    pub y_highlight: Handle<GizmoMaterial>,
    pub z_normal: Handle<GizmoMaterial>,
    pub z_highlight: Handle<GizmoMaterial>,
    pub center_normal: Handle<GizmoMaterial>,
    pub center_highlight: Handle<GizmoMaterial>,
    pub outline: Handle<GizmoMaterial>,
}

/// Marker for the gizmo root entity
#[derive(Component)]
pub struct GizmoRoot;

/// Distance at which gizmo_scale = 1.0
const GIZMO_SCALE_REF_DIST: f32 = 10.0;

/// System to spawn the gizmo mesh entities
pub fn setup_gizmo_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
) {
    let x_normal = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(0.9, 0.2, 0.2, 1.0),
        emissive: LinearRgba::new(0.9, 0.2, 0.2, 1.0),
    });
    let x_highlight = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
    });

    let y_normal = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(0.2, 0.9, 0.2, 1.0),
        emissive: LinearRgba::new(0.2, 0.9, 0.2, 1.0),
    });
    let y_highlight = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
    });

    let z_normal = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(0.2, 0.2, 0.9, 1.0),
        emissive: LinearRgba::new(0.2, 0.2, 0.9, 1.0),
    });
    let z_highlight = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
    });

    let center_normal = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(0.8, 0.8, 0.8, 1.0),
        emissive: LinearRgba::new(0.8, 0.8, 0.8, 1.0),
    });
    let center_highlight = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
        emissive: LinearRgba::new(1.0, 1.0, 0.3, 1.0),
    });

    let outline = materials.add(GizmoMaterial {
        base_color: LinearRgba::new(0.1, 0.1, 0.1, 1.0),
        emissive: LinearRgba::NONE,
    });

    let shaft_mesh = meshes.add(Cylinder::new(0.05, GIZMO_SIZE - 0.4));
    let cone_mesh = meshes.add(Cone { radius: 0.15, height: 0.4 });
    let cube_mesh = meshes.add(Cuboid::new(0.25, 0.25, 0.25));

    let render_layers = RenderLayers::layer(GIZMO_RENDER_LAYER);

    let gizmo_root = commands.spawn((
        Transform::default(),
        GlobalTransform::default(),
        Visibility::Hidden,
        InheritedVisibility::default(),
        ViewVisibility::default(),
        GizmoRoot,
        HideInHierarchy,
        render_layers.clone(),
    )).id();

    let spawn_part = |commands: &mut Commands, mesh: Handle<Mesh>, material: Handle<GizmoMaterial>, transform: Transform, part: GizmoPart, render_layers: &RenderLayers, gizmo_root: Entity| -> Entity {
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
            HideInHierarchy,
            render_layers.clone(),
            ChildOf(gizmo_root),
        )).id()
    };

    let x_arrow_shaft = spawn_part(
        &mut commands, shaft_mesh.clone(), x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new((GIZMO_SIZE - 0.4) / 2.0, 0.0, 0.0)),
        GizmoPart(GizmoPartKind::XAxis), &render_layers, gizmo_root,
    );

    let x_arrow_head = spawn_part(
        &mut commands, cone_mesh.clone(), x_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(GIZMO_SIZE - 0.2, 0.0, 0.0)),
        GizmoPart(GizmoPartKind::XAxis), &render_layers, gizmo_root,
    );

    let y_arrow_shaft = spawn_part(
        &mut commands, shaft_mesh.clone(), y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, (GIZMO_SIZE - 0.4) / 2.0, 0.0)),
        GizmoPart(GizmoPartKind::YAxis), &render_layers, gizmo_root,
    );

    let y_arrow_head = spawn_part(
        &mut commands, cone_mesh.clone(), y_normal.clone(),
        Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE - 0.2, 0.0)),
        GizmoPart(GizmoPartKind::YAxis), &render_layers, gizmo_root,
    );

    let z_arrow_shaft = spawn_part(
        &mut commands, shaft_mesh.clone(), z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, (GIZMO_SIZE - 0.4) / 2.0)),
        GizmoPart(GizmoPartKind::ZAxis), &render_layers, gizmo_root,
    );

    let z_arrow_head = spawn_part(
        &mut commands, cone_mesh.clone(), z_normal.clone(),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
            .with_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE - 0.2)),
        GizmoPart(GizmoPartKind::ZAxis), &render_layers, gizmo_root,
    );

    let center_cube = spawn_part(
        &mut commands, cube_mesh, center_normal.clone(),
        Transform::default(),
        GizmoPart(GizmoPartKind::Center), &render_layers, gizmo_root,
    );

    commands.insert_resource(GizmoMeshEntities {
        x_arrow_shaft, x_arrow_head,
        y_arrow_shaft, y_arrow_head,
        z_arrow_shaft, z_arrow_head,
        center_cube,
    });

    commands.insert_resource(GizmoMaterials {
        x_normal, x_highlight,
        y_normal, y_highlight,
        z_normal, z_highlight,
        center_normal, center_highlight,
        outline,
    });
}

/// System to update gizmo mesh positions based on selection
pub fn update_gizmo_mesh_transforms(
    selection: Res<EditorSelection>,
    mut gizmo_state: ResMut<GizmoState>,
    modal: Res<ModalTransformState>,
    transforms: Query<&Transform, (Without<GizmoMesh>, Without<GizmoRoot>)>,
    mut gizmo_root: Query<(&mut Transform, &mut Visibility), With<GizmoRoot>>,
    camera_query: Query<&GlobalTransform, With<EditorCamera>>,
) {
    let Ok((mut root_transform, mut root_visibility)) = gizmo_root.single_mut() else { return };

    let gizmo_tool_active = gizmo_state.tool == EditorTool::Transform;

    let selected = selection.get();
    let show_gizmos = selected.is_some()
        && gizmo_tool_active
        && gizmo_state.mode == GizmoMode::Translate
        && !modal.active;
    *root_visibility = if show_gizmos { Visibility::Visible } else { Visibility::Hidden };

    if let Some(selected) = selected {
        if let Ok(selected_transform) = transforms.get(selected) {
            let gizmo_pos = selected_transform.translation;
            root_transform.translation = gizmo_pos;

            let cam_pos: Option<Vec3> = camera_query.single().ok().map(|t| t.translation());

            if let Some(cam_translation) = cam_pos {
                let dist = (cam_translation - gizmo_pos).length().max(0.1);
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
    mut gizmo_query: Query<(&GizmoPart, &mut MeshMaterial3d<GizmoMaterial>), With<GizmoMesh>>,
) {
    let Some(materials) = gizmo_materials else { return };

    let active_axis = gizmo_state.drag_axis.or(gizmo_state.hovered_axis);

    for (part, mut material) in gizmo_query.iter_mut() {
        let (normal, highlight, is_highlighted) = match part {
            GizmoPart(GizmoPartKind::XAxis) => (
                materials.x_normal.clone(),
                materials.x_highlight.clone(),
                matches!(active_axis, Some(DragAxis::X) | Some(DragAxis::XY) | Some(DragAxis::XZ)),
            ),
            GizmoPart(GizmoPartKind::YAxis) => (
                materials.y_normal.clone(),
                materials.y_highlight.clone(),
                matches!(active_axis, Some(DragAxis::Y) | Some(DragAxis::XY) | Some(DragAxis::YZ)),
            ),
            GizmoPart(GizmoPartKind::ZAxis) => (
                materials.z_normal.clone(),
                materials.z_highlight.clone(),
                matches!(active_axis, Some(DragAxis::Z) | Some(DragAxis::XZ) | Some(DragAxis::YZ)),
            ),
            GizmoPart(GizmoPartKind::Center) => (
                materials.center_normal.clone(),
                materials.center_highlight.clone(),
                active_axis == Some(DragAxis::Free),
            ),
        };

        material.0 = if is_highlighted { highlight } else { normal };
    }
}
