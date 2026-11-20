use core::iter::Sum;
use core::ops::*;
#[cfg(feature = "f64")]
use glam::{DVec2, DVec3};
use glam::{Vec2, Vec3};

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize, std_traits::ReflectDefault};

#[cfg(feature = "f64")]
use crate::{DMat23, SymmetricDMat2, SymmetricDMat3};
#[cfg(feature = "f32")]
use crate::{Mat23, SymmetricMat2, SymmetricMat3};

macro_rules! symmetric_mat5s {
    ($($n:ident => $symmetricm2t:ident, $symmetricm3t:ident, $m23t:ident, $v2t:ident, $v3t:ident, $t:ident),+) => {
        $(
        /// The bottom left triangle (including the diagonal) of a symmetric 5x5 column-major matrix.
        ///
        /// This is useful for storing a symmetric 5x5 matrix in a more compact form and performing some
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
        ///
        /// The 5x5 matrix is represented as:
        ///
        /// ```text
        /// [ A  BT ]
        /// [ B  D  ]
        /// ```
        #[derive(Clone, Copy, PartialEq)]
        #[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(
            all(feature = "bevy_reflect", feature = "serde"),
            reflect(Debug, Default, PartialEq, Serialize, Deserialize)
        )]
        pub struct $n {
            /// The bottom left triangle of the top left 3x3 block of the matrix,
            /// including the diagonal.
            pub a: $symmetricm3t,
            /// The bottom left 2x3 block of the matrix.
            pub b: $m23t,
            /// The bottom left triangle of the bottom right 2x2 block of the matrix,
            /// including the diagonal.
            pub d: $symmetricm2t,
        }

        impl $n {
            /// A symmetric 5x5 matrix with all elements set to `0.0`.
            pub const ZERO: Self = Self::new(
                $symmetricm3t::ZERO,
                $m23t::ZERO,
                $symmetricm2t::ZERO,
            );

            /// A symmetric 5x5 identity matrix, where all diagonal elements are `1.0`,
            /// and all off-diagonal elements are `0.0`.
            pub const IDENTITY: Self = Self::new(
                $symmetricm3t::IDENTITY,
                $m23t::ZERO,
                $symmetricm2t::IDENTITY,
            );

            /// All NaNs.
            pub const NAN: Self = Self::new(
                $symmetricm3t::NAN,
                $m23t::NAN,
                $symmetricm2t::NAN,
            );

            /// Creates a new symmetric 5x5 matrix from its bottom left triangle, including diagonal elements.
            ///
            /// The matrix is represented as:
            ///
            /// ```text
            /// [ A  BT ]
            /// [ B  D  ]
            /// ```
            #[inline(always)]
            #[must_use]
            pub const fn new(
                a: $symmetricm3t,
                b: $m23t,
                d: $symmetricm2t,
            ) -> Self {
                Self { a, b, d }
            }

            /// Creates a new symmetric 5x5 matrix from the outer product `[v1, v2] * [v1, v2]^T`.
            #[inline]
            #[must_use]
            pub fn from_outer_product(v1: $v3t, v2: $v2t) -> Self {
                Self::new(
                    $symmetricm3t::from_outer_product(v1),
                    $m23t::from_outer_product(v2, v1),
                    $symmetricm2t::from_outer_product(v2),
                )
            }

            /// Returns `true` if, and only if, all elements are finite.
            /// If any element is either `NaN` or positive or negative infinity, this will return `false`.
            #[inline]
            #[must_use]
            pub fn is_finite(&self) -> bool {
                self.a.is_finite() && self.b.is_finite() && self.d.is_finite()
            }

            /// Returns `true` if any elements are `NaN`.
            #[inline]
            #[must_use]
            pub fn is_nan(&self) -> bool {
                self.a.is_nan() || self.b.is_nan() || self.d.is_nan()
            }

            /// Returns the inverse of `self`.
            ///
            /// If the matrix is not invertible the returned matrix will be invalid.
            #[inline]
            #[must_use]
            pub fn inverse(&self) -> Self {
                let inv_d = self.d.inverse();
                let bt_inv_d = inv_d.mul(self.b);
                let bt_inv_d_b = $symmetricm3t::complete_mat23_sandwich(&bt_inv_d, &self.b);

                let res_a = self.a.sub(bt_inv_d_b).inverse();
                let neg_res_bt = bt_inv_d.mul(res_a);
                let res_d = $symmetricm2t::complete_mat23_sandwich(&bt_inv_d, &neg_res_bt).add(inv_d);

                Self::new(res_a, -neg_res_bt, res_d)
            }

            /// Returns the inverse of `self`, or a zero matrix if the matrix is not invertible.
            #[inline]
            #[must_use]
            pub fn inverse_or_zero(&self) -> Self {
                // TODO: Optimize this.
                let inverse = self.inverse();
                if inverse.is_finite() {
                    inverse
                } else {
                    Self::ZERO
                }
            }

            /// Takes the absolute value of each element in `self`.
            #[inline]
            #[must_use]
            pub fn abs(&self) -> Self {
                Self::new(self.a.abs(), self.b.abs(), self.d.abs())
            }

            /// Transforms a 5x1 vector that is split into a 3x1 vector and 2x1 vector.
            #[inline]
            #[must_use]
            pub fn mul_vec5(&self, rhs1: $v3t, rhs2: $v2t) -> ($v3t, $v2t) {
                let res1 = $v3t::new(
                    rhs1.x * self.a.m00 + rhs1.y * self.a.m01 + rhs1.z * self.a.m02 + rhs2.dot(self.b.col(0)),
                    rhs1.x * self.a.m01 + rhs1.y * self.a.m11 + rhs1.z * self.a.m12 + rhs2.dot(self.b.col(1)),
                    rhs1.x * self.a.m02 + rhs1.y * self.a.m12 + rhs1.z * self.a.m22 + rhs2.dot(self.b.col(2)),
                );
                let res2 = $v2t::new(
                    rhs1.dot(self.b.row(0)) + rhs2.x * self.d.m00 + rhs2.y * self.d.m01,
                    rhs1.dot(self.b.row(1)) + rhs2.x * self.d.m01 + rhs2.y * self.d.m11,
                );
                (res1, res2)
            }

            /// Adds two symmetric 5x5 matrices.
            #[inline]
            #[must_use]
            pub fn add_symmetric_mat5(&self, rhs: &Self) -> Self {
                self.add(rhs)
            }

            /// Subtracts two symmetric 5x5 matrices.
            #[inline]
            #[must_use]
            pub fn sub_symmetric_mat5(&self, rhs: &Self) -> Self {
                self.sub(rhs)
            }

            /// Multiplies a 5x5 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn mul_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.a.mul_scalar(rhs),
                    self.b.mul_scalar(rhs),
                    self.d.mul_scalar(rhs),
                )
            }

            /// Divides a 5x5 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn div_scalar(&self, rhs: $t) -> Self {
                Self::new(
                    self.a.div_scalar(rhs),
                    self.b.div_scalar(rhs),
                    self.d.div_scalar(rhs),
                )
            }
        }

        impl Default for $n {
            #[inline(always)]
            fn default() -> Self {
                Self::IDENTITY
            }
        }

        impl Add for $n {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self::new(self.a.add(rhs.a), self.b.add(rhs.b), self.d.add(rhs.d))
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

        impl Sub for $n {
            type Output = Self;
            #[inline]
            fn sub(self, rhs: Self) -> Self::Output {
                Self::new(self.a.sub(rhs.a), self.b.sub(rhs.b), self.d.sub(rhs.d))
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

        impl Neg for $n {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self::Output {
                Self::new(-self.a, -self.b, -self.d)
            }
        }

        impl Neg for &$n {
            type Output = $n;
            #[inline]
            fn neg(self) -> Self::Output {
                (*self).neg()
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
                self.a.abs_diff_eq(&other.a, epsilon)
                    && self.b.abs_diff_eq(&other.b, epsilon)
                    && self.d.abs_diff_eq(&other.d, epsilon)
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
                self.a.relative_eq(&other.a, epsilon, max_relative)
                    && self.b.relative_eq(&other.b, epsilon, max_relative)
                    && self.d.relative_eq(&other.d, epsilon, max_relative)
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
                self.a.ulps_eq(&other.a, epsilon, max_ulps)
                    && self.b.ulps_eq(&other.b, epsilon, max_ulps)
                    && self.d.ulps_eq(&other.d, epsilon, max_ulps)
            }
        }

        impl core::fmt::Debug for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($n))
                    .field("a", &self.a)
                    .field("b", &self.b)
                    .field("d", &self.d)
                    .finish()
            }
        }

        impl core::fmt::Display for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if let Some(p) = f.precision() {
                    write!(
                        f,
                        r#"[
  [{:.*}, {:.*}, {:.*}, {:.*}, {:.*}],
  [{:.*}, {:.*}, {:.*}, {:.*}, {:.*}],
  [{:.*}, {:.*}, {:.*}, {:.*}, {:.*}],
  [{:.*}, {:.*}, {:.*}, {:.*}, {:.*}],
  [{:.*}, {:.*}, {:.*}, {:.*}, {:.*}],
]"#,
                        p, self.a.m00, p, self.a.m01, p, self.a.m02, p, self.b.x_axis.x, p, self.b.x_axis.y,
                        p, self.a.m01, p, self.a.m11, p, self.a.m12, p, self.b.y_axis.x, p, self.b.y_axis.y,
                        p, self.a.m02, p, self.a.m12, p, self.a.m22, p, self.b.z_axis.x, p, self.b.z_axis.y,
                        p, self.b.x_axis.x, p, self.b.y_axis.x, p, self.b.z_axis.x, p, self.d.m00, p, self.d.m01,
                        p, self.b.x_axis.y, p, self.b.y_axis.y, p, self.b.z_axis.y, p, self.d.m01, p, self.d.m11,
                    )
                } else {
                    write!(
                        f,
                        r#"[
  [{}, {}, {}, {}, {}],
  [{}, {}, {}, {}, {}],
  [{}, {}, {}, {}, {}],
  [{}, {}, {}, {}, {}],
  [{}, {}, {}, {}, {}],
]"#,
                        self.a.m00, self.a.m01, self.a.m02, self.b.x_axis.x, self.b.x_axis.y,
                        self.a.m01, self.a.m11, self.a.m12, self.b.y_axis.x, self.b.y_axis.y,
                        self.a.m02, self.a.m12, self.a.m22, self.b.z_axis.x, self.b.z_axis.y,
                        self.b.x_axis.x, self.b.y_axis.x, self.b.z_axis.x, self.d.m00, self.d.m01,
                        self.b.x_axis.y, self.b.y_axis.y, self.b.z_axis.y, self.d.m01, self.d.m11,
                    )
                }
            }
        }
        )+
    }
}

#[cfg(feature = "f32")]
symmetric_mat5s!(SymmetricMat5 => SymmetricMat2, SymmetricMat3, Mat23, Vec2, Vec3, f32);

#[cfg(feature = "f64")]
symmetric_mat5s!(SymmetricDMat5 => SymmetricDMat2, SymmetricDMat3, DMat23, DVec2, DVec3, f64);

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricMat5 {
    /// Returns the double precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_dmat5(&self) -> SymmetricDMat5 {
        SymmetricDMat5 {
            a: self.a.as_symmetric_dmat3(),
            b: self.b.as_dmat23(),
            d: self.d.as_symmetric_dmat2(),
        }
    }
}

#[cfg(all(feature = "f32", feature = "f64"))]
impl SymmetricDMat5 {
    /// Returns the single precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_symmetric_mat5(&self) -> SymmetricMat5 {
        SymmetricMat5 {
            a: self.a.as_symmetric_mat3(),
            b: self.b.as_mat23(),
            d: self.d.as_symmetric_mat2(),
        }
    }
}

#[cfg(test)]
mod tests {
    use approx::assert_relative_eq;
    use glam::{Vec2, Vec3, vec2, vec3};

    use crate::{Mat23, SymmetricMat2, SymmetricMat3, SymmetricMat5};

    #[test]
    fn mul_vec5() {
        let mat = SymmetricMat5::from_outer_product(Vec3::new(1.0, 2.0, 3.0), Vec2::new(4.0, 5.0));

        let (res1, res2) = mat.mul_vec5(Vec3::new(1.0, 2.0, 3.0), Vec2::new(4.0, 5.0));

        assert_eq!(res1, vec3(55.0, 110.0, 165.0));
        assert_eq!(res2, vec2(220.0, 275.0));
    }

    #[test]
    fn inverse() {
        let a = SymmetricMat3::new(1.0, 6.0, 7.0, 2.0, 10.0, 3.0);
        let b = Mat23::from_cols(vec2(8.0, 9.0), vec2(11.0, 12.0), vec2(13.0, 14.0));
        let d = SymmetricMat2::new(4.0, 15.0, 5.0);
        let mat = SymmetricMat5 { a, b, d };

        // Known solution x = (x1, x2)
        let x1 = Vec3::new(1.0, 2.0, 3.0);
        let x2 = Vec2::new(4.0, 5.0);

        // Compute rhs = mat * x
        let (rhs1, rhs2) = mat.mul_vec5(x1, x2);

        // Solve
        let (sol1, sol2) = mat.inverse().mul_vec5(rhs1, rhs2);

        // Check solution
        assert_relative_eq!(sol1, x1, epsilon = 1e-5);
        assert_relative_eq!(sol2, x2, epsilon = 1e-5);
    }
}
