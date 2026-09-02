//! Building the transform gizmo's mesh entities — one full handle set per
//! viewport slot, each on that slot's private overlay render layer.

use bevy::camera::visibility::RenderLayers;
use bevy::prelude::*;

use renzora_editor_framework::HideInHierarchy;

use crate::material::{GizmoMaterial, GizmoMaterials};
use crate::types::{GizmoMesh, GizmoPart, GizmoRoot, GizmoSlot};
use crate::GIZMO_SIZE;

pub(crate) fn setup_gizmo_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
) {
    let mat = |m: &mut Assets<GizmoMaterial>, r: f32, g: f32, b: f32| -> Handle<GizmoMaterial> {
        m.add(GizmoMaterial {
            base_color: LinearRgba::new(r, g, b, 1.0),
            emissive: LinearRgba::new(r, g, b, 1.0),
        })
    };

    let gizmo_mats = GizmoMaterials {
        x_normal: mat(&mut materials, 1.0, 0.15, 0.15),
        x_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        y_normal: mat(&mut materials, 0.15, 1.0, 0.15),
        y_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        z_normal: mat(&mut materials, 0.2, 0.3, 1.0),
        z_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
        center_normal: mat(&mut materials, 0.9, 0.9, 0.9),
        center_highlight: mat(&mut materials, 1.0, 1.0, 0.2),
    };

    let shaft_mesh = meshes.add(Cylinder::new(0.05, GIZMO_SIZE - 0.4));
    let cone_mesh = meshes.add(Cone {
        radius: 0.15,
        height: 0.4,
    });
    let cube_mesh = meshes.add(Cuboid::new(0.25, 0.25, 0.25));
    let scale_cube_mesh = meshes.add(Cuboid::new(0.15, 0.15, 0.15));
    let half_shaft = (GIZMO_SIZE - 0.4) / 2.0;

    // (mesh, material, transform, part) for one full handle set — spawned once
    // per viewport slot below. All sets share the same mesh + material handles;
    // `update_gizmo_materials` toggles them for every set uniformly.
    let parts: [(Handle<Mesh>, Handle<GizmoMaterial>, Transform, GizmoPart); 10] = [
        // X axis (rotate cylinder to point along X)
        (
            shaft_mesh.clone(),
            gizmo_mats.x_normal.clone(),
            Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(half_shaft, 0.0, 0.0)),
            GizmoPart::XShaft,
        ),
        (
            cone_mesh.clone(),
            gizmo_mats.x_normal.clone(),
            Transform::from_rotation(Quat::from_rotation_z(-std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(GIZMO_SIZE - 0.2, 0.0, 0.0)),
            GizmoPart::XHead,
        ),
        // Y axis (cylinder default is along Y)
        (
            shaft_mesh.clone(),
            gizmo_mats.y_normal.clone(),
            Transform::from_translation(Vec3::new(0.0, half_shaft, 0.0)),
            GizmoPart::YShaft,
        ),
        (
            cone_mesh.clone(),
            gizmo_mats.y_normal.clone(),
            Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE - 0.2, 0.0)),
            GizmoPart::YHead,
        ),
        // Z axis (rotate cylinder to point along Z)
        (
            shaft_mesh.clone(),
            gizmo_mats.z_normal.clone(),
            Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(0.0, 0.0, half_shaft)),
            GizmoPart::ZShaft,
        ),
        (
            cone_mesh.clone(),
            gizmo_mats.z_normal.clone(),
            Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))
                .with_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE - 0.2)),
            GizmoPart::ZHead,
        ),
        // Scale cubes at axis tips (hidden by default, shown in Scale mode)
        (
            scale_cube_mesh.clone(),
            gizmo_mats.x_normal.clone(),
            Transform::from_translation(Vec3::new(GIZMO_SIZE, 0.0, 0.0)),
            GizmoPart::XScaleCube,
        ),
        (
            scale_cube_mesh.clone(),
            gizmo_mats.y_normal.clone(),
            Transform::from_translation(Vec3::new(0.0, GIZMO_SIZE, 0.0)),
            GizmoPart::YScaleCube,
        ),
        (
            scale_cube_mesh.clone(),
            gizmo_mats.z_normal.clone(),
            Transform::from_translation(Vec3::new(0.0, 0.0, GIZMO_SIZE)),
            GizmoPart::ZScaleCube,
        ),
        // Center cube
        (
            cube_mesh.clone(),
            gizmo_mats.center_normal.clone(),
            Transform::default(),
            GizmoPart::Center,
        ),
    ];

    // One full handle set per viewport slot, each on that slot's private overlay
    // layer so only that slot's camera draws it. `update_gizmo_transforms` then
    // sizes each set from its own camera.
    use renzora::core::viewport_types::{VIEWPORT_3D_GIZMO_LAYER_BASE, VIEWPORT_COUNT};
    for slot in 0..VIEWPORT_COUNT {
        let layer = RenderLayers::layer(VIEWPORT_3D_GIZMO_LAYER_BASE + slot);
        let root = commands
            .spawn((
                Transform::default(),
                Visibility::Hidden,
                GizmoRoot,
                GizmoSlot(slot),
                HideInHierarchy,
                layer.clone(),
            ))
            .id();
        for (mesh, mat, transform, part) in parts.iter().cloned() {
            commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(mat),
                transform,
                Visibility::Inherited,
                GizmoMesh,
                GizmoSlot(slot),
                part,
                HideInHierarchy,
                layer.clone(),
                ChildOf(root),
            ));
        }
    }

    commands.insert_resource(gizmo_mats);
}
