//! `Vec3`, `Quat` and `Transform` — the maths types that cross the boundary.
//!
//! These are the `#[repr(C)]` mirrors from [`crate::sys`] with Bevy-shaped
//! methods hung off them, so plugin source reads identically to Bevy source.
//! That resemblance is the point and also the hazard: the closer the surface
//! reads like Bevy, the more a behavioural divergence costs, because an author
//! is entitled to assume Bevy's semantics from Bevy's spelling. See the note on
//! `rotate_x` for the one time that went wrong.
//!
//! Degenerate inputs return an identity rather than a `NaN` throughout — a NaN
//! rotation poisons every transform downstream and is very hard to trace back
//! to the normalise that produced it.

use super::component::{Quat, Transform, Vec3};

// `Vec3::length`, the `Quat` constructors and the colour conversions all call
// `f32` methods that only exist in `std`. Under `no_std` the shim supplies
// them; under `std` the inherent methods are used and this import is absent.
#[cfg(not(feature = "std"))]
use crate::float::FloatExt as _;

impl Vec3 {
    pub const ZERO: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 0.0 };
    pub const ONE: Vec3 = Vec3 { x: 1.0, y: 1.0, z: 1.0 };
    pub const X: Vec3 = Vec3 { x: 1.0, y: 0.0, z: 0.0 };
    pub const Y: Vec3 = Vec3 { x: 0.0, y: 1.0, z: 0.0 };
    pub const Z: Vec3 = Vec3 { x: 0.0, y: 0.0, z: 1.0 };

    pub const fn new(x: f32, y: f32, z: f32) -> Vec3 {
        Vec3 { x, y, z }
    }
    pub const fn splat(v: f32) -> Vec3 {
        Vec3 { x: v, y: v, z: v }
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        Vec3::new(
            self.y * o.z - self.z * o.y,
            self.z * o.x - self.x * o.z,
            self.x * o.y - self.y * o.x,
        )
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }
    pub fn distance(self, o: Vec3) -> f32 {
        (self - o).length()
    }
    /// Returns `ZERO` for a zero-length vector rather than `NaN` — a normalise
    /// in a loop over positions will hit coincident points sooner or later, and
    /// a silent NaN poisons everything downstream.
    pub fn normalize_or_zero(self) -> Vec3 {
        let len = self.length();
        if len > 1e-6 {
            self / len
        } else {
            Vec3::ZERO
        }
    }
    pub fn normalize(self) -> Vec3 {
        self / self.length()
    }
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    pub fn clamp_length_max(self, max: f32) -> Vec3 {
        let len = self.length();
        if len > max && len > 1e-6 {
            self * (max / len)
        } else {
            self
        }
    }
}

impl core::ops::Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl core::ops::Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl core::ops::Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        Vec3::new(self.x * s, self.y * s, self.z * s)
    }
}
impl core::ops::Mul<Vec3> for Vec3 {
    type Output = Vec3;
    fn mul(self, o: Vec3) -> Vec3 {
        Vec3::new(self.x * o.x, self.y * o.y, self.z * o.z)
    }
}
impl core::ops::Div<f32> for Vec3 {
    type Output = Vec3;
    fn div(self, s: f32) -> Vec3 {
        Vec3::new(self.x / s, self.y / s, self.z / s)
    }
}
impl core::ops::Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        Vec3::new(-self.x, -self.y, -self.z)
    }
}
impl core::ops::AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl core::ops::SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}
impl core::ops::MulAssign<f32> for Vec3 {
    fn mul_assign(&mut self, s: f32) {
        *self = *self * s;
    }
}

impl Quat {
    pub const IDENTITY: Quat = Quat { x: 0.0, y: 0.0, z: 0.0, w: 1.0 };

    pub fn from_axis_angle(axis: Vec3, angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        let a = axis.normalize_or_zero();
        Quat { x: a.x * s, y: a.y * s, z: a.z * s, w: c }
    }
    pub fn from_rotation_x(angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { x: s, y: 0.0, z: 0.0, w: c }
    }
    pub fn from_rotation_y(angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { x: 0.0, y: s, z: 0.0, w: c }
    }
    pub fn from_rotation_z(angle: f32) -> Quat {
        let (s, c) = (angle * 0.5).sin_cos();
        Quat { x: 0.0, y: 0.0, z: s, w: c }
    }
}

impl core::ops::Mul for Quat {
    type Output = Quat;
    fn mul(self, r: Quat) -> Quat {
        Quat {
            x: self.w * r.x + self.x * r.w + self.y * r.z - self.z * r.y,
            y: self.w * r.y - self.x * r.z + self.y * r.w + self.z * r.x,
            z: self.w * r.z + self.x * r.y - self.y * r.x + self.z * r.w,
            w: self.w * r.w - self.x * r.x - self.y * r.y - self.z * r.z,
        }
    }
}

impl core::ops::Mul<Vec3> for Quat {
    type Output = Vec3;
    /// Rotate a vector. `v + 2w(q x v) + 2(q x (q x v))`.
    fn mul(self, v: Vec3) -> Vec3 {
        let q = Vec3::new(self.x, self.y, self.z);
        let t = q.cross(v) * 2.0;
        v + t * self.w + q.cross(t)
    }
}

impl Quat {
    /// Build a rotation from three orthonormal basis vectors.
    ///
    /// Shepperd's method: pick the branch whose divisor is largest, because the
    /// naive `w`-first form divides by something near zero for rotations close to
    /// 180 degrees and loses most of its precision there.
    pub fn from_basis(x: Vec3, y: Vec3, z: Vec3) -> Quat {
        let trace = x.x + y.y + z.z;
        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            Quat { x: (y.z - z.y) / s, y: (z.x - x.z) / s, z: (x.y - y.x) / s, w: 0.25 * s }
        } else if x.x > y.y && x.x > z.z {
            let s = (1.0 + x.x - y.y - z.z).sqrt() * 2.0;
            Quat { x: 0.25 * s, y: (y.x + x.y) / s, z: (z.x + x.z) / s, w: (y.z - z.y) / s }
        } else if y.y > z.z {
            let s = (1.0 + y.y - x.x - z.z).sqrt() * 2.0;
            Quat { x: (y.x + x.y) / s, y: 0.25 * s, z: (z.y + y.z) / s, w: (z.x - x.z) / s }
        } else {
            let s = (1.0 + z.z - x.x - y.y).sqrt() * 2.0;
            Quat { x: (z.x + x.z) / s, y: (z.y + y.z) / s, z: 0.25 * s, w: (x.y - y.x) / s }
        }
    }

    /// The inverse of a **unit** quaternion, which is its conjugate.
    ///
    /// A `Transform`'s rotation is always unit, which is what makes the cheap
    /// form correct here. Normalise first if you built the quaternion yourself
    /// by other means.
    pub fn inverse(self) -> Quat {
        Quat { x: -self.x, y: -self.y, z: -self.z, w: self.w }
    }

    pub fn length(self) -> f32 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    /// Returns [`Quat::IDENTITY`] for a zero-length quaternion rather than
    /// `NaN`, for the same reason [`Vec3::normalize_or_zero`] exists: a NaN
    /// rotation poisons every transform downstream and is hard to trace back.
    pub fn normalize(self) -> Quat {
        let len = self.length();
        if len > 1e-6 {
            Quat { x: self.x / len, y: self.y / len, z: self.z / len, w: self.w / len }
        } else {
            Quat::IDENTITY
        }
    }

    /// Yaw, pitch, roll applied in Bevy's default order (`YXZ`).
    pub fn from_euler(yaw: f32, pitch: f32, roll: f32) -> Quat {
        Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch) * Quat::from_rotation_z(roll)
    }

    /// Spherical interpolation, falling back to linear when the two are nearly
    /// parallel — `sin(theta)` goes to zero there and the general form divides by it.
    pub fn slerp(self, mut other: Quat, t: f32) -> Quat {
        let mut dot =
            self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w;
        // Take the shorter arc: `q` and `-q` are the same rotation.
        if dot < 0.0 {
            other = Quat { x: -other.x, y: -other.y, z: -other.z, w: -other.w };
            dot = -dot;
        }
        if dot > 0.9995 {
            return Quat {
                x: self.x + (other.x - self.x) * t,
                y: self.y + (other.y - self.y) * t,
                z: self.z + (other.z - self.z) * t,
                w: self.w + (other.w - self.w) * t,
            }
            .normalize();
        }
        let theta = dot.clamp(-1.0, 1.0).acos();
        let sin_theta = theta.sin();
        let a = ((1.0 - t) * theta).sin() / sin_theta;
        let b = (t * theta).sin() / sin_theta;
        Quat {
            x: self.x * a + other.x * b,
            y: self.y * a + other.y * b,
            z: self.z * a + other.z * b,
            w: self.w * a + other.w * b,
        }
    }
}

impl Default for Quat {
    fn default() -> Self {
        Quat::IDENTITY
    }
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        scale: Vec3::ONE,
    };

    pub const fn from_xyz(x: f32, y: f32, z: f32) -> Transform {
        Transform {
            translation: Vec3::new(x, y, z),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
    pub const fn from_translation(translation: Vec3) -> Transform {
        Transform {
            translation,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
    pub const fn with_scale(mut self, scale: Vec3) -> Transform {
        self.scale = scale;
        self
    }
    pub const fn with_translation(mut self, translation: Vec3) -> Transform {
        self.translation = translation;
        self
    }

    pub fn rotate(&mut self, q: Quat) {
        self.rotation = q * self.rotation;
    }

    // ── Rotation, and the one place mimicking Bevy nearly went wrong ──────
    //
    // These four pre-multiply and the `rotate_local_*` below post-multiply.
    // That is not a stylistic pairing — it is the entire difference between
    // turning about the *parent's* axes and turning about the object's own, and
    // it is why the order matters more here than anywhere else in this file.
    //
    // Until 2026-08 `rotate_x/y/z` post-multiplied, so they silently were
    // `rotate_local_*`. Source copied from a Bevy project compiled and spun the
    // wrong way with nothing to see: no error, no warning, just a rotation that
    // drifts once the object is tilted. That is the worst failure this shim can
    // have — the closer the surface reads like Bevy, the more a divergence costs,
    // because an author is entitled to assume Bevy's semantics from Bevy's
    // spelling. Match behaviour, not just names.
    pub fn rotate_x(&mut self, angle: f32) {
        self.rotate(Quat::from_rotation_x(angle));
    }
    pub fn rotate_y(&mut self, angle: f32) {
        self.rotate(Quat::from_rotation_y(angle));
    }
    pub fn rotate_z(&mut self, angle: f32) {
        self.rotate(Quat::from_rotation_z(angle));
    }

    /// Rotate about this entity's own axes, ignoring how it is oriented.
    pub fn rotate_local(&mut self, q: Quat) {
        self.rotation = self.rotation * q;
    }
    pub fn rotate_local_x(&mut self, angle: f32) {
        self.rotate_local(Quat::from_rotation_x(angle));
    }
    pub fn rotate_local_y(&mut self, angle: f32) {
        self.rotate_local(Quat::from_rotation_y(angle));
    }
    pub fn rotate_local_z(&mut self, angle: f32) {
        self.rotate_local(Quat::from_rotation_z(angle));
    }

    /// Point the transform's `-Z` at `target`, keeping `up` as the roll
    /// reference. Mirrors Bevy's, including that `-Z` is forward.
    pub fn looking_at(mut self, target: Vec3, up: Vec3) -> Transform {
        let back = (self.translation - target).normalize_or_zero();
        // Degenerate: the target is where we already are, so any orientation is
        // as correct as any other. Leave the existing one rather than emitting a
        // NaN basis.
        if back == Vec3::ZERO {
            return self;
        }
        let right = up.cross(back).normalize_or_zero();
        if right == Vec3::ZERO {
            // `up` is parallel to the view direction and cannot disambiguate
            // roll. Same outcome, same reasoning.
            return self;
        }
        let up = back.cross(right);
        self.rotation = Quat::from_basis(right, up, back);
        self
    }

    /// Apply this transform to a point: scale, then rotate, then translate.
    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let scaled = Vec3::new(
            point.x * self.scale.x,
            point.y * self.scale.y,
            point.z * self.scale.z,
        );
        self.translation + self.rotation * scaled
    }

    pub fn forward(&self) -> Vec3 {
        self.rotation * -Vec3::Z
    }
    pub fn back(&self) -> Vec3 {
        self.rotation * Vec3::Z
    }
    pub fn right(&self) -> Vec3 {
        self.rotation * Vec3::X
    }
    pub fn left(&self) -> Vec3 {
        self.rotation * -Vec3::X
    }
    pub fn up(&self) -> Vec3 {
        self.rotation * Vec3::Y
    }
    pub fn down(&self) -> Vec3 {
        self.rotation * -Vec3::Y
    }
}

impl Default for Transform {
    fn default() -> Self {
        Transform::IDENTITY
    }
}
