use core::iter::Sum;
use core::ops::*;
#[cfg(feature = "f64")]
use glam::{DMat2, DVec2};
use glam::{Mat2, Vec2};

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize, std_traits::ReflectDefault};

#[cfg(feature = "f64")]
use crate::{DMat23, DMat32};
#[cfg(feature = "f32")]
use crate::{Mat23, Mat32};
use crate::{MatConversionError, SquareMatExt, ops::FloatAbs};

/// An extension trait for 2x2 matrices.
pub trait Mat2Ext {
    /// The type of the symmetric 2x2 matrix.
    type SymmetricMat2;

    /// Multiplies `self` by a symmetric 2x2 matrix.
    fn mul_symmetric_mat2(&self, rhs: &Self::SymmetricMat2) -> Self;

    /// Adds a symmetric 2x2 matrix to `self`.
    fn add_symmetric_mat2(&self, rhs: &Self::SymmetricMat2) -> Self;

    /// Subtracts a symmetric 2x2 matrix from `self`.
    fn sub_symmetric_mat2(&self, rhs: &Self::SymmetricMat2) -> Self;
}

#[cfg(feature = "f32")]
impl Mat2Ext for Mat2 {
    type SymmetricMat2 = SymmetricMat2;

    #[inline]
    fn mul_symmetric_mat2(&self, rhs: &SymmetricMat2) -> Mat2 {
        self.mul(rhs)
    }

    #[inline]
    fn add_symmetric_mat2(&self, rhs: &SymmetricMat2) -> Mat2 {
        self.add(rhs)
    }

    #[inline]
    fn sub_symmetric_mat2(&self, rhs: &SymmetricMat2) -> Mat2 {
        self.sub(rhs)
    }
}

#[cfg(feature = "f64")]
impl Mat2Ext for DMat2 {
    type SymmetricMat2 = SymmetricDMat2;

    #[inline]
    fn mul_symmetric_mat2(&self, rhs: &SymmetricDMat2) -> DMat2 {
        self.mul(rhs)
    }

    #[inline]
    fn add_symmetric_mat2(&self, rhs: &SymmetricDMat2) -> DMat2 {
        self.add(rhs)
    }

    #[inline]
    fn sub_symmetric_mat2(&self, rhs: &SymmetricDMat2) -> DMat2 {
        self.sub(rhs)
    }
}

macro_rules! symmetric_mat2s {
    ($($n:ident => $nonsymmetricn:ident, $m23t:ident, $m32t:ident, $vt:ident, $t:ident),+) => {
        $(
        /// The bottom left triangle (including the diagonal) of a symmetric 2x2 column-major matrix.
        ///
        /// This is useful for storing a symmetric 2x2 matrix in a more compact form and performing some
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
            /// The second element of the second column.
            pub m11: $t,
        }

        impl $n {
            /// A symmetric 2x2 matrix with all elements set to `0.0`.
            pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);

            /// A symmetric 2x2 identity matrix, where all diagonal elements are `1.0`,
            /// and all off-diagonal elements are `0.0`.
            pub const IDENTITY: Self = Self::new(1.0, 0.0, 1.0);

            /// All NaNs.
            pub const NAN: Self = Self::new($t::NAN, $t::NAN, $t::NAN);

            /// Creates a new symmetric 2x2 matrix from its bottom left triangle, including diagonal elements.
            ///
            /// The elements are in column-major order `mCR`, where `C` is the column index
            /// and `R` is the row index.
            #[inline(always)]
            #[must_use]
            pub const fn new(
                m00: $t,
                m01: $t,
                m11: $t,
            ) -> Self {
                Self { m00, m01, m11 }
            }

            /// Creates a symmetric 2x2 matrix from three column vectors.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline(always)]
            #[must_use]
            pub const fn from_cols_unchecked(x_axis: $vt, y_axis: $vt) -> Self {
                Self {
                    m00: x_axis.x,
                    m01: x_axis.y,
                    m11: y_axis.y,
                }
            }

            /// Creates a symmetric 2x2 matrix from an array stored in column major order.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline]
            #[must_use]
            pub const fn from_cols_array_unchecked(m: &[$t; 4]) -> Self {
                Self::new(m[0], m[1], m[3])
            }

            /// Creates an array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array(&self) -> [$t; 4] {
                [self.m00, self.m01, self.m01, self.m11]
            }

            /// Creates a symmetric 2x2 matrix from a 2D array stored in column major order.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            #[inline]
            #[must_use]
            pub const fn from_cols_array_2d_unchecked(m: &[[$t; 2]; 2]) -> Self {
                Self::from_cols_unchecked(
                    $vt::from_array(m[0]),
                    $vt::from_array(m[1]),
                )
            }

            /// Creates a 2D array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array_2d(&self) -> [[$t; 2]; 2] {
                [[self.m00, self.m01], [self.m01, self.m11]]
            }

            /// Creates a 2x2 matrix from the first 4 values in `slice`.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given columns truly produce a symmetric matrix.
            ///
            /// # Panics
            ///
            /// Panics if `slice` is less than 4 elements long.
            #[inline]
            #[must_use]
            pub const fn from_cols_slice(slice: &[$t]) -> Self {
                Self::new(slice[0], slice[1], slice[3])
            }

            /// Creates a symmetric 2x2 matrix with its diagonal set to `diagonal` and all other entries set to `0.0`.
            #[inline]
            #[must_use]
            #[doc(alias = "scale")]
            pub const fn from_diagonal(diagonal: $vt) -> Self {
                Self::new(diagonal.x, 0.0, diagonal.y)
            }

            /// Tries to create a symmetric 2x2 matrix from a 2x2 matrix.
            ///
            /// # Errors
            ///
            /// Returns a [`MatConversionError`] if the given matrix is not symmetric.
            #[inline]
            pub fn try_from_mat2(mat: $nonsymmetricn) -> Result<Self, MatConversionError> {
                if mat.is_symmetric() {
                    Ok(Self::from_mat2_unchecked(mat))
                } else {
                    Err(MatConversionError::Asymmetric)
                }
            }

            /// Creates a symmetric 2x2 matrix from a 2x2 matrix.
            ///
            /// Only the lower left triangle of the matrix is used. No check is performed to ensure
            /// that the given matrix is truly symmetric.
            #[inline]
            #[must_use]
            pub fn from_mat2_unchecked(mat: $nonsymmetricn) -> Self {
                Self::new(
                    mat.x_axis.x,
                    mat.x_axis.y,
                    mat.y_axis.y,
                )
            }

            /// Creates a 2x2 matrix from the symmetric 2x2 matrix in `self`.
            #[inline]
            #[must_use]
            pub const fn to_mat2(&self) -> $nonsymmetricn {
                $nonsymmetricn::from_cols_array(&self.to_cols_array())
            }

            /// Creates a new symmetric 2x2 matrix from the outer product `v * v^T`.
            #[inline(always)]
            #[must_use]
            pub fn from_outer_product(v: $vt) -> Self {
                Self::new(v.x * v.x, v.x * v.y, v.y * v.y)
            }

            /// Returns the matrix column for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 1.
            #[inline]
            #[must_use]
            pub const fn col(&self, index: usize) -> $vt {
                match index {
                    0 => $vt::new(self.m00, self.m01),
                    1 => $vt::new(self.m01, self.m11),
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns the matrix row for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 1.
            #[inline]
            #[must_use]
            pub const fn row(&self, index: usize) -> $vt {
                match index {
                    0 => $vt::new(self.m00, self.m01),
                    1 => $vt::new(self.m01, self.m11),
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns the diagonal of the matrix.
            #[inline]
            #[must_use]
            pub fn diagonal(&self) -> $vt {
                $vt::new(self.m00, self.m11)
            }

            /// Returns `true` if, and only if, all elements are finite.
            /// If any element is either `NaN` or positive or negative infinity, this will return `false`.
            #[inline]
            #[must_use]
            pub fn is_finite(&self) -> bool {
                self.m00.is_finite() && self.m01.is_finite() && self.m11.is_finite()
            }

            /// Returns `true` if any elements are `NaN`.
            #[inline]
            #[must_use]
            pub fn is_nan(&self) -> bool {
                self.m00.is_nan() || self.m01.is_nan() || self.m11.is_nan()
            }

            /// Returns the determinant of `self`.
            #[inline]
            #[must_use]
            pub fn determinant(&self) -> $t {
                // A = [ a c ]
                //     | c b |
                //
                // det(A) = ab - c^2
                let [a, b, c] = [self.m00, self.m11, self.m01];
                a * b - c * c
            }

            /// Returns the inverse of `self`.
            ///
            /// If the matrix is not invertible the returned matrix will be invalid.
            #[inline]
            #[must_use]
            pub fn inverse(&self) -> Self {
                let inv_det = 1.0 / self.determinant();
                Self {
                    m00: self.m11 * inv_det,
                    m01: -self.m01 * inv_det,
                    m11: self.m00 * inv_det,
                }
            }

            /// Returns the inverse of `self`, or a zero matrix if the matrix is not invertible.
            #[inline]
            #[must_use]
            pub fn inverse_or_zero(&self) -> Self {
                let det = self.determinant();
                if det == 0.0 {
                    Self::ZERO
                } else {
                    let inv_det = 1.0 / det;
                    Self {
                        m00: self.m11 * inv_det,
                        m01: -self.m01 * inv_det,
                        m11: self.m00 * inv_det,
                    }
                }
            }

            /// Takes the absolute value of each element in `self`.
            #[inline]
            #[must_use]
            pub fn abs(&self) -> Self {
                Self::new(
                    FloatAbs::abs(self.m00),
                    FloatAbs::abs(self.m01),
                    FloatAbs::abs(self.m11),
                )
            }

            /// Transforms a 2D vector.
            #[inline]
            #[must_use]
            pub fn mul_vec2(&self, rhs: $vt) -> $vt {
                let mut res = self.col(0).mul(rhs.x);
                res = res.add(self.col(1).mul(rhs.y));
                res
            }

            /// Multiplies two 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn mul_mat2(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.mul(rhs)
            }

            /// Multiplies `self` by a 2x3 matrix, `self * rhs`.
            #[inline]
            #[must_use]
            pub fn mul_mat23(&self, rhs: &$m23t) -> $m23t {
                self.mul(rhs)
            }

            /// Computes `a * transpose(b)`, assuming `a = b * M` for some symmetric matrix `M`.
            ///
            /// This effectively completes the second half of the sandwich product `b * M * transpose(b)`.
            #[inline]
            #[must_use]
            pub fn complete_mat23_sandwich(a: &$m23t, b: &$m23t) -> Self {
                Self::new(
                    a.row(0).dot(b.row(0)),
                    a.row(1).dot(b.row(0)),
                    a.row(1).dot(b.row(1)),
                )
            }

            /// Computes `a * transpose(b)`, assuming `a = b * M` for some symmetric matrix `M`.
            ///
            /// This effectively completes the second half of the sandwich product `b * M * transpose(b)`.
            #[inline]
            #[must_use]
            pub fn complete_mat32_sandwich(a: &$m32t, b: &$m32t) -> Self {
                Self::new(
                    a.col(0).dot(b.col(0)),
                    a.col(1).dot(b.col(0)),
                    a.col(1).dot(b.col(1)),
                )
            }

            /// Adds two 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn add_mat2(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.add(rhs)
            }

            /// Subtracts two 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn sub_mat2(&self, rhs: &$nonsymmetricn) -> $nonsymmetricn {
                self.sub(rhs)
            }

            /// Multiplies two symmetric 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn mul_symmetric_mat2(&self, rhs: &Self) -> $nonsymmetricn {
                self.mul(rhs)
            }

            /// Adds two symmetric 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn add_symmetric_mat2(&self, rhs: &Self) -> Self {
                self.add(rhs)
            }

            /// Subtracts two symmetric 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn sub_symmetric_mat2(&self, rhs: &Self) -> Self {
                self.sub(rhs)
            }

            /// Multiplies a 2x2 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn mul_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.m00 * rhs,
                    self.m01 * rhs,
                    self.m11 * rhs,
                )
            }

            /// Divides a 2x2 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn div_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.m00 / rhs,
                    self.m01 / rhs,
                    self.m11 / rhs,
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
                Self::try_from_mat2(mat)
            }
        }

        impl Add for $n {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self::new(
                    self.m00 + rhs.m00,
                    self.m01 + rhs.m01,
                    self.m11 + rhs.m11,
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
                    self.m11 - rhs.m11,
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
                    -self.m00, -self.m01, -self.m11,
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
                $nonsymmetricn::from_cols(self.mul(rhs.col(0)), self.mul(rhs.col(1)))
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
                Self::from_cols_array(&[
                    self.x_axis.x * rhs.m00 + self.y_axis.x * rhs.m01,
                    self.x_axis.y * rhs.m00 + self.y_axis.y * rhs.m01,
                    self.x_axis.x * rhs.m01 + self.y_axis.x * rhs.m11,
                    self.x_axis.y * rhs.m01 + self.y_axis.y * rhs.m11,
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

        impl Mul<$m23t> for $n {
            type Output = $m23t;
            #[inline]
            fn mul(self, rhs: $m23t) -> Self::Output {
                $m23t::from_cols(
                    $vt::new(
                        self.row(0).dot(rhs.x_axis),
                        self.row(1).dot(rhs.x_axis),
                    ),
                    $vt::new(
                        self.row(0).dot(rhs.y_axis),
                        self.row(1).dot(rhs.y_axis),
                    ),
                    $vt::new(
                        self.row(0).dot(rhs.z_axis),
                        self.row(1).dot(rhs.z_axis),
                    ),
                )
            }
        }

        impl Mul<&$m23t> for $n {
            type Output = $m23t;
            #[inline]
            fn mul(self, rhs: &$m23t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$m23t> for &$n {
            type Output = $m23t;
            #[inline]
            fn mul(self, rhs: $m23t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$m23t> for &$n {
            type Output = $m23t;
            #[inline]
            fn mul(self, rhs: &$m23t) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$vt> for $n {
            type Output = $vt;
            #[inline]
            fn mul(self, rhs: $vt) -> Self::Output {
                self.mul_vec2(rhs)
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
                mat.to_mat2()
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
                    && self.m11.abs_diff_eq(&other.m11, epsilon)
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
                    && self.m11.relative_eq(&other.m11, epsilon, max_relative)
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
                    && self.m11.ulps_eq(&other.m11, epsilon, max_ulps)
            }
        }

        impl core::fmt::Debug for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($n))
                    .field("m00", &self.m00)
                    .field("m01", &self.m01)
                    .field("m11", &self.m11)
                    .finish()
            }
        }

        impl core::fmt::Display for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if let Some(p) = f.precision() {
                    write!(
                        f,
                        "[[{:.*}, {:.*}], [{:.*}, {:.*}]]",
                        p, self.m00, p, self.m01, p, self.m01, p, self.m11
                    )
                } else {
                    write!(
                        f,
                        "[[{}, {}], [{}, {}]]",
                        self.m00, self.m01, self.m01, self.m11
                    )
                }
            }
        }
        )+
    }
}

#[cfg(feature = "f32")]
symmetric_mat2s!(SymmetricMat2 => Mat2, Mat23, Mat32, Vec2, f32);

#[cfg(feature = "f64")]
symmetric_mat2s!(SymmetricDMat2 => DMat2, DMat23, DMat32, DVec2, f64);

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricMat2 {
    /// Returns the double precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_dmat2(&self) -> SymmetricDMat2 {
        SymmetricDMat2 {
            m00: self.m00 as f64,
            m01: self.m01 as f64,
            m11: self.m11 as f64,
        }
    }
}

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricDMat2 {
    /// Returns the single precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_mat2(&self) -> SymmetricMat2 {
        SymmetricMat2 {
            m00: self.m00 as f32,
            m01: self.m01 as f32,
            m11: self.m11 as f32,
        }
    }
}
