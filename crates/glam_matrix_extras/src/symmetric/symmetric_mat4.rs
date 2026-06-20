use core::iter::Sum;
use core::ops::*;
#[cfg(feature = "f64")]
use glam::{DMat4, DVec4};
use glam::{Mat4, Vec4};

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize, std_traits::ReflectDefault};

use crate::{MatConversionError, SquareMatExt, ops::FloatAbs};

/// An extension trait for 4x4 matrices.
pub trait Mat4Ext {
    /// The type of the symmetric 4x4 matrix.
    type SymmetricMat4;

    /// Multiplies `self` by a symmetric 4x4 matrix.
    fn mul_symmetric_mat4(&self, rhs: &Self::SymmetricMat4) -> Self;

    /// Adds a symmetric 4x4 matrix to `self`.
    fn add_symmetric_mat4(&self, rhs: &Self::SymmetricMat4) -> Self;

    /// Subtracts a symmetric 4x4 matrix from `self`.
    fn sub_symmetric_mat4(&self, rhs: &Self::SymmetricMat4) -> Self;
}

#[cfg(feature = "f32")]
impl Mat4Ext for Mat4 {
    type SymmetricMat4 = SymmetricMat4;

    #[inline]
    fn mul_symmetric_mat4(&self, rhs: &SymmetricMat4) -> Mat4 {
        self.mul(rhs)
    }

    #[inline]
    fn add_symmetric_mat4(&self, rhs: &SymmetricMat4) -> Mat4 {
        self.add(rhs)
    }

    #[inline]
    fn sub_symmetric_mat4(&self, rhs: &SymmetricMat4) -> Mat4 {
        self.sub(rhs)
    }
}

#[cfg(feature = "f64")]
impl Mat4Ext for DMat4 {
    type SymmetricMat4 = SymmetricDMat4;

    #[inline]
    fn mul_symmetric_mat4(&self, rhs: &SymmetricDMat4) -> DMat4 {
        self.mul(rhs)
    }

    #[inline]
    fn add_symmetric_mat4(&self, rhs: &SymmetricDMat4) -> DMat4 {
        self.add(rhs)
    }

    #[inline]
    fn sub_symmetric_mat4(&self, rhs: &SymmetricDMat4) -> DMat4 {
        self.sub(rhs)
    }
}

macro_rules! symmetric_mat4s {
    ($($n:ident => $nonsymmetricn:ident, $vt:ident, $t:ident),+) => {
        $(
        /// The bottom left triangle (including the diagonal) of a symmetric 4x4 column-major matrix.
        ///
        /// This is useful for storing a symmetric 4x4 matrix in a more compact form and performing some
        /// matrix operations more efficiently.
        ///
        /// Some defining properties of symmetric matrices include:
        ///
        /// - The matrix is equal to its transpose.
        /// - The matrix has real eigenvalues.
        /// - The eigenvectors corresponding to the eigenvalues are orthogonal.
        /// - The matrix is always diagonalizable.
        ///
        /// The sum and difference of two symmetric matrices is always symmetric.
        /// However, the product of two symmetric matrices is *only* symmetric
        /// if the matrices are commutable, meaning that `AB = BA`.
        #[derive(Clone, Copy, PartialEq)]
        #[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(
            all(feature = "bevy_reflect", feature = "serde"),
            reflect(Debug, Default, PartialEq, Serialize, Deserialize)
        )]
        pub struct $n {
            /// The first element of the first column.
            pub m00: $t,
            /// The second element of the first column.
            pub m01: $t,
            /// The third element of the first column.
            pub m02: $t,
            /// The fourth element of the first column.
            pub m03: $t,
            /// The second element of the second column.
            pub m11: $t,
            /// The third element of the second column.
            pub m12: $t,
            /// The fourth element of the second column.
            pub m13: $t,
            /// The third element of the third column.
            pub m22: $t,
            /// The fourth element of the third column.
            pub m23: $t,
            /// The fourth element of the fourth column.
            pub m33: $t,
        }

        impl $n {
            /// A symmetric 4x4 matrix with all elements set to `0.0`.
            pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);

            /// A symmetric 4x4 identity matrix, where all diagonal elements are `1.0`,
            /// and all off-diagonal elements are `0.0`.
            pub const IDENTITY: Self = Self::new(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 1.0);

            /// All NaNs.
            pub const NAN: Self = Self::new(
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
                $t::NAN,
            );

            /// Creates a new symmetric 4x4 matrix from its bottom left triangle, including diagonal elements.
            ///
            /// The elements are in column-major order `mCR`, where `C` is the column index
            /// and `R` is the row index.
            #[inline(always)]
            #[must_use]
            #[expect(clippy::too_many_arguments, reason = "It's important to have a raw constructor for this.")]
            pub const fn new(
                m00: $t,
                m01: $t,
                m02: $t,
                m03: $t,
                m11: $t,
                m12: $t,
                m13: $t,
                m22: $t,
                m23: $t,
                m33: $t,
            ) -> Self {
                Self {
                    m00,
                    m01,
                    m02,
                    m03,
                    m11,
                    m12,
                    m13,
                    m22,
                    m23,
                    m33,
                }
            }

            /// Creates a symmetric 4x4 matrix from four column vectors.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline(always)]
            #[must_use]
            pub fn from_cols_unchecked(x_axis: $vt, y_axis: $vt, z_axis: $vt, w_axis: $vt) -> Self {
                Self {
                    m00: x_axis.x,
                    m01: x_axis.y,
                    m02: x_axis.z,
                    m03: w_axis.x,
                    m11: y_axis.y,
                    m12: y_axis.z,
                    m13: w_axis.y,
                    m22: z_axis.z,
                    m23: w_axis.z,
                    m33: w_axis.w,
                }
            }

            /// Creates a symmetric 4x4 matrix from an array stored in column major order.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline]
            #[must_use]
            pub const fn from_cols_array_unchecked(m: &[$t; 16]) -> Self {
                Self::new(m[0], m[1], m[2], m[3], m[5], m[6], m[7], m[10], m[11], m[15])
            }

            /// Creates an array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array(&self) -> [$t; 16] {
                [
                    self.m00, self.m01, self.m02, self.m03,
                    self.m01, self.m11, self.m12, self.m13,
                    self.m02, self.m12, self.m22, self.m23,
                    self.m03, self.m13, self.m23, self.m33,
                ]
            }

            /// Creates a symmetric 4x4 matrix from a 2D array stored in column major order.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline]
            #[must_use]
            pub fn from_cols_array_2d_unchecked(m: &[[$t; 4]; 4]) -> Self {
                Self::from_cols_unchecked(
                    $vt::from_array(m[0]),
                    $vt::from_array(m[1]),
                    $vt::from_array(m[2]),
                    $vt::from_array(m[3]),
                )
            }

            /// Creates a 2D array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array_2d(&self) -> [[$t; 4]; 4] {
                [
                    [self.m00, self.m01, self.m02, self.m03],
                    [self.m01, self.m11, self.m12, self.m13],
                    [self.m02, self.m12, self.m22, self.m23],
                    [self.m03, self.m13, self.m23, self.m33],
                ]
            }

            /// Creates a 4x4 matrix from the first 16 values in `slice`.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            ///
            /// # Panics
            ///
            /// Panics if `slice` is less than 16 elements long.
            #[inline]
            #[must_use]
            pub const fn from_cols_slice(slice: &[$t]) -> Self {
                Self::new(
                    slice[0],
                    slice[1],
                    slice[2],
                    slice[3],
                    slice[5],
                    slice[6],
                    slice[7],
                    slice[10],
                    slice[11],
                    slice[15],
                )
            }

            /// Creates a symmetric 4x4 matrix with its diagonal set to `diagonal` and all other entries set to `0.0`.
            #[inline]
            #[must_use]
            #[doc(alias = "scale")]
            pub fn from_diagonal(diagonal: $vt) -> Self {
                Self::new(
                    diagonal.x,
                    0.0,
                    0.0,
                    0.0,
                    diagonal.y,
                    0.0,
                    0.0,
                    diagonal.z,
                    0.0,
                    diagonal.w,
                )
            }

            /// Tries to create a symmetric 4x4 matrix from a 4x4 matrix.
            ///
            /// # Errors
            ///
            /// Returns a [`MatConversionError`] if the given matrix is not symmetric.
            #[inline]
            pub fn try_from_mat4(mat: $nonsymmetricn) -> Result<Self, MatConversionError> {
                if mat.is_symmetric() {
                    Ok(Self::from_mat4_unchecked(mat))
                } else {
                    Err(MatConversionError::Asymmetric)
                }
            }

            /// Creates a symmetric 4x4 matrix from a 4x4 matrix.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given matrix is truly symmetric.
            #[inline]
            #[must_use]
            pub fn from_mat4_unchecked(mat: $nonsymmetricn) -> Self {
                Self::new(
                    mat.x_axis.x,
                    mat.x_axis.y,
                    mat.x_axis.z,
                    mat.x_axis.w,
                    mat.y_axis.y,
                    mat.y_axis.z,
                    mat.y_axis.w,
                    mat.z_axis.z,
                    mat.z_axis.w,
                    mat.w_axis.w,
                )
            }

            /// Creates a 4x4 matrix from the symmetric 4x4 matrix in `self`.
            #[inline]
            #[must_use]
            pub const fn to_mat4(&self) -> $nonsymmetricn {
                $nonsymmetricn::from_cols_array(&self.to_cols_array())
            }

            /// Creates a new symmetric 4x4 matrix from the outer product `v * v^T`.
            #[inline(always)]
            #[must_use]
            pub fn from_outer_product(v: $vt) -> Self {
                Self::new(
                    v.x * v.x,
                    v.x * v.y,
                    v.x * v.z,
                    v.x * v.w,
                    v.y * v.y,
                    v.y * v.z,
                    v.y * v.w,
                    v.z * v.z,
                    v.z * v.w,
                    v.w * v.w,
                )
            }

            /// Returns the matrix column for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 3.
            #[inline]
            #[must_use]
            pub const fn col(&self, index: usize) -> $vt {
                match index {
                    0 => $vt::new(self.m00, self.m01, self.m02, self.m03),
                    1 => $vt::new(self.m01, self.m11, self.m12, self.m13),
                    2 => $vt::new(self.m02, self.m12, self.m22, self.m23),
                    3 => $vt::new(self.m03, self.m13, self.m23, self.m33),
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns the matrix row for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 3.
            #[inline]
            #[must_use]
            pub const fn row(&self, index: usize) -> $vt {
                match index {
                    0 => $vt::new(self.m00, self.m01, self.m02, self.m03),
                    1 => $vt::new(self.m01, self.m11, self.m12, self.m13),
                    2 => $vt::new(self.m02, self.m12, self.m22, self.m23),
                    3 => $vt::new(self.m03, self.m13, self.m23, self.m33),
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns the diagonal of the matrix.
            #[inline]
            #[must_use]
            pub fn diagonal(&self) -> $vt {
                $vt::new(self.m00, self.m11, self.m22, self.m33)
            }

            /// Returns `true` if, and only if, all elements are finite.
            /// If any element is either `NaN` or positive or negative infinity, this will return `false`.
            #[inline]
            #[must_use]
            pub fn is_finite(&self) -> bool {
                self.m00.is_finite()
                    && self.m01.is_finite()
                    && self.m02.is_finite()
                    && self.m03.is_finite()
                    && self.m11.is_finite()
                    && self.m12.is_finite()
                    && self.m13.is_finite()
                    && self.m22.is_finite()
                    && self.m23.is_finite()
                    && self.m33.is_finite()
            }

            /// Returns `true` if any elements are `NaN`.
            #[inline]
            #[must_use]
            pub fn is_nan(&self) -> bool {
                self.m00.is_nan()
                    || self.m01.is_nan()
                    || self.m02.is_nan()
                    || self.m03.is_nan()
                    || self.m11.is_nan()
                    || self.m12.is_nan()
                    || self.m13.is_nan()
                    || self.m22.is_nan()
                    || self.m23.is_nan()
                    || self.m33.is_nan()
            }

            /// Returns the determinant of `self`.
            #[inline]
            #[must_use]
            pub fn determinant(&self) -> $t {
                // Reference: Symmetric4x4Wide in Bepu
                // https://github.com/bepu/bepuphysics2/blob/1d1aead82c493c22793bc02a233a6dcd08b57bd6/BepuUtilities/Symmetric4x4Wide.cs#L62

                // TODO: This probably isn't as optimized as it could be.
                let s0 = self.m00 * self.m11 - self.m01 * self.m01;
                let s1 = self.m00 * self.m12 - self.m01 * self.m02;
                let s2 = self.m00 * self.m13 - self.m01 * self.m03;
                let s3 = self.m01 * self.m12 - self.m11 * self.m02;
                let s4 = self.m01 * self.m13 - self.m11 * self.m03;
                let s5 = self.m02 * self.m13 - self.m12 * self.m03;
                let c5 = self.m22 * self.m33 - self.m23 * self.m23;
                let c4 = self.m12 * self.m33 - self.m13 * self.m23;
                let c3 = self.m12 * self.m23 - self.m13 * self.m22;
                let c2 = self.m02 * self.m33 - self.m03 * self.m23;
                let c1 = self.m02 * self.m23 - self.m03 * self.m22;
                // let c0 = s5;

                s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * s5
            }

            /// Returns the inverse of `self`.
            ///
            /// If the matrix is not invertible the returned matrix will be invalid.
            #[inline]
            #[must_use]
            pub fn inverse(&self) -> Self {
                // Reference: Symmetric4x4Wide in Bepu
                // https://github.com/bepu/bepuphysics2/blob/1d1aead82c493c22793bc02a233a6dcd08b57bd6/BepuUtilities/Symmetric4x4Wide.cs#L62

                let s0 = self.m00 * self.m11 - self.m01 * self.m01;
                let s1 = self.m00 * self.m12 - self.m01 * self.m02;
                let s2 = self.m00 * self.m13 - self.m01 * self.m03;
                let s3 = self.m01 * self.m12 - self.m11 * self.m02;
                let s4 = self.m01 * self.m13 - self.m11 * self.m03;
                let s5 = self.m02 * self.m13 - self.m12 * self.m03;
                let c5 = self.m22 * self.m33 - self.m23 * self.m23;
                let c4 = self.m12 * self.m33 - self.m13 * self.m23;
                let c3 = self.m12 * self.m23 - self.m13 * self.m22;
                let c2 = self.m02 * self.m33 - self.m03 * self.m23;
                let c1 = self.m02 * self.m23 - self.m03 * self.m22;
                // let c0 = s5;

                let inverse_determinant = 1.0 / (s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * s5);

                let m00 = self.m11 * c5 - self.m12 * c4 + self.m13 * c3;
                let m01 = -self.m01 * c5 + self.m12 * c2 - self.m13 * c1;
                let m02 = self.m01 * c4 - self.m11 * c2 + self.m13 * s5;
                let m03 = -self.m01 * c3 + self.m11 * c1 - self.m12 * s5;
                let m11 = self.m00 * c5 - self.m02 * c2 + self.m03 * c1;
                let m12 = -self.m00 * c4 + self.m01 * c2 - self.m03 * s5;
                let m13 = self.m00 * c3 - self.m01 * c1 + self.m02 * s5;
                let m22 = self.m03 * s4 - self.m13 * s2 + self.m33 * s0;
                let m23 = -self.m03 * s3 + self.m13 * s1 - self.m23 * s0;
                let m33 = self.m02 * s3 - self.m12 * s1 + self.m22 * s0;

                Self {
                    m00: m00 * inverse_determinant,
                    m01: m01 * inverse_determinant,
                    m02: m02 * inverse_determinant,
                    m03: m03 * inverse_determinant,
                    m11: m11 * inverse_determinant,
                    m12: m12 * inverse_determinant,
                    m13: m13 * inverse_determinant,
                    m22: m22 * inverse_determinant,
                    m23: m23 * inverse_determinant,
                    m33: m33 * inverse_determinant,
                }
            }

            /// Returns the inverse of `self`, or a zero matrix if the matrix is not invertible.
            #[inline]
            #[must_use]
            pub fn inverse_or_zero(&self) -> Self {
                // Reference: Symmetric4x4Wide in Bepu
                // https://github.com/bepu/bepuphysics2/blob/1d1aead82c493c22793bc02a233a6dcd08b57bd6/BepuUtilities/Symmetric4x4Wide.cs#L62

                let s0 = self.m00 * self.m11 - self.m01 * self.m01;
                let s1 = self.m00 * self.m12 - self.m01 * self.m02;
                let s2 = self.m00 * self.m13 - self.m01 * self.m03;
                let s3 = self.m01 * self.m12 - self.m11 * self.m02;
                let s4 = self.m01 * self.m13 - self.m11 * self.m03;
                let s5 = self.m02 * self.m13 - self.m12 * self.m03;
                let c5 = self.m22 * self.m33 - self.m23 * self.m23;
                let c4 = self.m12 * self.m33 - self.m13 * self.m23;
                let c3 = self.m12 * self.m23 - self.m13 * self.m22;
                let c2 = self.m02 * self.m33 - self.m03 * self.m23;
                let c1 = self.m02 * self.m23 - self.m03 * self.m22;
                // let c0 = s5;

                let determinant = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * s5;

                if determinant == 0.0 {
                    return Self::ZERO;
                }

                let inverse_determinant = 1.0 / determinant;

                let m00 = self.m11 * c5 - self.m12 * c4 + self.m13 * c3;
                let m01 = -self.m01 * c5 + self.m12 * c2 - self.m13 * c1;
                let m02 = self.m01 * c4 - self.m11 * c2 + self.m13 * s5;
                let m03 = -self.m01 * c3 + self.m11 * c1 - self.m12 * s5;
                let m11 = self.m00 * c5 - self.m02 * c2 + self.m03 * c1;
                let m12 = -self.m00 * c4 + self.m01 * c2 - self.m03 * s5;
                let m13 = self.m00 * c3 - self.m01 * c1 + self.m02 * s5;
                let m22 = self.m03 * s4 - self.m13 * s2 + self.m33 * s0;
                let m23 = -self.m03 * s3 + self.m13 * s1 - self.m23 * s0;
                let m33 = self.m02 * s3 - self.m12 * s1 + self.m22 * s0;

                Self {
                    m00: m00 * inverse_determinant,
                    m01: m01 * inverse_determinant,
                    m02: m02 * inverse_determinant,
                    m03: m03 * inverse_determinant,
                    m11: m11 * inverse_determinant,
                    m12: m12 * inverse_determinant,
                    m13: m13 * inverse_determinant,
                    m22: m22 * inverse_determinant,
                    m23: m23 * inverse_determinant,
                    m33: m33 * inverse_determinant,
                }
            }

            /// Takes the absolute value of each element in `self`.
            #[inline]
            #[must_use]
            pub fn abs(&self) -> Self {
                Self::new(
                    FloatAbs::abs(self.m00),
                    FloatAbs::abs(self.m01),
                    FloatAbs::abs(self.m02),
                    FloatAbs::abs(self.m03),
                    FloatAbs::abs(self.m11),
                    FloatAbs::abs(self.m12),
                    FloatAbs::abs(self.m13),
                    FloatAbs::abs(self.m22),
                    FloatAbs::abs(self.m23),
                    FloatAbs::abs(self.m33),
                )
            }

            /// Transforms a 4D vector.
            #[inline]
            #[must_use]
            pub fn mul_vec4(&self, rhs: $vt) -> $vt {
                let mut res = self.col(0).mul(rhs.x);
                res = res.add(self.col(1).mul(rhs.y));
                res = res.add(self.col(2).mul(rhs.z));
                res = res.add(self.col(3).mul(rhs.w));
                res
            }

            /// Multiplies two 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn mul_mat4(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.mul(rhs)
            }

            /// Adds two 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn add_mat4(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.add(rhs)
            }

            /// Subtracts two 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn sub_mat4(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.sub(rhs)
            }

            /// Multiplies two symmetric 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn mul_symmetric_mat4(&self, rhs: &Self) -> $nonsymmetricn {
                self.mul(rhs)
            }

            /// Adds two symmetric 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn add_symmetric_mat4(&self, rhs: &Self) -> Self {
                self.add(rhs)
            }

            /// Subtracts two symmetric 4x4 matrices.
            #[inline]
            #[must_use]
            pub fn sub_symmetric_mat4(&self, rhs: &Self) -> Self {
                self.sub(rhs)
            }

            /// Multiplies a 4x4 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn mul_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.m00 * rhs,
                    self.m01 * rhs,
                    self.m02 * rhs,
                    self.m03 * rhs,
                    self.m11 * rhs,
                    self.m12 * rhs,
                    self.m13 * rhs,
                    self.m22 * rhs,
                    self.m23 * rhs,
                    self.m33 * rhs,
                )
            }

            /// Divides a 4x4 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn div_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.m00 / rhs,
                    self.m01 / rhs,
                    self.m02 / rhs,
                    self.m03 / rhs,
                    self.m11 / rhs,
                    self.m12 / rhs,
                    self.m13 / rhs,
                    self.m22 / rhs,
                    self.m23 / rhs,
                    self.m33 / rhs,
                )
            }
        }

        impl Default for $n {
            #[inline(always)]
            fn default() -> Self {
                Self::IDENTITY
            }
        }

        impl TryFrom<$nonsymmetricn> for $n {
            type Error = MatConversionError;

            #[inline]
            fn try_from(mat: $nonsymmetricn) -> Result<Self, Self::Error> {
                Self::try_from_mat4(mat)
            }
        }

        impl Add for $n {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self::new(
                    self.m00 + rhs.m00,
                    self.m01 + rhs.m01,
                    self.m02 + rhs.m02,
                    self.m03 + rhs.m03,
                    self.m11 + rhs.m11,
                    self.m12 + rhs.m12,
                    self.m13 + rhs.m13,
                    self.m22 + rhs.m22,
                    self.m23 + rhs.m23,
                    self.m33 + rhs.m33,
                )
            }
        }

        impl Add<&Self> for $n {
            type Output = Self;
            #[inline]
            fn add(self, rhs: &Self) -> Self::Output {
                self.add(*rhs)
            }
        }

        impl Add<Self> for &$n {
            type Output = $n;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                (*self).add(rhs)
            }
        }

        impl Add<&Self> for &$n {
            type Output = $n;
            #[inline]
            fn add(self, rhs: &Self) -> Self::Output {
                (*self).add(*rhs)
            }
        }

        impl AddAssign for $n {
            #[inline]
            fn add_assign(&mut self, rhs: Self) {
                *self = self.add(rhs);
            }
        }

        impl AddAssign<&Self> for $n {
            #[inline]
            fn add_assign(&mut self, rhs: &Self) {
                self.add_assign(*rhs);
            }
        }

        impl Add<$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: $nonsymmetricn) -> Self::Output {
                $nonsymmetricn::from_cols(
                    self.col(0).add(rhs.x_axis),
                    self.col(1).add(rhs.y_axis),
                    self.col(2).add(rhs.z_axis),
                    self.col(3).add(rhs.w_axis),
                )
            }
        }

        impl Add<&$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: &$nonsymmetricn) -> Self::Output {
                self.add(*rhs)
            }
        }

        impl Add<$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: $nonsymmetricn) -> Self::Output {
                (*self).add(rhs)
            }
        }

        impl Add<&$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: &$nonsymmetricn) -> Self::Output {
                (*self).add(*rhs)
            }
        }

        impl Add<$n> for $nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: $n) -> Self::Output {
                rhs.add(&self)
            }
        }

        impl Add<&$n> for $nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: &$n) -> Self::Output {
                self.add(*rhs)
            }
        }

        impl Add<&$n> for &$nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn add(self, rhs: &$n) -> Self::Output {
                (*self).add(*rhs)
            }
        }

        impl AddAssign<$n> for $nonsymmetricn {
            #[inline]
            fn add_assign(&mut self, rhs: $n) {
                *self = self.add(rhs);
            }
        }

        impl AddAssign<&$n> for $nonsymmetricn {
            #[inline]
            fn add_assign(&mut self, rhs: &$n) {
                *self = self.add(*rhs);
            }
        }

        impl Sub for $n {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self::new(
                    self.m00 - rhs.m00,
                    self.m01 - rhs.m01,
                    self.m02 - rhs.m02,
                    self.m03 - rhs.m03,
                    self.m11 - rhs.m11,
                    self.m12 - rhs.m12,
                    self.m13 - rhs.m13,
                    self.m22 - rhs.m22,
                    self.m23 - rhs.m23,
                    self.m33 - rhs.m33,
                )
            }
        }

        impl Sub<&Self> for $n {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: &Self) -> Self::Output {
                self.sub(*rhs)
            }
        }

        impl Sub<Self> for &$n {
            type Output = $n;
            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                (*self).sub(rhs)
            }
        }

        impl Sub<&Self> for &$n {
            type Output = $n;
            #[inline]
            fn sub(self, rhs: &Self) -> Self::Output {
                (*self).sub(*rhs)
            }
        }

        impl SubAssign for $n {
            #[inline]
            fn sub_assign(&mut self, rhs: Self) {
                *self = self.sub(rhs);
            }
        }

        impl SubAssign<&Self> for $n {
            #[inline]
            fn sub_assign(&mut self, rhs: &Self) {
                self.sub_assign(*rhs);
            }
        }

        impl Sub<$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: $nonsymmetricn) -> Self::Output {
                $nonsymmetricn::from_cols(
                    self.col(0).sub(rhs.x_axis),
                    self.col(1).sub(rhs.y_axis),
                    self.col(2).sub(rhs.z_axis),
                    self.col(3).sub(rhs.w_axis),
                )
            }
        }

        impl Sub<&$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: &$nonsymmetricn) -> Self::Output {
                self.sub(*rhs)
            }
        }

        impl Sub<$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: $nonsymmetricn) -> Self::Output {
                (*self).sub(rhs)
            }
        }

        impl Sub<&$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: &$nonsymmetricn) -> Self::Output {
                (*self).sub(*rhs)
            }
        }

        impl Sub<$n> for $nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: $n) -> Self::Output {
                rhs.sub(&self)
            }
        }

        impl Sub<&$n> for $nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: &$n) -> Self::Output {
                self.sub(*rhs)
            }
        }

        impl Sub<&$n> for &$nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn sub(self, rhs: &$n) -> Self::Output {
                (*self).sub(*rhs)
            }
        }

        impl SubAssign<$n> for $nonsymmetricn {
            #[inline]
            fn sub_assign(&mut self, rhs: $n) {
                *self = self.sub(rhs);
            }
        }

        impl SubAssign<&$n> for $nonsymmetricn {
            #[inline]
            fn sub_assign(&mut self, rhs: &$n) {
                *self = self.sub(*rhs);
            }
        }

        impl Neg for $n {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self::Output {
                Self::new(
                    -self.m00,
                    -self.m01,
                    -self.m02,
                    -self.m03,
                    -self.m11,
                    -self.m12,
                    -self.m13,
                    -self.m22,
                    -self.m23,
                    -self.m33,
                )
            }
        }

        impl Neg for &$n {
            type Output = $n;
            #[inline]
            fn neg(self) -> Self::Output {
                (*self).neg()
            }
        }

        impl Mul for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: Self) -> Self::Output {
                $nonsymmetricn::from_cols(
                    self.mul(rhs.col(0)),
                    self.mul(rhs.col(1)),
                    self.mul(rhs.col(2)),
                    self.mul(rhs.col(3)),
                )
            }
        }

        impl Mul<&Self> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: &Self) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<Self> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: Self) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&Self> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: &Self) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$n> for $nonsymmetricn {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: $n) -> Self::Output {
                Self::from_cols_array_2d(&[
                    [
                        self.x_axis.x * rhs.m00 + self.y_axis.x * rhs.m01 + self.z_axis.x * rhs.m02 + self.w_axis.x * rhs.m03,
                        self.x_axis.y * rhs.m00 + self.y_axis.y * rhs.m01 + self.z_axis.y * rhs.m02 + self.w_axis.y * rhs.m03,
                        self.x_axis.z * rhs.m00 + self.y_axis.z * rhs.m01 + self.z_axis.z * rhs.m02 + self.w_axis.z * rhs.m03,
                        self.x_axis.w * rhs.m00 + self.y_axis.w * rhs.m01 + self.z_axis.w * rhs.m02 + self.w_axis.w * rhs.m03,
                    ],
                    [
                        self.x_axis.x * rhs.m01 + self.y_axis.x * rhs.m11 + self.z_axis.x * rhs.m12 + self.w_axis.x * rhs.m13,
                        self.x_axis.y * rhs.m01 + self.y_axis.y * rhs.m11 + self.z_axis.y * rhs.m12 + self.w_axis.y * rhs.m13,
                        self.x_axis.z * rhs.m01 + self.y_axis.z * rhs.m11 + self.z_axis.z * rhs.m12 + self.w_axis.z * rhs.m13,
                        self.x_axis.w * rhs.m01 + self.y_axis.w * rhs.m11 + self.z_axis.w * rhs.m12 + self.w_axis.w * rhs.m13,
                    ],
                    [
                        self.x_axis.x * rhs.m02 + self.y_axis.x * rhs.m12 + self.z_axis.x * rhs.m22 + self.w_axis.x * rhs.m23,
                        self.x_axis.y * rhs.m02 + self.y_axis.y * rhs.m12 + self.z_axis.y * rhs.m22 + self.w_axis.y * rhs.m23,
                        self.x_axis.z * rhs.m02 + self.y_axis.z * rhs.m12 + self.z_axis.z * rhs.m22 + self.w_axis.z * rhs.m23,
                        self.x_axis.w * rhs.m02 + self.y_axis.w * rhs.m12 + self.z_axis.w * rhs.m22 + self.w_axis.w * rhs.m23,
                    ],
                    [
                        self.x_axis.x * rhs.m03 + self.y_axis.x * rhs.m13 + self.z_axis.x * rhs.m23 + self.w_axis.x * rhs.m33,
                        self.x_axis.y * rhs.m03 + self.y_axis.y * rhs.m13 + self.z_axis.y * rhs.m23 + self.w_axis.y * rhs.m33,
                        self.x_axis.z * rhs.m03 + self.y_axis.z * rhs.m13 + self.z_axis.z * rhs.m23 + self.w_axis.z * rhs.m33,
                        self.x_axis.w * rhs.m03 + self.y_axis.w * rhs.m13 + self.z_axis.w * rhs.m23 + self.w_axis.w * rhs.m33,
                    ],
                ])
            }
        }

        impl Mul<&$n> for $nonsymmetricn {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: &$n) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$n> for &$nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: $n) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$n> for &$nonsymmetricn {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: &$n) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl MulAssign<$n> for $nonsymmetricn {
            #[inline]
            fn mul_assign(&mut self, rhs: $n) {
                *self = self.mul(rhs);
            }
        }

        impl MulAssign<&$n> for $nonsymmetricn {
            #[inline]
            fn mul_assign(&mut self, rhs: &$n) {
                *self = self.mul(*rhs);
            }
        }

        impl Mul<$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: $nonsymmetricn) -> Self::Output {
                $nonsymmetricn::from_cols(
                    self.mul(rhs.x_axis),
                    self.mul(rhs.y_axis),
                    self.mul(rhs.z_axis),
                    self.mul(rhs.w_axis),
                )
            }
        }

        impl Mul<&$nonsymmetricn> for $n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: &$nonsymmetricn) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: $nonsymmetricn) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$nonsymmetricn> for &$n {
            type Output = $nonsymmetricn;
            #[inline]
            fn mul(self, rhs: &$nonsymmetricn) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$vt> for $n {
            type Output = $vt;
            #[inline]
            fn mul(self, rhs: $vt) -> Self::Output {
                self.mul_vec4(rhs)
            }
        }

        impl Mul<&$vt> for $n {
            type Output = $vt;
            #[inline]
            fn mul(self, rhs: &$vt) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$vt> for &$n {
            type Output = $vt;
            #[inline]
            fn mul(self, rhs: $vt) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$vt> for &$n {
            type Output = $vt;
            #[inline]
            fn mul(self, rhs: &$vt) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$n> for $t {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: $n) -> Self::Output {
                rhs.mul_scalar(self)
            }
        }

        impl Mul<&$n> for $t {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: &$n) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$n> for &$t {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: $n) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$n> for &$t {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: &$n) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: $t) -> Self::Output {
                self.mul_scalar(rhs)
            }
        }

        impl Mul<&$t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: &$t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: $t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: &$t) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl MulAssign<$t> for $n {
            #[inline]
            fn mul_assign(&mut self, rhs: $t) {
                *self = self.mul(rhs);
            }
        }

        impl MulAssign<&$t> for $n {
            #[inline]
            fn mul_assign(&mut self, rhs: &$t) {
                self.mul_assign(*rhs);
            }
        }

        impl Div<$n> for $t {
            type Output = $n;
            #[inline]
            fn div(self, rhs: $n) -> Self::Output {
                rhs.div_scalar(self)
            }
        }

        impl Div<&$n> for $t {
            type Output = $n;
            #[inline]
            fn div(self, rhs: &$n) -> Self::Output {
                self.div(*rhs)
            }
        }

        impl Div<$n> for &$t {
            type Output = $n;
            #[inline]
            fn div(self, rhs: $n) -> Self::Output {
                (*self).div(rhs)
            }
        }

        impl Div<&$n> for &$t {
            type Output = $n;
            #[inline]
            fn div(self, rhs: &$n) -> Self::Output {
                (*self).div(*rhs)
            }
        }

        impl Div<$t> for $n {
            type Output = Self;
            #[inline]
            fn div(self, rhs: $t) -> Self::Output {
                self.div_scalar(rhs)
            }
        }

        impl Div<&$t> for $n {
            type Output = Self;
            #[inline]
            fn div(self, rhs: &$t) -> Self::Output {
                self.div(*rhs)
            }
        }

        impl Div<$t> for &$n {
            type Output = $n;
            #[inline]
            fn div(self, rhs: $t) -> Self::Output {
                (*self).div(rhs)
            }
        }

        impl Div<&$t> for &$n {
            type Output = $n;
            #[inline]
            fn div(self, rhs: &$t) -> Self::Output {
                (*self).div(*rhs)
            }
        }

        impl DivAssign<$t> for $n {
            #[inline]
            fn div_assign(&mut self, rhs: $t) {
                *self = self.div(rhs);
            }
        }

        impl DivAssign<&$t> for $n {
            #[inline]
            fn div_assign(&mut self, rhs: &$t) {
                self.div_assign(*rhs);
            }
        }

        impl From<$n> for $nonsymmetricn {
            #[inline]
            fn from(mat: $n) -> Self {
                mat.to_mat4()
            }
        }

        impl Sum<$n> for $n {
            fn sum<I: Iterator<Item = $n>>(iter: I) -> Self {
                iter.fold(Self::ZERO, Self::add)
            }
        }

        impl<'a> Sum<&'a $n> for $n {
            fn sum<I: Iterator<Item = &'a $n>>(iter: I) -> Self {
                iter.fold(Self::ZERO, |a, &b| a.add(b))
            }
        }

        #[cfg(feature = "approx")]
        impl approx::AbsDiffEq for $n {
            type Epsilon = $t;

            #[inline]
            fn default_epsilon() -> Self::Epsilon {
                $t::default_epsilon()
            }

            #[inline]
            fn abs_diff_eq(&self, other: &Self, epsilon: Self::Epsilon) -> bool {
                self.m00.abs_diff_eq(&other.m00, epsilon)
                    && self.m01.abs_diff_eq(&other.m01, epsilon)
                    && self.m02.abs_diff_eq(&other.m02, epsilon)
                    && self.m03.abs_diff_eq(&other.m03, epsilon)
                    && self.m11.abs_diff_eq(&other.m11, epsilon)
                    && self.m12.abs_diff_eq(&other.m12, epsilon)
                    && self.m13.abs_diff_eq(&other.m13, epsilon)
                    && self.m22.abs_diff_eq(&other.m22, epsilon)
                    && self.m23.abs_diff_eq(&other.m23, epsilon)
                    && self.m33.abs_diff_eq(&other.m33, epsilon)
            }
        }

        #[cfg(feature = "approx")]
        impl approx::RelativeEq for $n {
            #[inline]
            fn default_max_relative() -> Self::Epsilon {
                $t::default_max_relative()
            }

            #[inline]
            fn relative_eq(
                &self,
                other: &Self,
                epsilon: Self::Epsilon,
                max_relative: Self::Epsilon,
            ) -> bool {
                self.m00.relative_eq(&other.m00, epsilon, max_relative)
                    && self.m01.relative_eq(&other.m01, epsilon, max_relative)
                    && self.m02.relative_eq(&other.m02, epsilon, max_relative)
                    && self.m03.relative_eq(&other.m03, epsilon, max_relative)
                    && self.m11.relative_eq(&other.m11, epsilon, max_relative)
                    && self.m12.relative_eq(&other.m12, epsilon, max_relative)
                    && self.m13.relative_eq(&other.m13, epsilon, max_relative)
                    && self.m22.relative_eq(&other.m22, epsilon, max_relative)
                    && self.m23.relative_eq(&other.m23, epsilon, max_relative)
                    && self.m33.relative_eq(&other.m33, epsilon, max_relative)
            }
        }

        #[cfg(feature = "approx")]
        impl approx::UlpsEq for $n {
            #[inline]
            fn default_max_ulps() -> u32 {
                $t::default_max_ulps()
            }

            #[inline]
            fn ulps_eq(
                &self,
                other: &Self,
                epsilon: Self::Epsilon,
                max_ulps: u32,
            ) -> bool {
                self.m00.ulps_eq(&other.m00, epsilon, max_ulps)
                    && self.m01.ulps_eq(&other.m01, epsilon, max_ulps)
                    && self.m02.ulps_eq(&other.m02, epsilon, max_ulps)
                    && self.m03.ulps_eq(&other.m03, epsilon, max_ulps)
                    && self.m11.ulps_eq(&other.m11, epsilon, max_ulps)
                    && self.m12.ulps_eq(&other.m12, epsilon, max_ulps)
                    && self.m13.ulps_eq(&other.m13, epsilon, max_ulps)
                    && self.m22.ulps_eq(&other.m22, epsilon, max_ulps)
                    && self.m23.ulps_eq(&other.m23, epsilon, max_ulps)
                    && self.m33.ulps_eq(&other.m33, epsilon, max_ulps)
            }
        }

        impl core::fmt::Debug for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($n))
                    .field("m00", &self.m00)
                    .field("m01", &self.m01)
                    .field("m02", &self.m02)
                    .field("m03", &self.m03)
                    .field("m11", &self.m11)
                    .field("m12", &self.m12)
                    .field("m13", &self.m13)
                    .field("m22", &self.m22)
                    .field("m23", &self.m23)
                    .field("m33", &self.m33)
                    .finish()
            }
        }

        impl core::fmt::Display for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if let Some(p) = f.precision() {
                    write!(
                        f,
                        "[[{:.*}, {:.*}, {:.*}, {:.*}], [{:.*}, {:.*}, {:.*}, {:.*}], [{:.*}, {:.*}, {:.*}, {:.*}], [{:.*}, {:.*}, {:.*}, {:.*}]]",
                        p, self.m00, p, self.m01, p, self.m02, p, self.m03,
                        p, self.m01, p, self.m11, p, self.m12, p, self.m13,
                        p, self.m02, p, self.m12, p, self.m22, p, self.m23,
                        p, self.m03, p, self.m13, p, self.m23, p, self.m33,
                    )
                } else {
                    write!(
                        f,
                        "[[{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}], [{}, {}, {}, {}]]",
                        self.m00, self.m01, self.m02, self.m03,
                        self.m01, self.m11, self.m12, self.m13,
                        self.m02, self.m12, self.m22, self.m23,
                        self.m03, self.m13, self.m23, self.m33,
                    )
                }
            }
        }
        )+
    }
}

#[cfg(feature = "f32")]
symmetric_mat4s!(SymmetricMat4 => Mat4, Vec4, f32);

#[cfg(feature = "f64")]
symmetric_mat4s!(SymmetricDMat4 => DMat4, DVec4, f64);

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricMat4 {
    /// Returns the double precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_dmat4(&self) -> SymmetricDMat4 {
        SymmetricDMat4 {
            m00: self.m00 as f64,
            m01: self.m01 as f64,
            m02: self.m02 as f64,
            m03: self.m03 as f64,
            m11: self.m11 as f64,
            m12: self.m12 as f64,
            m13: self.m13 as f64,
            m22: self.m22 as f64,
            m23: self.m23 as f64,
            m33: self.m33 as f64,
        }
    }
}

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricDMat4 {
    /// Returns the single precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_mat4(&self) -> SymmetricMat4 {
        SymmetricMat4 {
            m00: self.m00 as f32,
            m01: self.m01 as f32,
            m02: self.m02 as f32,
            m03: self.m03 as f32,
            m11: self.m11 as f32,
            m12: self.m12 as f32,
            m13: self.m13 as f32,
            m22: self.m22 as f32,
            m23: self.m23 as f32,
            m33: self.m33 as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;

    use crate::SymmetricMat4;

    #[test]
    fn determinant() {
        let mat = SymmetricMat4::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0);
        assert_relative_eq!(mat.determinant(), mat.to_mat4().determinant());
    }

    #[test]
    fn inverse() {
        let mat = SymmetricMat4::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0);
        assert_relative_eq!(mat.inverse().to_mat4(), mat.to_mat4().inverse());
    }
}
