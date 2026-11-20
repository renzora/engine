use alloc::vec::Vec;

use bevy_math::{DVec2, Isometry2d, Vec2};
#[cfg(all(feature = "bevy_reflect", feature = "serialize"))]
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};

use crate::RecipOrZero;

/// [`ComputeMassProperties2d`] implementations for 2D geometric primitives.
mod impls;

pub use impls::*;

/// A trait for computing [`MassProperties2d`] for 2D objects.
///
/// For the 3D equivalent, see [`ComputeMassProperties3d`](crate::ComputeMassProperties3d).
pub trait ComputeMassProperties2d {
    /// Computes the [mass] of the object with a given `density`.
    ///
    /// [mass]: crate#mass
    fn mass(&self, density: f32) -> f32;

    /// Computes the [angular inertia] corresponding to a mass of `1.0`.
    ///
    /// [angular inertia]: crate#angular-inertia
    #[doc(alias = "unit_moment_of_inertia")]
    fn unit_angular_inertia(&self) -> f32;

    /// Computes the [angular inertia] corresponding to the given `mass`.
    ///
    /// Equivalent to `mass * shape.unit_angular_inertia()`.
    ///
    /// [angular inertia]: crate#angular-inertia
    #[inline]
    #[doc(alias = "moment_of_inertia")]
    fn angular_inertia(&self, mass: f32) -> f32 {
        mass * self.unit_angular_inertia()
    }

    /// Computes the local [center of mass] relative to the object's origin.
    ///
    /// [center of mass]: crate#center-of-mass
    fn center_of_mass(&self) -> Vec2;

    /// Computes the [`MassProperties2d`] with a given `density`.
    #[inline]
    fn mass_properties(&self, density: f32) -> MassProperties2d {
        let mass = self.mass(density);
        MassProperties2d::new(mass, self.angular_inertia(mass), self.center_of_mass())
    }
}

/// The [mass], [angular inertia], and local [center of mass] of an object in 2D space.
///
/// [mass]: crate#mass
/// [angular inertia]: crate#angular-inertia
/// [center of mass]: crate#center-of-mass
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "bevy_reflect", reflect(Debug, PartialEq))]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    all(feature = "bevy_reflect", feature = "serialize"),
    reflect(Serialize, Deserialize)
)]
pub struct MassProperties2d {
    /// The [mass].
    ///
    /// [mass]: crate#mass
    pub mass: f32,
    /// The angular inertia along the principal axis.
    ///
    /// [angular inertia]: crate#angular-inertia
    pub angular_inertia: f32,
    /// The local [center of mass] relative to the object's origin.
    ///
    /// [center of mass]: crate#center-of-mass
    pub center_of_mass: Vec2,
}

impl Default for MassProperties2d {
    /// Returns the default [`MassProperties2d`], with zero mass and angular inertia.
    fn default() -> Self {
        Self::ZERO
    }
}

impl MassProperties2d {
    /// Zero mass and angular inertia.
    pub const ZERO: Self = Self {
        mass: 0.0,
        angular_inertia: 0.0,
        center_of_mass: Vec2::ZERO,
    };

    /// Creates a new [`MassProperties2d`] from a given mass, principal angular inertia,
    /// and center of mass in local space.
    #[inline]
    pub fn new(mass: f32, angular_inertia: f32, center_of_mass: Vec2) -> Self {
        Self {
            mass,
            angular_inertia,
            center_of_mass,
        }
    }

    /// Computes approximate mass properties from the given set of points representing a shape.
    ///
    /// This can be used to estimate mass properties for arbitrary shapes
    /// by providing a set of sample points from inside the shape.
    ///
    /// The more points there are, and the more uniformly distributed they are,
    /// the more accurate the estimation will be.
    #[inline]
    pub fn from_point_cloud(points: &[Vec2], mass: f32) -> Self {
        let points_recip = 1.0 / points.len() as f64;

        let center_of_mass =
            (points.iter().fold(DVec2::ZERO, |acc, p| acc + p.as_dvec2()) * points_recip).as_vec2();
        let unit_angular_inertia = points.iter().fold(0.0, |acc, p| {
            let r = p.distance_squared(center_of_mass) as f64;
            acc + r * points_recip
        }) as f32;

        Self::new(mass, mass * unit_angular_inertia, center_of_mass)
    }

    /// Returns the center of mass transformed into global space using the given [isometry].
    ///
    /// [isometry]: Isometry2d
    #[inline]
    pub fn global_center_of_mass(&self, isometry: impl Into<Isometry2d>) -> Vec2 {
        let isometry: Isometry2d = isometry.into();
        isometry.transform_point(self.center_of_mass)
    }

    /// Computes the angular inertia corresponding to a mass of `1.0`.
    ///
    /// If the mass is zero, zero is returned.
    #[inline]
    pub fn unit_angular_inertia(&self) -> f32 {
        self.mass.recip_or_zero() * self.angular_inertia
    }

    /// Computes the principal angular inertia at a given `offset`.
    ///
    /// The shifted angular inertia is computed as `angular_inertia + mass * offset.length_squared()`.
    #[inline]
    pub fn shifted_angular_inertia(&self, offset: Vec2) -> f32 {
        self.angular_inertia + offset.length_squared() * self.mass
    }

    /// Returns the mass properties transformed by the given [isometry].
    ///
    /// [isometry]: Isometry2d
    #[inline]
    pub fn transformed_by(mut self, isometry: impl Into<Isometry2d>) -> Self {
        self.transform_by(isometry);
        self
    }

    /// Transforms the mass properties by the given [isometry].
    ///
    /// [isometry]: Isometry2d
    #[inline]
    pub fn transform_by(&mut self, isometry: impl Into<Isometry2d>) {
        self.center_of_mass = self.global_center_of_mass(isometry);
    }

    /// Returns the mass propeorties with the inverse of mass and angular inertia.
    ///
    /// The center of mass is left unchanged.
    #[inline]
    pub fn inverse(&self) -> Self {
        Self {
            mass: self.mass.recip_or_zero(),
            angular_inertia: self.angular_inertia.recip_or_zero(),
            center_of_mass: self.center_of_mass,
        }
    }

    /// Sets the mass to the given `new_mass`.
    ///
    /// If `update_angular_inertia` is `true`, the angular inertia will be scaled accordingly.
    #[inline]
    pub fn set_mass(&mut self, new_mass: f32, update_angular_inertia: bool) {
        if update_angular_inertia {
            // Adjust angular inertia to match the new mass.
            self.angular_inertia *= new_mass * self.mass.recip_or_zero();
        }
        self.mass = new_mass;
    }
}

impl core::ops::Add for MassProperties2d {
    type Output = Self;

    #[inline]
    fn add(self, other: Self) -> Self::Output {
        if self == Self::ZERO {
            return other;
        } else if other == Self::ZERO {
            return self;
        }

        let mass1 = self.mass;
        let mass2 = other.mass;
        let new_mass = mass1 + mass2;

        // The new center of mass is the weighted average of the centers of masses of `self` and `other`.
        let new_center_of_mass =
            (self.center_of_mass * mass1 + other.center_of_mass * mass2) * new_mass.recip_or_zero();

        // Compute the new principal angular inertia, taking the new center of mass into account.
        let i1 = self.shifted_angular_inertia(new_center_of_mass - self.center_of_mass);
        let i2 = other.shifted_angular_inertia(new_center_of_mass - other.center_of_mass);
        let new_angular_inertia = i1 + i2;

        Self {
            mass: new_mass,
            angular_inertia: new_angular_inertia,
            center_of_mass: new_center_of_mass,
        }
    }
}

impl core::ops::AddAssign for MassProperties2d {
    #[inline]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl core::ops::Sub for MassProperties2d {
    type Output = Self;

    #[inline]
    fn sub(self, other: Self) -> Self::Output {
        if self == Self::ZERO || other == Self::ZERO {
            return self;
        }

        let mass1 = self.mass;
        let mass2 = other.mass;

        if mass1 <= mass2 {
            // The result would have non-positive mass.
            return Self {
                center_of_mass: self.center_of_mass,
                ..Self::ZERO
            };
        }

        let new_mass = mass1 - mass2;

        // The new center of mass is the negated weighted average of the centers of masses of `self` and `other`.
        let new_center_of_mass =
            (self.center_of_mass * mass1 - other.center_of_mass * mass2) * new_mass.recip_or_zero();

        // Compute the new principal angular inertia, taking the new center of mass into account.
        let i1 = self.shifted_angular_inertia(new_center_of_mass - self.center_of_mass);
        let i2 = other.shifted_angular_inertia(new_center_of_mass - other.center_of_mass);
        let new_angular_inertia = (i1 - i2).max(0.0);

        Self {
            mass: new_mass,
            angular_inertia: new_angular_inertia,
            center_of_mass: new_center_of_mass,
        }
    }
}

impl core::ops::SubAssign for MassProperties2d {
    #[inline]
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl core::iter::Sum for MassProperties2d {
    #[inline]
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        let mut total_mass = 0.0;
        let mut total_angular_inertia = 0.0;
        let mut total_center_of_mass = Vec2::ZERO;

        // TODO: Avoid this allocation if possible. This is currently needed because we iterate twice.
        let mut all_properties = Vec::with_capacity(iter.size_hint().1.unwrap_or_default());

        for props in iter {
            total_mass += props.mass;
            total_center_of_mass += props.center_of_mass * props.mass;
            all_properties.push(props);
        }

        if total_mass > 0.0 {
            total_center_of_mass *= total_mass.recip_or_zero();
        }

        for props in all_properties {
            total_angular_inertia +=
                props.shifted_angular_inertia(total_center_of_mass - props.center_of_mass);
        }

        if !total_center_of_mass.is_finite() {
            // The center of mass can be non-finite if the mass is non-finite.
            total_center_of_mass = Vec2::ZERO;
        }

        Self::new(total_mass, total_angular_inertia, total_center_of_mass)
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::AbsDiffEq for MassProperties2d {
    type Epsilon = f32;
    fn default_epsilon() -> f32 {
        f32::EPSILON
    }
    fn abs_diff_eq(&self, other: &Self, epsilon: f32) -> bool {
        self.mass.abs_diff_eq(&other.mass, epsilon)
            && self
                .angular_inertia
                .abs_diff_eq(&other.angular_inertia, epsilon)
            && self
                .center_of_mass
                .abs_diff_eq(other.center_of_mass, epsilon)
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::RelativeEq for MassProperties2d {
    fn default_max_relative() -> f32 {
        f32::EPSILON
    }
    fn relative_eq(&self, other: &Self, epsilon: f32, max_relative: f32) -> bool {
        self.mass.relative_eq(&other.mass, epsilon, max_relative)
            && self
                .angular_inertia
                .relative_eq(&other.angular_inertia, epsilon, max_relative)
            && self
                .center_of_mass
                .relative_eq(&other.center_of_mass, epsilon, max_relative)
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::UlpsEq for MassProperties2d {
    fn default_max_ulps() -> u32 {
        4
    }
    fn ulps_eq(&self, other: &Self, epsilon: f32, max_ulps: u32) -> bool {
        self.mass.ulps_eq(&other.mass, epsilon, max_ulps)
            && self
                .angular_inertia
                .ulps_eq(&other.angular_inertia, epsilon, max_ulps)
            && self
                .center_of_mass
                .ulps_eq(&other.center_of_mass, epsilon, max_ulps)
    }
}

// TODO: Tests
