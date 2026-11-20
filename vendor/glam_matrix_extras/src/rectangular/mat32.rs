use core::iter::Sum;
use core::ops::*;
#[cfg(feature = "f64")]
use glam::{DMat2, DMat3, DVec2, DVec3};
#[cfg(feature = "f32")]
use glam::{Mat2, Mat3, Vec2, Vec3};

#[cfg(feature = "bevy_reflect")]
use bevy_reflect::{Reflect, ReflectDeserialize, ReflectSerialize, std_traits::ReflectDefault};

#[cfg(feature = "f64")]
use crate::rectangular::DMat23;
#[cfg(feature = "f32")]
use crate::rectangular::Mat23;
#[cfg(feature = "f64")]
use crate::symmetric::SymmetricDMat2;
#[cfg(feature = "f32")]
use crate::symmetric::SymmetricMat2;

macro_rules! mat32s {
    ($($n:ident => $m23t:ident, $symmetricm2t:ident, $m2t:ident, $m3t:ident, $v2t:ident, $v3t:ident, $t:ident),+) => {
        $(
        /// A 3x2 column-major matrix.
        #[derive(Clone, Copy, PartialEq)]
        #[cfg_attr(feature = "bevy_reflect", derive(Reflect))]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        #[cfg_attr(
            all(feature = "bevy_reflect", feature = "serde"),
            reflect(Debug, Default, PartialEq, Serialize, Deserialize)
        )]
        pub struct $n {
            /// The first column of the matrix.
            pub x_axis: $v3t,
            /// The second column of the matrix.
            pub y_axis: $v3t,
        }

        impl $n {
            /// A 3x2 matrix with all elements set to `0.0`.
            pub const ZERO: Self = Self::from_cols($v3t::ZERO, $v3t::ZERO);

            /// All NaNs.
            pub const NAN: Self = Self::from_cols($v3t::NAN, $v3t::NAN);

            /// Creates a 3x2 matrix from two column vectors.
            #[inline(always)]
            #[must_use]
            pub const fn from_cols(x_axis: $v3t, y_axis: $v3t) -> Self {
                Self { x_axis, y_axis }
            }

            /// Creates a 3x2 matrix from an array stored in column major order.
            #[inline]
            #[must_use]
            pub const fn from_cols_array(m: &[$t; 6]) -> Self {
                Self::from_cols(
                    $v3t::new(m[0], m[1], m[2]),
                    $v3t::new(m[3], m[4], m[5]),
                )
            }

            /// Creates an array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array(&self) -> [$t; 6] {
                [
                    self.x_axis.x,
                    self.x_axis.y,
                    self.x_axis.z,
                    self.y_axis.x,
                    self.y_axis.y,
                    self.y_axis.z,
                ]
            }

            /// Creates a 3x2 matrix from a 2D array stored in column major order.
            #[inline]
            #[must_use]
            pub const fn from_cols_array_2d(m: &[[$t; 3]; 2]) -> Self {
                Self::from_cols($v3t::from_array(m[0]), $v3t::from_array(m[1]))
            }

            /// Creates a 2D array storing data in column major order.
            #[inline]
            #[must_use]
            pub const fn to_cols_array_2d(&self) -> [[$t; 3]; 2] {
                [self.x_axis.to_array(), self.y_axis.to_array()]
            }

            /// Creates a 3x2 matrix from the first 6 values in `slice`.
            ///
            /// # Panics
            ///
            /// Panics if `slice` is less than 6 elements long.
            #[inline]
            #[must_use]
            pub const fn from_cols_slice(slice: &[$t]) -> Self {
                Self::from_cols(
                    $v3t::new(slice[0], slice[1], slice[2]),
                    $v3t::new(slice[3], slice[4], slice[5]),
                )
            }

            /// Creates a 3x2 matrix from three row vectors.
            pub const fn from_rows(row0: $v2t, row1: $v2t, row2: $v2t) -> Self {
                Self::from_cols(
                    $v3t::new(row0.x, row1.x, row2.x),
                    $v3t::new(row0.y, row1.y, row2.y),
                )
            }

            /// Creates a 2x3 matrix from an array stored in row major order.
            #[inline]
            #[must_use]
            pub const fn from_rows_array(m: &[$t; 6]) -> Self {
                Self::from_rows(
                    $v2t::new(m[0], m[1]),
                    $v2t::new(m[2], m[3]),
                    $v2t::new(m[4], m[5]),
                )
            }

            /// Creates an array storing data in row major order.
            #[inline]
            #[must_use]
            pub const fn to_rows_array(&self) -> [$t; 6] {
                [
                    self.x_axis.x,
                    self.y_axis.x,
                    self.x_axis.y,
                    self.y_axis.y,
                    self.x_axis.z,
                    self.y_axis.z,
                ]
            }

            /// Creates a 2x3 matrix from a 2D array stored in row major order.
            #[inline]
            #[must_use]
            pub const fn from_rows_array_2d(m: &[[$t; 2]; 3]) -> Self {
                Self::from_rows(
                    $v2t::new(m[0][0], m[0][1]),
                    $v2t::new(m[1][0], m[1][1]),
                    $v2t::new(m[2][0], m[2][1]),
                )
            }

            /// Creates a 2D array storing data in row major order.
            #[inline]
            #[must_use]
            pub const fn to_rows_array_2d(&self) -> [[$t; 2]; 3] {
                [
                    [self.x_axis.x, self.y_axis.x],
                    [self.x_axis.y, self.y_axis.y],
                    [self.x_axis.z, self.y_axis.z],
                ]
            }

            /// Creates a 2x3 matrix from the first 6 values in `slice`.
            ///
            /// # Panics
            ///
            /// Panics if `slice` is less than 6 elements long.
            #[inline]
            #[must_use]
            pub const fn from_rows_slice(slice: &[$t]) -> Self {
                Self::from_rows(
                    $v2t::new(slice[0], slice[1]),
                    $v2t::new(slice[2], slice[3]),
                    $v2t::new(slice[4], slice[5]),
                )
            }

            /// Creates a new 3x2 matrix from the outer product `a * b^T`.
            #[inline(always)]
            #[must_use]
            pub fn from_outer_product(a: $v3t, b: $v2t) -> Self {
                Self::from_cols(
                    $v3t::new(a.x * b.x, a.y * b.x, a.z * b.x),
                    $v3t::new(a.x * b.y, a.y * b.y, a.z * b.y),
                )
            }

            /// Returns the matrix column for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 1.
            #[inline]
            #[must_use]
            pub const fn col(&self, index: usize) -> $v3t {
                match index {
                    0 => self.x_axis,
                    1 => self.y_axis,
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns the matrix row for the given `index`.
            ///
            /// # Panics
            ///
            /// Panics if `index` is greater than 2.
            #[inline]
            #[must_use]
            pub const fn row(&self, index: usize) -> $v2t {
                match index {
                    0 => $v2t::new(self.x_axis.x, self.y_axis.x),
                    1 => $v2t::new(self.x_axis.y, self.y_axis.y),
                    2 => $v2t::new(self.x_axis.z, self.y_axis.z),
                    _ => panic!("index out of bounds"),
                }
            }

            /// Returns `true` if, and only if, all elements are finite.
            /// If any element is either `NaN` or positive or negative infinity, this will return `false`.
            #[inline]
            #[must_use]
            pub fn is_finite(&self) -> bool {
                self.x_axis.is_finite() && self.y_axis.is_finite()
            }

            /// Returns `true` if any elements are `NaN`.
            #[inline]
            #[must_use]
            pub fn is_nan(&self) -> bool {
                self.x_axis.is_nan() || self.y_axis.is_nan()
            }

            /// Returns the transpose of `self`.
            #[inline]
            #[must_use]
            pub fn transpose(&self) -> $m23t {
                $m23t::from_rows(self.x_axis, self.y_axis)
            }

            /// Takes the absolute value of each element in `self`.
            #[inline]
            #[must_use]
            pub fn abs(&self) -> Self {
                Self::from_cols(self.x_axis.abs(), self.y_axis.abs())
            }

            /// Transforms a 2D vector into a 3D vector.
            #[inline]
            #[must_use]
            pub fn mul_vec2(&self, rhs: $v2t) -> $v3t {
                $v3t::new(
                    rhs.dot(self.row(0)),
                    rhs.dot(self.row(1)),
                    rhs.dot(self.row(2)),
                )
            }

            /// Multiplies `self` by a 2x2 matrix, `self * rhs`.
            #[inline]
            #[must_use]
            pub fn mul_mat2(&self, rhs: &$m2t) -> Self {
                self.mul(rhs)
            }

            /// Multiplies `self` by a symmetric 2x2 matrix, `self * rhs`.
            #[inline]
            #[must_use]
            pub fn mul_symmetric_mat2(&self, rhs: &$symmetricm2t) -> Self {
                self.mul(rhs)
            }

            /// Multiplies `self` by a 2x3 matrix, `self * rhs`.
            #[inline]
            #[must_use]
            pub fn mul_mat23(&self, rhs: &$m23t) -> $m3t {
                self.mul(rhs)
            }

            /// Multiplies `self` by another matrix that is treated as transposed, `self * rhs.transpose()`.
            #[inline]
            #[must_use]
            pub fn mul_transposed_mat32(&self, rhs: &Self) -> $m3t {
                $m3t::from_cols(
                    $v3t::new(
                        self.row(0).dot(rhs.row(0)),
                        self.row(1).dot(rhs.row(0)),
                        self.row(2).dot(rhs.row(0)),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.row(1)),
                        self.row(1).dot(rhs.row(1)),
                        self.row(2).dot(rhs.row(1)),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.row(2)),
                        self.row(1).dot(rhs.row(2)),
                        self.row(2).dot(rhs.row(2)),
                    ),
                )
            }

            /// Adds two 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn add_mat32(&self, rhs: &Self) -> Self {
                self.add(rhs)
            }

            /// Subtracts two 2x2 matrices.
            #[inline]
            #[must_use]
            pub fn sub_mat32(&self, rhs: &Self) -> Self {
                self.sub(rhs)
            }

            /// Multiplies a 3x2 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn mul_scalar(&self, rhs: $t) -> Self {
                Self::from_cols(self.x_axis * rhs, self.y_axis * rhs)
            }

            /// Divides a 3x2 matrix by a scalar.
            #[inline]
            #[must_use]
            pub fn div_scalar(&self, rhs: $t) -> Self {
                Self::from_cols(self.x_axis / rhs, self.y_axis / rhs)
            }
        }

        impl Default for $n {
            #[inline(always)]
            fn default() -> Self {
                Self::ZERO
            }
        }

        impl Add for $n {
            type Output = Self;
            #[inline]
            fn add(self, rhs: Self) -> Self::Output {
                Self::from_cols(self.x_axis + rhs.x_axis, self.y_axis + rhs.y_axis)
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
                Self::from_cols(self.x_axis - rhs.x_axis, self.y_axis - rhs.y_axis)
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
                Self::from_cols(-self.x_axis, -self.y_axis)
            }
        }

        impl Neg for &$n {
            type Output = $n;
            #[inline]
            fn neg(self) -> Self::Output {
                (*self).neg()
            }
        }

        impl Mul<$m2t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: $m2t) -> Self::Output {
                Self::from_cols(
                    $v3t::new(
                        self.row(0).dot(rhs.x_axis),
                        self.row(1).dot(rhs.x_axis),
                        self.row(2).dot(rhs.x_axis),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.y_axis),
                        self.row(1).dot(rhs.y_axis),
                        self.row(2).dot(rhs.y_axis),
                    )
                )
            }
        }

        impl Mul<&$m2t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: &$m2t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$m2t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: $m2t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$m2t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: &$m2t) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$symmetricm2t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: $symmetricm2t) -> Self::Output {
                Self::from_cols(
                    $v3t::new(
                        self.row(0).dot(rhs.col(0)),
                        self.row(1).dot(rhs.col(0)),
                        self.row(2).dot(rhs.col(0)),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.col(1)),
                        self.row(1).dot(rhs.col(1)),
                        self.row(2).dot(rhs.col(1)),
                    ),
                )
            }
        }

        impl Mul<&$symmetricm2t> for $n {
            type Output = Self;
            #[inline]
            fn mul(self, rhs: &$symmetricm2t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$symmetricm2t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: $symmetricm2t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$symmetricm2t> for &$n {
            type Output = $n;
            #[inline]
            fn mul(self, rhs: &$symmetricm2t) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$m23t> for $n {
            type Output = $m3t;
            #[inline]
            fn mul(self, rhs: $m23t) -> Self::Output {
                $m3t::from_cols(
                    $v3t::new(
                        self.row(0).dot(rhs.x_axis),
                        self.row(1).dot(rhs.x_axis),
                        self.row(2).dot(rhs.x_axis),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.y_axis),
                        self.row(1).dot(rhs.y_axis),
                        self.row(2).dot(rhs.y_axis),
                    ),
                    $v3t::new(
                        self.row(0).dot(rhs.z_axis),
                        self.row(1).dot(rhs.z_axis),
                        self.row(2).dot(rhs.z_axis),
                    ),
                )
            }
        }

        impl Mul<&$m23t> for $n {
            type Output = $m3t;
            #[inline]
            fn mul(self, rhs: &$m23t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$m23t> for &$n {
            type Output = $m3t;
            #[inline]
            fn mul(self, rhs: $m23t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$m23t> for &$n {
            type Output = $m3t;
            #[inline]
            fn mul(self, rhs: &$m23t) -> Self::Output {
                (*self).mul(*rhs)
            }
        }

        impl Mul<$v2t> for $n {
            type Output = $v3t;
            #[inline]
            fn mul(self, rhs: $v2t) -> Self::Output {
                self.mul_vec2(rhs)
            }
        }

        impl Mul<&$v2t> for $n {
            type Output = $v3t;
            #[inline]
            fn mul(self, rhs: &$v2t) -> Self::Output {
                self.mul(*rhs)
            }
        }

        impl Mul<$v2t> for &$n {
            type Output = $v3t;
            #[inline]
            fn mul(self, rhs: $v2t) -> Self::Output {
                (*self).mul(rhs)
            }
        }

        impl Mul<&$v2t> for &$n {
            type Output = $v3t;
            #[inline]
            fn mul(self, rhs: &$v2t) -> Self::Output {
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
                self.x_axis.abs_diff_eq(other.x_axis, epsilon)
                    && self.y_axis.abs_diff_eq(other.y_axis, epsilon)
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
                self.x_axis.relative_eq(&other.x_axis, epsilon, max_relative)
                    && self.y_axis.relative_eq(&other.y_axis, epsilon, max_relative)
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
                self.x_axis.ulps_eq(&other.x_axis, epsilon, max_ulps)
                    && self.y_axis.ulps_eq(&other.y_axis, epsilon, max_ulps)
            }
        }

        impl core::fmt::Debug for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.debug_struct(stringify!($n))
                    .field("x_axis", &self.x_axis)
                    .field("y_axis", &self.y_axis)
                    .finish()
            }
        }

        impl core::fmt::Display for $n {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if let Some(p) = f.precision() {
                    write!(
                        f,
                        "[[{:.*}, {:.*}, {:.*}], [{:.*}, {:.*}, {:.*}]]",
                        p, self.x_axis.x, p, self.x_axis.y, p, self.x_axis.z,
                        p, self.y_axis.x, p, self.y_axis.y, p, self.y_axis.z
                    )
                } else {
                    write!(
                        f,
                        "[[{}, {}, {}], [{}, {}, {}]]",
                        self.x_axis.x, self.x_axis.y, self.x_axis.z,
                        self.y_axis.x, self.y_axis.y, self.y_axis.z
                    )
                }
            }
        }
        )+
    }
}

#[cfg(feature = "f32")]
mat32s!(Mat32 => Mat23, SymmetricMat2, Mat2, Mat3, Vec2, Vec3, f32);

#[cfg(feature = "f64")]
mat32s!(DMat32 => DMat23, SymmetricDMat2, DMat2, DMat3, DVec2, DVec3, f64);

#[cfg(all(feature = "f32", feature = "f64"))]
impl Mat32 {
    /// Returns the double precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_dmat32(&self) -> DMat32 {
        DMat32 {
            x_axis: self.x_axis.as_dvec3(),
            y_axis: self.y_axis.as_dvec3(),
        }
    }
}

#[cfg(all(feature = "f32", feature = "f64"))]
impl DMat32 {
    /// Returns the single precision version of `self`.
    #[inline]
    #[must_use]
    pub fn as_mat32(&self) -> Mat32 {
        Mat32 {
            x_axis: self.x_axis.as_vec3(),
            y_axis: self.y_axis.as_vec3(),
        }
    }
}

#[cfg(test)]
mod tests {
    use glam::{Mat2, Mat3, vec2, vec3};

    use crate::{Mat23, Mat32};

    #[test]
    fn mat32_mul_vec2() {
        let mat = Mat32::from_cols(vec3(4.0, 1.0, 6.0), vec3(7.0, 9.0, 2.0));
        let vec = vec2(1.0, 2.0);

        let expected = vec3(18.0, 19.0, 10.0);
        let result = mat.mul_vec2(vec);

        assert_eq!(result, expected);
    }

    #[test]
    fn mat32_mul_mat2() {
        let mat32 = Mat32::from_cols(vec3(4.0, 1.0, 6.0), vec3(7.0, 9.0, 2.0));
        let mat2 = Mat2::from_cols(vec2(2.0, 5.0), vec2(1.0, 8.0));

        let expected = Mat32::from_cols(vec3(43.0, 47.0, 22.0), vec3(60.0, 73.0, 22.0));
        let result = mat32.mul_mat2(&mat2);

        assert_eq!(result, expected);
    }

    #[test]
    fn mat32_mul_mat23() {
        let mat32 = Mat32::from_cols(vec3(4.0, 1.0, 6.0), vec3(7.0, 9.0, 2.0));
        let mat23 = Mat23::from_rows(vec3(2.0, 5.0, 1.0), vec3(8.0, 3.0, 4.0));

        let expected = Mat3::from_cols(
            vec3(64.0, 74.0, 28.0),
            vec3(41.0, 32.0, 36.0),
            vec3(32.0, 37.0, 14.0),
        );
        let result = mat32.mul_mat23(&mat23);

        assert_eq!(result, expected);
    }

    #[test]
    fn mat32_mul_transposed_mat32() {
        let mat32_a = Mat32::from_cols(vec3(4.0, 1.0, 6.0), vec3(7.0, 9.0, 2.0));
        let mat32_b = Mat32::from_cols(vec3(2.0, 5.0, 1.0), vec3(8.0, 3.0, 4.0));

        let expected = Mat3::from_cols(
            vec3(64.0, 74.0, 28.0),
            vec3(41.0, 32.0, 36.0),
            vec3(32.0, 37.0, 14.0),
        );
        let result = mat32_a.mul_transposed_mat32(&mat32_b);

        assert_eq!(result, expected);
        assert_eq!(result, mat32_a * mat32_b.transpose());
    }
}
