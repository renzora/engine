//! Interactive resize + move handles for the selected collider when
//! `ColliderEditMode.active` is true.
//!
//! Handles are real `Mesh3d` sphere entities (spawned fresh each frame, like
//! the skeleton gizmo) so they pick via `MeshRayCast` and read as solid in
//! the viewport.

use bevy::prelude::*;
use bevy::input::mouse::MouseMotion;
use bevy::camera::visibility::RenderLayers;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility};
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora_editor::{EditorCamera, EditorSelection, HideInHierarchy};
use renzora_physics::{ColliderEditMode, CollisionShapeData, CollisionShapeType};

use crate::GizmoMaterial;

const HANDLE_SCREEN_SIZE: f32 = 12.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum HandleKind {
    Resize(ResizeAxis),
    Offset(LinearAxis),
    /// Free-plane drag — moves `offset` along the camera's right/up axes.
    BodyMove,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ResizeAxis {
    BoxX(Sign),
    BoxY(Sign),
    BoxZ(Sign),
    Radius,
    Height(Sign),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LinearAxis { X, Y, Z }

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sign { Pos, Neg }
impl Sign { fn f(self) -> f32 { match self { Sign::Pos => 1.0, Sign::Neg => -1.0 } } }

/// Marker on each spawned sphere so we can despawn all at start of frame.
#[derive(Component)]
pub struct ColliderHandleMesh {
    pub handle: HandleKind,
}

#[derive(Resource)]
pub struct ColliderHandleAssets {
    pub mesh: Handle<Mesh>,
    pub mat_x: Handle<GizmoMaterial>,
    pub mat_y: Handle<GizmoMaterial>,
    pub mat_z: Handle<GizmoMaterial>,
    pub mat_resize_x: Handle<GizmoMaterial>,
    pub mat_resize_y: Handle<GizmoMaterial>,
    pub mat_resize_z: Handle<GizmoMaterial>,
    pub mat_hover: Handle<GizmoMaterial>,
    pub mat_body: Handle<GizmoMaterial>,
}

#[derive(Resource, Default)]
pub struct ColliderHandleState {
    pub hovered: Option<HandleKind>,
    pub dragging: Option<DragInfo>,
}

pub struct DragInfo {
    pub handle: HandleKind,
    pub start_shape: CollisionShapeData,
    pub accumulated: f32,
    pub accumulated_2d: Vec2,
}

fn handle_world(
    handle: HandleKind,
    shape: &CollisionShapeData,
    center: Vec3,
    rot: Quat,
) -> (Vec3, Vec3) {
    let axis_of = |a: LinearAxis| match a {
        LinearAxis::X => rot * Vec3::X,
        LinearAxis::Y => rot * Vec3::Y,
        LinearAxis::Z => rot * Vec3::Z,
    };
    if matches!(handle, HandleKind::BodyMove) {
        return (center, Vec3::Y);
    }
    match handle {
        HandleKind::Offset(a) => {
            let dir = axis_of(a);
            // Push offset arrows well past the resize handles so they never overlap.
            let len = max_extent(shape) + 0.35;
            (center + dir * len, dir)
        }
        HandleKind::Resize(ResizeAxis::BoxX(s)) => {
            let d = rot * Vec3::X * s.f();
            (center + d * shape.half_extents.x, d)
        }
        HandleKind::Resize(ResizeAxis::BoxY(s)) => {
            let d = rot * Vec3::Y * s.f();
            (center + d * shape.half_extents.y, d)
        }
        HandleKind::Resize(ResizeAxis::BoxZ(s)) => {
            let d = rot * Vec3::Z * s.f();
            (center + d * shape.half_extents.z, d)
        }
        HandleKind::Resize(ResizeAxis::Radius) => {
            let d = rot * Vec3::X;
            (center + d * shape.radius, d)
        }
        HandleKind::Resize(ResizeAxis::Height(s)) => {
            let d = rot * Vec3::Y * s.f();
            let y_extent = match shape.shape_type {
                CollisionShapeType::Capsule => shape.half_height + shape.radius,
                _ => shape.half_height,
            };
            (center + d * y_extent, d)
        }
        HandleKind::BodyMove => unreachable!(),
    }
}

fn max_extent(shape: &CollisionShapeData) -> f32 {
    match shape.shape_type {
        CollisionShapeType::Box => shape.half_extents.max_element(),
        CollisionShapeType::Sphere => shape.radius,
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => shape.radius.max(shape.half_height + shape.radius),
        CollisionShapeType::Mesh => 0.5,
    }
}

fn handles_for(shape_type: CollisionShapeType) -> Vec<HandleKind> {
    let mut out = vec![
        HandleKind::BodyMove,
        HandleKind::Offset(LinearAxis::X),
        HandleKind::Offset(LinearAxis::Y),
        HandleKind::Offset(LinearAxis::Z),
    ];
    match shape_type {
        CollisionShapeType::Box => out.extend([
            HandleKind::Resize(ResizeAxis::BoxX(Sign::Pos)),
            HandleKind::Resize(ResizeAxis::BoxX(Sign::Neg)),
            HandleKind::Resize(ResizeAxis::BoxY(Sign::Pos)),
            HandleKind::Resize(ResizeAxis::BoxY(Sign::Neg)),
            HandleKind::Resize(ResizeAxis::BoxZ(Sign::Pos)),
            HandleKind::Resize(ResizeAxis::BoxZ(Sign::Neg)),
        ]),
        CollisionShapeType::Sphere => out.push(HandleKind::Resize(ResizeAxis::Radius)),
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => out.extend([
            HandleKind::Resize(ResizeAxis::Radius),
            HandleKind::Resize(ResizeAxis::Height(Sign::Pos)),
            HandleKind::Resize(ResizeAxis::Height(Sign::Neg)),
        ]),
        CollisionShapeType::Mesh => {}
    }
    out
}

fn material_for(handle: HandleKind, hovered: bool, a: &ColliderHandleAssets) -> Handle<GizmoMaterial> {
    if hovered { return a.mat_hover.clone(); }
    match handle {
        HandleKind::BodyMove => a.mat_body.clone(),
        HandleKind::Offset(LinearAxis::X) => a.mat_x.clone(),
        HandleKind::Offset(LinearAxis::Y) => a.mat_y.clone(),
        HandleKind::Offset(LinearAxis::Z) => a.mat_z.clone(),
        HandleKind::Resize(ResizeAxis::BoxX(_)) | HandleKind::Resize(ResizeAxis::Radius) => a.mat_resize_x.clone(),
        HandleKind::Resize(ResizeAxis::BoxY(_)) | HandleKind::Resize(ResizeAxis::Height(_)) => a.mat_resize_y.clone(),
        HandleKind::Resize(ResizeAxis::BoxZ(_)) => a.mat_resize_z.clone(),
    }
}

fn make_uv_sphere(radius: f32, lat: u32, lon: u32) -> Mesh {
    use bevy::mesh::{Indices, PrimitiveTopology};
    use bevy::asset::RenderAssetUsages;
    let mut positions = Vec::with_capacity(((lat + 1) * (lon + 1)) as usize);
    let mut normals = Vec::with_capacity(positions.capacity());
    let mut uvs = Vec::with_capacity(positions.capacity());
    for i in 0..=lat {
        let v = i as f32 / lat as f32;
        let phi = std::f32::consts::PI * v;
        for j in 0..=lon {
            let u = j as f32 / lon as f32;
            let theta = std::f32::consts::TAU * u;
            let x = phi.sin() * theta.cos();
            let y = phi.cos();
            let z = phi.sin() * theta.sin();
            positions.push([x * radius, y * radius, z * radius]);
            normals.push([x, y, z]);
            uvs.push([u, v]);
        }
    }
    let mut indices = Vec::new();
    for i in 0..lat {
        for j in 0..lon {
            let a = i * (lon + 1) + j;
            let b = a + lon + 1;
            indices.extend_from_slice(&[a, b, a + 1, b, b + 1, a + 1]);
        }
    }
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}

fn ensure_assets(
    commands: &mut Commands,
    assets: Option<&ColliderHandleAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<GizmoMaterial>,
) {
    if assets.is_some() { return; }
    let mk = |m: &mut Assets<GizmoMaterial>, r: f32, g: f32, b: f32| m.add(GizmoMaterial {
        base_color: LinearRgba::new(r, g, b, 1.0),
        emissive: LinearRgba::new(r, g, b, 1.0),
    });
    commands.insert_resource(ColliderHandleAssets {
        mesh: meshes.add(make_uv_sphere(1.0, 10, 14)),
        mat_x: mk(materials, 1.0, 0.25, 0.25),
        mat_y: mk(materials, 0.3, 0.95, 0.3),
        mat_z: mk(materials, 0.25, 0.45, 1.0),
        mat_resize_x: mk(materials, 1.0, 0.55, 0.55),
        mat_resize_y: mk(materials, 0.55, 1.0, 0.55),
        mat_resize_z: mk(materials, 0.55, 0.7, 1.0),
        mat_hover: mk(materials, 1.0, 0.85, 0.15),
        mat_body: mk(materials, 0.85, 0.85, 0.85),
    });
}

pub fn spawn_handle_meshes(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<GizmoMaterial>>,
    assets: Option<Res<ColliderHandleAssets>>,
    edit_mode: Res<ColliderEditMode>,
    selection: Res<EditorSelection>,
    state: Res<ColliderHandleState>,
    camera_q: Query<(&GlobalTransform, &Projection), With<EditorCamera>>,
    viewport: Option<Res<ViewportState>>,
    shapes: Query<(&CollisionShapeData, &GlobalTransform)>,
    existing: Query<Entity, With<ColliderHandleMesh>>,
) {
    // Clear previous frame's handles.
    for e in &existing { commands.entity(e).despawn(); }

    if !edit_mode.active { return; }
    let Some(selected) = selection.get() else { return };
    let Ok((shape, gt)) = shapes.get(selected) else { return };
    let Ok((cam_gt, projection)) = camera_q.single() else { return };
    let Some(vp) = viewport.as_deref() else { return };

    ensure_assets(&mut commands, assets.as_deref(), &mut meshes, &mut materials);
    let Some(assets) = assets.as_deref() else { return };

    let (_scale, rot, trans) = gt.to_scale_rotation_translation();
    let center = trans + rot * shape.offset;

    for handle in handles_for(shape.shape_type) {
        let (pos, _axis) = handle_world(handle, shape, center, rot);
        let distance = (cam_gt.translation() - pos).length().max(0.01);
        let size_pixels = if matches!(handle, HandleKind::BodyMove) {
            HANDLE_SCREEN_SIZE * 1.6
        } else {
            HANDLE_SCREEN_SIZE
        };
        let world_size = screen_to_world(size_pixels, distance, projection, vp);
        let hovered = state.hovered == Some(handle) || state.dragging.as_ref().map(|d| d.handle) == Some(handle);

        commands.spawn((
            Name::new("ColliderHandle"),
            Mesh3d(assets.mesh.clone()),
            MeshMaterial3d(material_for(handle, hovered, assets)),
            Transform {
                translation: pos,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(world_size * 0.5),
            },
            Visibility::default(),
            RenderLayers::layer(0),
            HideInHierarchy,
            ColliderHandleMesh { handle },
        ));
    }
}

fn screen_to_world(pixels: f32, distance: f32, projection: &Projection, viewport: &ViewportState) -> f32 {
    match projection {
        Projection::Perspective(p) => distance * (p.fov * 0.5).tan() * 2.0 * pixels / viewport.screen_size.y,
        Projection::Orthographic(o) => o.area.height() * pixels / viewport.screen_size.y,
        _ => 0.1,
    }
}

pub fn pick_and_drag_handles(
    mut state: ResMut<ColliderHandleState>,
    edit_mode: Res<ColliderEditMode>,
    selection: Res<EditorSelection>,
    camera_q: Query<(&Camera, &GlobalTransform, &Projection), With<EditorCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    viewport: Option<Res<ViewportState>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut mouse_motion: MessageReader<MouseMotion>,
    mut shapes: Query<(&mut CollisionShapeData, &GlobalTransform)>,
    mut ray_cast: MeshRayCast,
    handle_meshes: Query<&ColliderHandleMesh>,
) {
    if !edit_mode.active {
        state.hovered = None;
        state.dragging = None;
        mouse_motion.clear();
        return;
    }
    let Some(selected) = selection.get() else {
        state.hovered = None; state.dragging = None; mouse_motion.clear(); return;
    };
    let Ok((mut shape, gt)) = shapes.get_mut(selected) else { mouse_motion.clear(); return };
    let Ok((camera, cam_gt, projection)) = camera_q.single() else { mouse_motion.clear(); return };
    let Ok(window) = windows.single() else { mouse_motion.clear(); return };
    let Some(cursor) = window.cursor_position() else { mouse_motion.clear(); return };
    let Some(vp) = viewport.as_deref() else { mouse_motion.clear(); return };

    if state.dragging.is_some() && !mouse.pressed(MouseButton::Left) {
        state.dragging = None;
        mouse_motion.clear();
        return;
    }

    let (_sc, rot, trans) = gt.to_scale_rotation_translation();
    let center = trans + rot * shape.offset;

    // Hover via mesh raycast — only done when not dragging.
    if state.dragging.is_none() {
        let cursor_in_vp = Vec2::new(
            (cursor.x - vp.screen_position.x).max(0.0),
            (cursor.y - vp.screen_position.y).max(0.0),
        );
        let viewport_px = Vec2::new(
            cursor_in_vp.x / vp.screen_size.x * vp.current_size.x as f32,
            cursor_in_vp.y / vp.screen_size.y * vp.current_size.y as f32,
        );
        state.hovered = if let Ok(ray) = camera.viewport_to_world(cam_gt, viewport_px) {
            let settings = MeshRayCastSettings {
                visibility: RayCastVisibility::VisibleInView,
                filter: &|e| handle_meshes.contains(e),
                early_exit_test: &|_| true,
            };
            ray_cast.cast_ray(ray, &settings)
                .first()
                .and_then(|(e, _hit)| handle_meshes.get(*e).ok())
                .map(|h| h.handle)
        } else { None };
    }

    if mouse.just_pressed(MouseButton::Left) {
        if let Some(h) = state.hovered {
            state.dragging = Some(DragInfo {
                handle: h,
                start_shape: shape.clone(),
                accumulated: 0.0,
                accumulated_2d: Vec2::ZERO,
            });
            mouse_motion.clear();
            return;
        }
    }

    let Some(drag) = state.dragging.as_mut() else { mouse_motion.clear(); return };
    let mut total_delta = Vec2::ZERO;
    for ev in mouse_motion.read() { total_delta += ev.delta; }
    if total_delta.length_squared() < 1e-6 { return; }

    let distance = (cam_gt.translation() - center).length().max(0.01);
    let scale = match projection {
        Projection::Perspective(p) => distance * (p.fov * 0.5).tan() * 2.0 / vp.screen_size.y,
        Projection::Orthographic(o) => o.area.height() / vp.screen_size.y,
        _ => return,
    };
    let cam_right = cam_gt.right().as_vec3();
    let cam_up = cam_gt.up().as_vec3();

    if matches!(drag.handle, HandleKind::BodyMove) {
        drag.accumulated_2d += total_delta;
        let world_delta = cam_right * (drag.accumulated_2d.x * scale)
            + cam_up * (-drag.accumulated_2d.y * scale);
        // Convert world-space offset delta into entity-local space.
        let local_delta = rot.inverse() * world_delta;
        shape.offset = drag.start_shape.offset + local_delta;
        return;
    }

    let (_, axis_dir) = handle_world(drag.handle, &drag.start_shape, center, rot);
    let screen_axis = Vec2::new(axis_dir.dot(cam_right), -axis_dir.dot(cam_up));
    let len = screen_axis.length();
    if len < 1e-4 { return; }
    let delta_along = total_delta.dot(screen_axis / len) * scale;
    drag.accumulated += delta_along;

    apply_drag(&mut shape, drag.handle, &drag.start_shape, drag.accumulated);
}

fn apply_drag(shape: &mut CollisionShapeData, handle: HandleKind, start: &CollisionShapeData, delta: f32) {
    match handle {
        HandleKind::Offset(LinearAxis::X) => shape.offset.x = start.offset.x + delta,
        HandleKind::Offset(LinearAxis::Y) => shape.offset.y = start.offset.y + delta,
        HandleKind::Offset(LinearAxis::Z) => shape.offset.z = start.offset.z + delta,
        HandleKind::Resize(ResizeAxis::BoxX(_)) => shape.half_extents.x = (start.half_extents.x + delta).max(0.01),
        HandleKind::Resize(ResizeAxis::BoxY(_)) => shape.half_extents.y = (start.half_extents.y + delta).max(0.01),
        HandleKind::Resize(ResizeAxis::BoxZ(_)) => shape.half_extents.z = (start.half_extents.z + delta).max(0.01),
        HandleKind::Resize(ResizeAxis::Radius) => shape.radius = (start.radius + delta).max(0.01),
        HandleKind::Resize(ResizeAxis::Height(_)) => shape.half_height = (start.half_height + delta).max(0.01),
        HandleKind::BodyMove => {}
    }
}
