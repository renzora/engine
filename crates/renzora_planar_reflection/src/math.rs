//! The geometry: the plane, the mirrored camera pose, and the oblique
//! projection that clips everything behind the mirror.
//!
//! All of it is plain math over Bevy types, with no ECS access, so the tests
//! can pin it down exactly.

use bevy::shape::{HalfSpace, ViewFrustum};
use bevy::camera::primitives::Frustum;
use bevy::camera::{CameraProjection, PerspectiveProjection, SubCameraView};
use bevy::math::{Affine3A, Mat4, Vec3, Vec3A, Vec4};
use bevy::prelude::{GlobalTransform, Transform};

use crate::component::NormalAxis;

/// A plane in world space: a unit normal and a point on it.
///
/// The normal points to the side the surface reflects toward, which is also
/// the side the mirror camera may see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionPlane {
    pub normal: Vec3,
    pub point: Vec3,
}

impl ReflectionPlane {
    /// The plane through the entity's origin, normal along `axis` in its local
    /// space.
    ///
    /// The normal goes through the inverse transpose of the linear part, not
    /// the rotation alone: a mirror squashed by a non-uniform scale and then
    /// rotated has a normal that is not the rotated axis, and using the
    /// rotation would tilt the reflection off the visible surface. `None` for
    /// a degenerate (zero-scale) transform, which has no plane.
    pub fn from_transform(transform: &GlobalTransform, axis: NormalAxis) -> Option<Self> {
        let affine = transform.affine();
        let linear = affine.matrix3;
        if linear.determinant().abs() < 1e-12 {
            return None;
        }
        let normal = (linear.inverse().transpose() * Vec3A::from(axis.local())).normalize_or_zero();
        if normal == Vec3A::ZERO {
            return None;
        }
        Some(Self {
            normal: normal.into(),
            point: affine.translation.into(),
        })
    }

    /// The plane a surface actually reflects about: the axis plane, replaced
    /// by the glass found in the mesh when [`PlanarReflection::fit_to_mesh`]
    /// is set (see `fit`; through the middle of its local bounds when nothing
    /// was found), turned round for `flip`, then moved by
    /// `plane_offset`. Every reader goes through this so the capture camera,
    /// the visibility test and the gizmo agree on one plane.
    pub fn for_surface(
        transform: &GlobalTransform,
        reflection: &crate::PlanarReflection,
        aabb: Option<&bevy::camera::primitives::Aabb>,
        fit: Option<&crate::fit::MeshPlaneFit>,
    ) -> Option<Self> {
        let mut plane = Self::from_transform(transform, reflection.normal_axis)?;
        if reflection.fit_to_mesh {
            // The sheet found in the geometry when there is one for this
            // axis (see `fit`), else the middle of the bounds.
            match fit.filter(|f| f.found && f.axis == reflection.normal_axis) {
                Some(f) => {
                    // The glass's own normal, through the same inverse
                    // transpose as the axis in `from_transform`.
                    let linear = transform.affine().matrix3;
                    let normal = (linear.inverse().transpose() * Vec3A::from(f.local_normal)).normalize_or_zero();
                    if normal != Vec3A::ZERO {
                        plane.normal = normal.into();
                    }
                    plane.point = transform.transform_point(f.local_point);
                }
                None => {
                    if let Some(aabb) = aabb {
                        plane.point = transform.transform_point(Vec3::from(aabb.center));
                    }
                }
            }
        }
        if reflection.flip {
            plane.normal = -plane.normal;
        }
        let offset = if reflection.plane_offset.is_finite() { reflection.plane_offset } else { 0.0 };
        Some(plane.offset(offset))
    }

    /// Positive on the reflecting side, in world units.
    pub fn signed_distance(&self, point: Vec3) -> f32 {
        self.normal.dot(point - self.point)
    }

    pub fn reflect_point(&self, point: Vec3) -> Vec3 {
        point - 2.0 * self.signed_distance(point) * self.normal
    }

    pub fn reflect_vector(&self, v: Vec3) -> Vec3 {
        v - 2.0 * v.dot(self.normal) * self.normal
    }

    /// The same plane moved `distance` along its normal.
    pub fn offset(&self, distance: f32) -> Self {
        Self {
            normal: self.normal,
            point: self.point + self.normal * distance,
        }
    }

    /// `(n, d)` with `n . p + d = 0` on the plane, positive on the reflecting
    /// side.
    pub fn to_vec4(&self) -> Vec4 {
        self.normal.extend(-self.normal.dot(self.point))
    }
}

/// The mirror camera's pose: `camera` reflected across `plane`.
///
/// A true reflection is improper (determinant -1) and a `Transform` cannot
/// hold one, so this keeps a proper rotation built from the reflected forward
/// and up vectors. That camera's right axis is the reflection's left, so its
/// image is the true reflection flipped horizontally; the overlay shader
/// samples at `1 - u` to undo it. Keeping the rotation proper also keeps the
/// triangle winding, so back-face culling stays correct in the capture without
/// touching every material's cull mode.
pub fn mirror_camera_transform(camera: &GlobalTransform, plane: &ReflectionPlane) -> Transform {
    let position = plane.reflect_point(camera.translation());
    let forward = plane.reflect_vector(*camera.forward());
    let up = plane.reflect_vector(*camera.up());
    Transform::from_translation(position).looking_to(forward, up)
}

/// A world-space plane `(n, d)` expressed in a camera's view space.
///
/// Planes transform by the inverse transpose of the point transform. Points go
/// view to world through `world_from_view`, so planes go world to view through
/// its transpose: `plane_world . (M p) = (M^T plane_world) . p`.
pub fn plane_in_view_space(plane_world: Vec4, world_from_view: &Affine3A) -> Vec4 {
    Mat4::from(*world_from_view).transpose() * plane_world
}

/// Replace a reverse-Z perspective matrix's near plane with `plane`, a view
/// space plane `(n, d)` with the visible side positive.
///
/// This is Eric Lengyel's oblique frustum ("Modifying the Projection Matrix to
/// Perform Oblique Near-Plane Clipping", 2005), restated for Bevy's
/// conventions: depth runs from 1 at the near plane to 0 at the far one, and
/// the far plane is at infinity. Clip-space depth is `row3 . v` against
/// `w = row4 . v`, so the near plane (depth 1) is `row4 - row3 = 0` and the far
/// plane (depth 0) is `row3 = 0`. Setting `row3 = row4 - a C` makes the near
/// plane `a C . v = 0`, the clip plane itself. The scale `a` is chosen so the
/// new far plane still passes through the frustum corner opposite the clip
/// plane, `Q`, which keeps the depth range as tight as the geometry allows:
/// `(row4 - a C) . Q = 0`, and with `row4 = (0, 0, -1, 0)` that is
/// `a = -Q.z / (C . Q)`.
///
/// The caller must only pass a plane the camera is behind (`d < 0`); a plane
/// through or behind the camera has no oblique frustum.
pub fn apply_oblique_near_plane(matrix: &mut Mat4, plane: Vec4) {
    // The far corner opposite the plane, in clip space (Lengyel's equation 7).
    // With the far plane at infinity this is a point at infinity in view
    // space; in homogeneous coordinates that is fine.
    let q_clip = Vec4::new(plane.x.signum(), plane.y.signum(), 0.0, 1.0);
    let q = matrix.inverse() * q_clip;
    let a = -q.z / plane.dot(q);
    let row3 = Vec4::NEG_Z - plane * a;
    matrix.x_axis.z = row3.x;
    matrix.y_axis.z = row3.y;
    matrix.z_axis.z = row3.z;
    matrix.w_axis.z = row3.w;
}

/// Below this (in view space, so in world units) the camera is effectively on
/// the clip plane and the oblique matrix would crush depth to nothing.
const MIN_PLANE_DISTANCE: f32 = 1e-4;

/// A perspective projection whose near plane is the mirror plane.
///
/// Wrapped as a custom projection (`Projection::custom`) rather than using
/// `PerspectiveProjection::near_clip_plane` so the oblique step is ours to
/// test and to reason about, and so the clip plane can be absent (`None`) for
/// a frame without the inner projection carrying a sentinel.
///
/// Frustum culling stays correct through the `compute_frustum` override below,
/// which makes the clip plane the frustum's near half-space, so whole objects
/// behind the mirror are culled before they cost a draw.
#[derive(Clone, Debug)]
pub struct ObliqueProjection {
    /// FOV, aspect, near and far. Its own `near_clip_plane` is left at the
    /// default so Bevy does not apply a second adjustment.
    pub perspective: PerspectiveProjection,
    /// The view-space clip plane, or `None` for a plain perspective.
    pub clip_plane: Option<Vec4>,
}

impl ObliqueProjection {
    pub fn new(perspective: PerspectiveProjection, clip_plane: Option<Vec4>) -> Self {
        Self {
            perspective: PerspectiveProjection {
                near_clip_plane: PerspectiveProjection::default().near_clip_plane,
                ..perspective
            },
            clip_plane: clip_plane.filter(|p| p.w < -MIN_PLANE_DISTANCE),
        }
    }

    /// Whether `other` would produce the same matrix. `PerspectiveProjection`
    /// has no `PartialEq`, and the drive system compares before writing so an
    /// unchanged mirror does not flag `Projection` changed every frame.
    pub fn same_as(&self, other: &Self) -> bool {
        let (a, b) = (&self.perspective, &other.perspective);
        a.fov == b.fov
            && a.aspect_ratio == b.aspect_ratio
            && a.near == b.near
            && a.far == b.far
            && self.clip_plane == other.clip_plane
    }

    fn adjust(&self, mut matrix: Mat4) -> Mat4 {
        if let Some(plane) = self.clip_plane {
            apply_oblique_near_plane(&mut matrix, plane);
        }
        matrix
    }
}

impl CameraProjection for ObliqueProjection {
    fn get_clip_from_view(&self) -> Mat4 {
        self.adjust(self.perspective.get_clip_from_view())
    }

    fn get_clip_from_view_for_sub(&self, sub_view: &SubCameraView) -> Mat4 {
        self.adjust(self.perspective.get_clip_from_view_for_sub(sub_view))
    }

    fn update(&mut self, width: f32, height: f32) {
        self.perspective.update(width, height);
    }

    fn far(&self) -> f32 {
        self.perspective.far
    }

    /// The plain perspective's corners. These feed directional-shadow cascade
    /// fitting, which wants the camera's real view volume rather than one
    /// sheared by the clip plane.
    fn get_frustum_corners(&self, z_near: f32, z_far: f32) -> [Vec3A; 8] {
        self.perspective.get_frustum_corners(z_near, z_far)
    }

    /// Bevy's frustum with its near half-space replaced by the clip plane.
    ///
    /// The default derivation cannot be trusted here: it takes the near
    /// half-space as `row4 + row3` of the clip matrix, which under reverse Z
    /// with an infinite far plane is just "in front of the camera", and after
    /// the oblique rewrite of row 3 it is not even that. The side planes come
    /// from rows 1, 2 and 4, which the rewrite leaves alone, so only the near
    /// plane needs supplying: the mirror itself, in world space, which is what
    /// lets culling drop whole objects behind the mirror.
    fn compute_frustum(&self, camera_transform: &GlobalTransform) -> Frustum {
        let view_from_world = camera_transform.affine().inverse();
        let clip_from_world = self.get_clip_from_view() * view_from_world;
        let mut frustum = Frustum(ViewFrustum::from_clip_from_world_custom_far(
            &clip_from_world,
            &camera_transform.translation(),
            &camera_transform.back(),
            self.far(),
        ));
        if let Some(plane) = self.clip_plane {
            // View to world for a plane is the transpose of world to view for
            // a point (see `plane_in_view_space`).
            let world = Mat4::from(view_from_world).transpose() * plane;
            frustum.half_spaces[ViewFrustum::NEAR_PLANE_IDX] = HalfSpace::new(world);
        }
        frustum
    }
}
