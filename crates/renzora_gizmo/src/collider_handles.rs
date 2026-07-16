//! Interactive resize + move handles for the selected collider when
//! `ColliderEditMode.active` is true.
//!
//! Handles are real `Mesh3d` entities (spawned fresh each frame, like the
//! skeleton gizmo) so they pick via `MeshRayCast` and read as solid in the
//! viewport. The visual vocabulary is deliberately kept legible rather than a
//! cloud of same-shape balls:
//!
//! - **Resize** handles are small white cube grips sitting on each face/extent
//!   of the shape. Each grip rides a thin green **axis spoke** running out from
//!   the centre, so the set reads as a coordinate manipulator with grab points
//!   on its axes rather than a cloud of disconnected dots. The spokes stay thin
//!   so they never occlude the mesh being edited.
//! - **Offset** handles are axis-coloured cubes (red X, green Y, blue Z) sitting
//!   out past the shape on a matching thin spoke — the same grip shape as resize,
//!   distinguished by colour rather than by being a different primitive.
//! - **Body move** is a single small neutral sphere at the centre for
//!   free screen-plane dragging.
//! - Whatever is hovered/dragged turns yellow.
//!
//! Every piece of a handle carries the same [`ColliderHandleMesh`] marker, so a
//! multi-part arrow (shaft + cone) still resolves to one logical handle when
//! picked — the pick/drag logic keys purely off that marker.

use bevy::camera::visibility::RenderLayers;
use bevy::input::mouse::MouseMotion;
use bevy::picking::mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings, RayCastVisibility};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use renzora::core::viewport_types::ViewportState;
use renzora_editor_framework::{EditorCamera, EditorSelection, HideInHierarchy};
use renzora_physics::{ColliderEditMode, CollisionShapeData, CollisionShapeType};

use crate::GizmoMaterial;

/// Reference on-screen size (px) of a resize cube / centre handle. Arrow
/// dimensions are derived from this so the whole gizmo stays a constant size in
/// the viewport regardless of camera distance.
const HANDLE_SCREEN_SIZE: f32 = 11.0;

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
pub enum LinearAxis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sign {
    Pos,
    Neg,
}
impl Sign {
    fn f(self) -> f32 {
        match self {
            Sign::Pos => 1.0,
            Sign::Neg => -1.0,
        }
    }
}

/// Marker on each spawned sphere so we can despawn all at start of frame.
#[derive(Component)]
pub struct ColliderHandleMesh {
    pub handle: HandleKind,
}

#[derive(Resource)]
pub struct ColliderHandleAssets {
    /// Unit sphere (radius 1) — the centre body-move handle.
    pub sphere: Handle<Mesh>,
    /// Unit cube (1×1×1, origin-centred) — the resize grips.
    pub cube: Handle<Mesh>,
    /// Unit cylinder (radius 1, height 1, along +Y) — the thin axis spokes that
    /// carry both the resize grips and the offset cubes (same mesh, different
    /// scale).
    pub shaft: Handle<Mesh>,
    /// One uniform colour for every resize grip cube.
    pub mat_face: Handle<GizmoMaterial>,
    /// Green axis spokes tying the resize grips to the centre.
    pub mat_spoke: Handle<GizmoMaterial>,
    pub mat_x: Handle<GizmoMaterial>,
    pub mat_y: Handle<GizmoMaterial>,
    pub mat_z: Handle<GizmoMaterial>,
    pub mat_body: Handle<GizmoMaterial>,
    pub mat_hover: Handle<GizmoMaterial>,
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
        CollisionShapeType::Capsule | CollisionShapeType::Cylinder => {
            shape.radius.max(shape.half_height + shape.radius)
        }
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

fn ensure_assets(
    commands: &mut Commands,
    assets: Option<&ColliderHandleAssets>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<GizmoMaterial>,
) {
    if assets.is_some() {
        return;
    }
    let mk = |m: &mut Assets<GizmoMaterial>, r: f32, g: f32, b: f32| {
        m.add(GizmoMaterial {
            base_color: LinearRgba::new(r, g, b, 1.0),
            emissive: LinearRgba::new(r, g, b, 1.0),
        })
    };
    commands.insert_resource(ColliderHandleAssets {
        sphere: meshes.add(Sphere::new(1.0)),
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        shaft: meshes.add(Cylinder::new(1.0, 1.0)),
        mat_face: mk(materials, 0.92, 0.92, 0.96),
        // Green spokes echo the collider outline so the axes read as part of the
        // same gizmo.
        mat_spoke: mk(materials, 0.30, 0.85, 0.42),
        // Axis colours match the translate gizmo (x=red, y=green, z=blue).
        mat_x: mk(materials, 1.0, 0.15, 0.15),
        mat_y: mk(materials, 0.15, 1.0, 0.15),
        mat_z: mk(materials, 0.2, 0.3, 1.0),
        mat_body: mk(materials, 0.82, 0.82, 0.88),
        mat_hover: mk(materials, 1.0, 0.85, 0.15),
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
    for e in &existing {
        commands.entity(e).despawn();
    }

    if !edit_mode.active {
        return;
    }
    let Some(selected) = selection.get() else {
        return;
    };
    let Ok((shape, gt)) = shapes.get(selected) else {
        return;
    };
    let Ok((cam_gt, projection)) = camera_q.single() else {
        return;
    };
    let Some(vp) = viewport.as_deref() else {
        return;
    };

    ensure_assets(
        &mut commands,
        assets.as_deref(),
        &mut meshes,
        &mut materials,
    );
    let Some(assets) = assets.as_deref() else {
        return;
    };

    let (_scale, rot, trans) = gt.to_scale_rotation_translation();
    let center = trans + rot * shape.offset;
    let cam = cam_gt.translation();

    for handle in handles_for(shape.shape_type) {
        let (pos, axis) = handle_world(handle, shape, center, rot);
        let hovered = state.hovered == Some(handle)
            || state.dragging.as_ref().map(|d| d.handle) == Some(handle);

        // Size point-scaled handles (arrows, round grips) for a constant on-screen
        // footprint. Face panels are NOT scaled — they *are* the surface, so they
        // track the collider's real dimensions.
        let distance = (cam - pos).length().max(0.01);
        let unit = screen_to_world(HANDLE_SCREEN_SIZE, distance, projection, vp);

        // Each handle contributes one or more mesh pieces, all tagged with the
        // same `handle` so a picked shaft/cone still resolves to the arrow.
        let mut pieces: Vec<(Handle<Mesh>, Handle<GizmoMaterial>, Transform)> =
            Vec::with_capacity(2);

        match handle {
            // Resize grip: a small white cube on the face/extent, riding a thin
            // green spoke from the centre so it reads as a point on an axis
            // rather than a floating dot. The spoke is kept hair-thin so it never
            // hides the mesh underneath.
            HandleKind::Resize(_) => {
                // Spoke: a thin cylinder from the centre out to the grip. Skip it
                // if the grip sits on the centre (zero-length) to avoid a NaN
                // orientation.
                let span = pos - center;
                let len = span.length();
                if len > 1e-4 {
                    let dir = span / len;
                    let orient = Quat::from_rotation_arc(Vec3::Y, dir);
                    pieces.push((
                        assets.shaft.clone(),
                        assets.mat_spoke.clone(),
                        Transform {
                            translation: center + dir * (len * 0.5),
                            rotation: orient,
                            scale: Vec3::new(unit * 0.05, len, unit * 0.05),
                        },
                    ));
                }
                let mat = if hovered {
                    assets.mat_hover.clone()
                } else {
                    assets.mat_face.clone()
                };
                pieces.push((
                    assets.cube.clone(),
                    mat,
                    Transform {
                        translation: pos,
                        rotation: rot,
                        scale: Vec3::splat(unit),
                    },
                ));
            }
            // Free screen-plane move: a small neutral sphere at the centre.
            HandleKind::BodyMove => {
                let mat = if hovered {
                    assets.mat_hover.clone()
                } else {
                    assets.mat_body.clone()
                };
                pieces.push((
                    assets.sphere.clone(),
                    mat,
                    Transform {
                        translation: center,
                        rotation: Quat::IDENTITY,
                        scale: Vec3::splat(unit * 0.6),
                    },
                ));
            }
            // Offset: an axis-coloured cube sitting out past the shape on a thin
            // spoke of the same colour, so it reads as the same kind of grip as
            // the resize handles but clearly distinguished by axis colour.
            HandleKind::Offset(a) => {
                let dir = axis.normalize_or_zero();
                if dir == Vec3::ZERO {
                    continue;
                }
                let orient = Quat::from_rotation_arc(Vec3::Y, dir);
                // Start just clear of the shape surface along this axis; the cube
                // sits a short stalk further out.
                let base = center + dir * (max_extent(shape) + unit * 0.6);
                let stalk = unit * 2.2;
                let cube_pos = base + dir * stalk;
                let spoke_mat = match a {
                    LinearAxis::X => assets.mat_x.clone(),
                    LinearAxis::Y => assets.mat_y.clone(),
                    LinearAxis::Z => assets.mat_z.clone(),
                };
                let cube_mat = if hovered {
                    assets.mat_hover.clone()
                } else {
                    spoke_mat.clone()
                };

                pieces.push((
                    assets.shaft.clone(),
                    spoke_mat,
                    Transform {
                        translation: base + dir * (stalk * 0.5),
                        rotation: orient,
                        scale: Vec3::new(unit * 0.05, stalk, unit * 0.05),
                    },
                ));
                pieces.push((
                    assets.cube.clone(),
                    cube_mat,
                    Transform {
                        translation: cube_pos,
                        rotation: rot,
                        scale: Vec3::splat(unit),
                    },
                ));
            }
        }

        for (mesh, material, transform) in pieces {
            commands.spawn((
                Name::new("ColliderHandle"),
                Mesh3d(mesh),
                MeshMaterial3d(material),
                transform,
                Visibility::default(),
                RenderLayers::layer(0),
                HideInHierarchy,
                ColliderHandleMesh { handle },
            ));
        }
    }
}

fn screen_to_world(
    pixels: f32,
    distance: f32,
    projection: &Projection,
    viewport: &ViewportState,
) -> f32 {
    match projection {
        Projection::Perspective(p) => {
            distance * (p.fov * 0.5).tan() * 2.0 * pixels / viewport.screen_size.y
        }
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
        state.hovered = None;
        state.dragging = None;
        mouse_motion.clear();
        return;
    };
    let Ok((mut shape, gt)) = shapes.get_mut(selected) else {
        mouse_motion.clear();
        return;
    };
    let Ok((camera, cam_gt, projection)) = camera_q.single() else {
        mouse_motion.clear();
        return;
    };
    let Ok(window) = windows.single() else {
        mouse_motion.clear();
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        mouse_motion.clear();
        return;
    };
    let Some(vp) = viewport.as_deref() else {
        mouse_motion.clear();
        return;
    };

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
            ray_cast
                .cast_ray(ray, &settings)
                .first()
                .and_then(|(e, _hit)| handle_meshes.get(*e).ok())
                .map(|h| h.handle)
        } else {
            None
        };
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

    let Some(drag) = state.dragging.as_mut() else {
        mouse_motion.clear();
        return;
    };
    let mut total_delta = Vec2::ZERO;
    for ev in mouse_motion.read() {
        total_delta += ev.delta;
    }
    if total_delta.length_squared() < 1e-6 {
        return;
    }

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
        let world_delta =
            cam_right * (drag.accumulated_2d.x * scale) + cam_up * (-drag.accumulated_2d.y * scale);
        // Convert world-space offset delta into entity-local space.
        let local_delta = rot.inverse() * world_delta;
        shape.offset = drag.start_shape.offset + local_delta;
        return;
    }

    let (_, axis_dir) = handle_world(drag.handle, &drag.start_shape, center, rot);
    let screen_axis = Vec2::new(axis_dir.dot(cam_right), -axis_dir.dot(cam_up));
    let len = screen_axis.length();
    if len < 1e-4 {
        return;
    }
    let delta_along = total_delta.dot(screen_axis / len) * scale;
    drag.accumulated += delta_along;

    apply_drag(&mut shape, drag.handle, &drag.start_shape, drag.accumulated);
}

fn apply_drag(
    shape: &mut CollisionShapeData,
    handle: HandleKind,
    start: &CollisionShapeData,
    delta: f32,
) {
    match handle {
        HandleKind::Offset(LinearAxis::X) => shape.offset.x = start.offset.x + delta,
        HandleKind::Offset(LinearAxis::Y) => shape.offset.y = start.offset.y + delta,
        HandleKind::Offset(LinearAxis::Z) => shape.offset.z = start.offset.z + delta,
        HandleKind::Resize(ResizeAxis::BoxX(_)) => {
            shape.half_extents.x = (start.half_extents.x + delta).max(0.01)
        }
        HandleKind::Resize(ResizeAxis::BoxY(_)) => {
            shape.half_extents.y = (start.half_extents.y + delta).max(0.01)
        }
        HandleKind::Resize(ResizeAxis::BoxZ(_)) => {
            shape.half_extents.z = (start.half_extents.z + delta).max(0.01)
        }
        HandleKind::Resize(ResizeAxis::Radius) => shape.radius = (start.radius + delta).max(0.01),
        HandleKind::Resize(ResizeAxis::Height(_)) => {
            shape.half_height = (start.half_height + delta).max(0.01)
        }
        HandleKind::BodyMove => {}
    }
}
