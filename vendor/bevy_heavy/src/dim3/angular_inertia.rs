use core::ops::*;

use bevy_math::{Mat3, Quat, Vec3};
#[cfg(all(feature = "bevy_reflect", feature = "serialize"))]
use bevy_reflect::{ReflectDeserialize, ReflectSerialize};
use glam_matrix_extras::{MatConversionError, SymmetricEigen3, SymmetricMat3};

/// An error returned for an invalid [`AngularInertiaTensor`] in 3D.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AngularInertiaTensorError {
    /// The mass is negative or NaN.
    Negative,
    /// The mass is NaN.
    Nan,
}

/// The 3x3 [angular inertia] tensor of a 3D object, representing resistance to angular acceleration.
///
/// The [inertia tensor] is a [symmetric], [positive-semidefinite] matrix that describes the moment of inertia
/// for rotations about the X, Y, and Z axes. By [diagonalizing] this matrix, it is possible to extract
/// the [principal axes of inertia] (a [`Vec3`]) and a local inertial frame (a [`Quat`]) that defines
/// the XYZ axes. This diagonalization can be performed using the [`principal_angular_inertia_with_local_frame`] method.
///
/// The diagonalized representation is more compact and often easier to work with,
/// but the full tensor can be more efficient for computations using the angular inertia.
///
/// [angular inertia]: crate#angular-inertia
/// [inertia tensor]: https://en.wikipedia.org/wiki/Moment_of_inertia#Inertia_tensor
/// [symmetric]: https://en.wikipedia.org/wiki/Symmetric_matrix
/// [positive-semidefinite]: https://en.wikipedia.org/wiki/Definite_matrix
/// [diagonalizing]: https://en.wikipedia.org/wiki/Diagonalizable_matrix#Diagonalization
/// [principal axes of inertia]: https://en.wikipedia.org/wiki/Moment_of_inertia#Principal_axes
/// [`principal_angular_inertia_with_local_frame`]: AngularInertiaTensor::principal_angular_inertia_with_local_frame
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy_reflect", derive(bevy_reflect::Reflect))]
#[cfg_attr(feature = "bevy_reflect", reflect(Debug, PartialEq))]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    all(feature = "bevy_reflect", feature = "serialize"),
    reflect(Serialize, Deserialize)
)]
#[doc(alias = "MomentOfInertiaTensor")]
pub struct AngularInertiaTensor(SymmetricMat3);

impl Deref for AngularInertiaTensor {
    type Target = SymmetricMat3;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AngularInertiaTensor {
    /// Zero angular inertia.
    pub const ZERO: Self = Self(SymmetricMat3::ZERO);

    /// An angular inertia tensor with a principal angular inertia of `1.0` along the diagonal.
    pub const IDENTITY: Self = Self(SymmetricMat3::IDENTITY);

    /// Infinite angular inertia.
    pub const INFINITY: Self = Self(SymmetricMat3::from_diagonal(Vec3::INFINITY));

    /// Creates a new [`AngularInertiaTensor`] from the given principal angular inertia.
    ///
    /// The principal angular inertia represents the torque needed for a desired angular acceleration
    /// about the local coordinate axes. To specify the orientation of the local inertial frame,
    /// consider using [`new_with_local_frame`](AngularInertiaTensor::new_with_local_frame).
    ///
    /// # Panics
    ///
    /// Panics if any component of the principal angular inertia is negative or NaN
    /// when `debug_assertions` are enabled.
    #[inline]
    #[doc(alias = "from_principal_angular_inertia")]
    pub fn new(principal_angular_inertia: Vec3) -> Self {
        debug_assert!(
            principal_angular_inertia.cmpge(Vec3::ZERO).all(),
            "principal angular inertia must be positive or zero for all axes"
        );

        Self(SymmetricMat3::from_diagonal(principal_angular_inertia))
    }

    /// Tries to create a new [`AngularInertiaTensor`] from the given principal angular inertia.
    ///
    /// The principal angular inertia represents the torque needed for a desired angular acceleration
    /// about the local coordinate axes. To specify the orientation of the local inertial frame,
    /// consider using [`try_new_with_local_frame`](AngularInertiaTensor::try_new_with_local_frame).
    ///
    /// # Errors
    ///
    /// Returns [`Err(AngularInertiaTensorError)`](AngularInertiaTensorError) if any component
    /// of the principal angular inertia is negative or NaN.
    #[inline]
    pub fn try_new(principal_angular_inertia: Vec3) -> Result<Self, AngularInertiaTensorError> {
        if !principal_angular_inertia.cmpge(Vec3::ZERO).all() {
            Err(AngularInertiaTensorError::Negative)
        } else if principal_angular_inertia.is_nan() {
            Err(AngularInertiaTensorError::Nan)
        } else {
            Ok(Self(SymmetricMat3::from_diagonal(
                principal_angular_inertia,
            )))
        }
    }

    /// Creates a new [`AngularInertiaTensor`] from the given principal angular inertia
    /// and the orientation of the local inertial frame.
    ///
    /// The principal angular inertia represents the torque needed for a desired angular acceleration
    /// about the local coordinate axes defined by the given `orientation`.
    ///
    /// # Panics
    ///
    /// Panics if any component of the principal angular inertia is negative or NaN
    /// when `debug_assertions` are enabled.
    #[inline]
    #[doc(alias = "from_principal_angular_inertia_with_local_frame")]
    pub fn new_with_local_frame(principal_angular_inertia: Vec3, orientation: Quat) -> Self {
        debug_assert!(
            principal_angular_inertia.cmpge(Vec3::ZERO).all(),
            "principal angular inertia must be positive or zero for all axes"
        );

        Self(SymmetricMat3::from_mat3_unchecked(
            Mat3::from_quat(orientation)
                * Mat3::from_diagonal(principal_angular_inertia)
                * Mat3::from_quat(orientation.inverse()),
        ))
    }

    /// Tries to create a new [`AngularInertiaTensor`] from the given principal angular inertia
    /// and the orientation of the local inertial frame.
    ///
    /// The principal angular inertia represents the torque needed for a desired angular acceleration
    /// about the local coordinate axes defined by the given `orientation`.
    ///
    /// # Errors
    ///
    /// Returns [`Err(AngularInertiaTensorError)`](AngularInertiaTensorError) if any component
    /// of the principal angular inertia is negative or NaN.
    #[inline]
    pub fn try_new_with_local_frame(
        principal_angular_inertia: Vec3,
        orientation: Quat,
    ) -> Result<Self, AngularInertiaTensorError> {
        if !principal_angular_inertia.cmpge(Vec3::ZERO).all() {
            Err(AngularInertiaTensorError::Negative)
        } else if principal_angular_inertia.is_nan() {
            Err(AngularInertiaTensorError::Nan)
        } else {
            Ok(Self(SymmetricMat3::from_mat3_unchecked(
                Mat3::from_quat(orientation)
                    * Mat3::from_diagonal(principal_angular_inertia)
                    * Mat3::from_quat(orientation.inverse()),
            )))
        }
    }

    /// Creates a new [`AngularInertiaTensor`] from the given angular inertia [tensor]
    /// represented as a [`SymmetricMat3`].
    ///
    /// The tensor should be [positive-semidefinite], but this is *not* checked.
    ///
    /// [tensor]: https://en.wikipedia.org/wiki/Moment_of_inertia#Inertia_tensor
    /// [positive-semidefinite]: https://en.wikipedia.org/wiki/Definite_matrix
    #[inline]
    #[must_use]
    #[doc(alias = "from_tensor")]
    pub const fn from_symmetric_mat3(mat: SymmetricMat3) -> Self {
        Self(mat)
    }

    /// Tries to create a new [`AngularInertiaTensor`] from the given angular inertia [tensor]
    /// represented as a [`Mat3`].
    ///
    /// The tensor should be [positive-semidefinite], but this is *not* checked.
    ///
    /// [tensor]: https://en.wikipedia.org/wiki/Moment_of_inertia#Inertia_tensor
    /// [positive-semidefinite]: https://en.wikipedia.org/wiki/Definite_matrix
    ///
    /// # Errors
    ///
    /// Returns a [`MatConversionError`] if the given matrix is not symmetric.
    #[inline]
    pub fn try_from_mat3(mat: Mat3) -> Result<Self, MatConversionError> {
        SymmetricMat3::try_from_mat3(mat).map(Self)
    }

    /// Creates a new [`AngularInertiaTensor`] from the given angular inertia [tensor]
    /// represented as a [`Mat3`].
    ///
    /// Only the lower left triangle of the matrix is used. No check is performed to ensure
    /// that the given matrix is truly symmetric or positive-semidefinite.
    #[inline]
    #[must_use]
    pub const fn from_mat3_unchecked(mat: Mat3) -> Self {
        Self(SymmetricMat3::from_mat3_unchecked(mat))
    }

    /// Returns the angular inertia tensor as a [`SymmetricMat3`].
    ///
    /// Equivalent to [`value`](AngularInertiaTensor::value).
    #[inline]
    #[doc(alias = "as_tensor")]
    pub fn as_symmetric_mat3(&self) -> SymmetricMat3 {
        self.0
    }

    /// Returns a mutable reference to the [`SymmetricMat3`] stored in `self`.
    ///
    /// Equivalent to [`value_mut`](AngularInertiaTensor::value_mut).
    #[inline]
    #[doc(alias = "as_tensor_mut")]
    pub fn as_symmetric_mat3_mut(&mut self) -> &mut SymmetricMat3 {
        &mut self.0
    }

    /// Returns the angular inertia tensor as a [`SymmetricMat3`].
    ///
    /// Equivalent to [`as_symmetric_mat3`](AngularInertiaTensor::as_symmetric_mat3).
    #[inline]
    pub fn value(self) -> SymmetricMat3 {
        self.0
    }

    /// Returns a mutable reference to the [`SymmetricMat3`] stored in `self`.
    ///
    /// Equivalent to [`as_symmetric_mat3_mut`](AngularInertiaTensor::as_symmetric_mat3_mut).
    #[inline]
    pub fn value_mut(&mut self) -> &mut SymmetricMat3 {
        &mut self.0
    }

    /// Returns the angular inertia tensor as a [`Mat3`].
    #[inline]
    #[doc(alias = "to_tensor")]
    pub fn to_mat3(&self) -> Mat3 {
        self.0.to_mat3()
    }

    /// Returns the inverse of the angular inertia tensor.
    #[inline]
    pub fn inverse(self) -> Self {
        Self(self.inverse_or_zero())
    }

    /// Sets the angular inertia tensor to the given value.
    #[inline]
    pub fn set(&mut self, angular_inertia: impl Into<AngularInertiaTensor>) {
        *self = angular_inertia.into();
    }

    /// Computes the principal angular inertia and local inertial frame
    /// by diagonalizing the 3x3 tensor matrix.
    ///
    /// The principal angular inertia represents the torque needed for a desired angular acceleration
    /// about the local coordinate axes defined by the local inertial frame.
    #[doc(alias = "diagonalize")]
    pub fn principal_angular_inertia_with_local_frame(&self) -> (Vec3, Quat) {
        let mut eigen = SymmetricEigen3::new(self.0).reverse();

        if eigen.eigenvectors.determinant() < 0.0 {
            core::mem::swap(
                &mut eigen.eigenvectors.y_axis,
                &mut eigen.eigenvectors.z_axis,
            );
            core::mem::swap(&mut eigen.eigenvalues.y, &mut eigen.eigenvalues.z);
        }

        let mut local_inertial_frame = Quat::from_mat3(&eigen.eigenvectors).normalize();

        if !local_inertial_frame.is_finite() {
            local_inertial_frame = Quat::IDENTITY;
        }

        // Clamp eigenvalues to be non-negative.
        let principal_angular_inertia = eigen.eigenvalues.max(Vec3::ZERO);

        (principal_angular_inertia, local_inertial_frame)
    }

    /// Computes the angular inertia tensor with the given rotation.
    ///
    /// This can be used to transform local angular inertia to world space.
    #[inline]
    pub fn rotated(self, rotation: Quat) -> Self {
        let rot_mat3 = Mat3::from_quat(rotation);
        Self::from_mat3_unchecked((rot_mat3 * self.0) * rot_mat3.transpose())
    }

    /// Computes the angular inertia tensor shifted by the given offset, taking into account the given mass.
    #[inline]
    pub fn shifted(self, mass: f32, offset: Vec3) -> Self {
        if offset != Vec3::ZERO {
            // https://en.wikipedia.org/wiki/Parallel_axis_theorem#Tensor_generalization
            let diagonal_element = offset.length_squared();
            let diagonal_mat = Mat3::from_diagonal(Vec3::splat(diagonal_element));
            let offset_outer_product =
                Mat3::from_cols(offset * offset.x, offset * offset.y, offset * offset.z);
            Self::from_mat3_unchecked(self.0 + mass * (diagonal_mat - offset_outer_product))
        } else {
            self
        }
    }
}

impl From<SymmetricMat3> for AngularInertiaTensor {
    #[inline]
    fn from(angular_inertia: SymmetricMat3) -> Self {
        Self::from_symmetric_mat3(angular_inertia)
    }
}

impl TryFrom<Mat3> for AngularInertiaTensor {
    type Error = MatConversionError;

    #[inline]
    fn try_from(angular_inertia: Mat3) -> Result<Self, Self::Error> {
        Self::try_from_mat3(angular_inertia)
    }
}

impl From<AngularInertiaTensor> for SymmetricMat3 {
    #[inline]
    fn from(angular_inertia: AngularInertiaTensor) -> Self {
        angular_inertia.0
    }
}

impl From<AngularInertiaTensor> for Mat3 {
    #[inline]
    fn from(angular_inertia: AngularInertiaTensor) -> Self {
        angular_inertia.0.to_mat3()
    }
}

impl TryFrom<Vec3> for AngularInertiaTensor {
    type Error = AngularInertiaTensorError;

    #[inline]
    fn try_from(principal_angular_inertia: Vec3) -> Result<Self, Self::Error> {
        Self::try_new(principal_angular_inertia)
    }
}

impl Add<AngularInertiaTensor> for AngularInertiaTensor {
    type Output = Self;

    #[inline]
    fn add(self, rhs: AngularInertiaTensor) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign<AngularInertiaTensor> for AngularInertiaTensor {
    #[inline]
    fn add_assign(&mut self, rhs: AngularInertiaTensor) {
        self.0 += rhs.0;
    }
}

impl Mul<AngularInertiaTensor> for f32 {
    type Output = AngularInertiaTensor;

    #[inline]
    fn mul(self, rhs: AngularInertiaTensor) -> AngularInertiaTensor {
        AngularInertiaTensor(self * rhs.0)
    }
}

impl MulAssign<f32> for AngularInertiaTensor {
    #[inline]
    fn mul_assign(&mut self, rhs: f32) {
        self.0 *= rhs;
    }
}

impl Div<f32> for AngularInertiaTensor {
    type Output = Self;

    #[inline]
    fn div(self, rhs: f32) -> Self {
        Self(self.0 / rhs)
    }
}

impl DivAssign<f32> for AngularInertiaTensor {
    #[inline]
    fn div_assign(&mut self, rhs: f32) {
        self.0 /= rhs;
    }
}

impl Mul<AngularInertiaTensor> for Quat {
    type Output = AngularInertiaTensor;

    #[inline]
    fn mul(self, angular_inertia: AngularInertiaTensor) -> AngularInertiaTensor {
        angular_inertia.rotated(self)
    }
}

impl Mul<Vec3> for AngularInertiaTensor {
    type Output = Vec3;

    #[inline]
    fn mul(self, rhs: Vec3) -> Vec3 {
        self.0 * rhs
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::AbsDiffEq for AngularInertiaTensor {
    type Epsilon = f32;
    fn default_epsilon() -> f32 {
        f32::EPSILON
    }
    fn abs_diff_eq(&self, other: &Self, epsilon: f32) -> bool {
        self.0.abs_diff_eq(&other.0, epsilon)
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::RelativeEq for AngularInertiaTensor {
    fn default_max_relative() -> f32 {
        f32::EPSILON
    }
    fn relative_eq(&self, other: &Self, epsilon: f32, max_relative: f32) -> bool {
        self.0.relative_eq(&other.0, epsilon, max_relative)
    }
}

#[cfg(any(feature = "approx", test))]
impl approx::UlpsEq for AngularInertiaTensor {
    fn default_max_ulps() -> u32 {
        4
    }
    fn ulps_eq(&self, other: &Self, epsilon: f32, max_ulps: u32) -> bool {
        self.0.ulps_eq(&other.0, epsilon, max_ulps)
    }
}
