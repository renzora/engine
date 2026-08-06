//! `f32` math for `#![no_std]` plugins.
//!
//! `sqrt`, `sin`, `powf` and the rest are inherent methods on `f32` in `std` and
//! absent from `core` — not an oversight, but because they lower to libm calls
//! and `core` links no libm. A plugin doing any trigonometry therefore stops
//! compiling the moment it drops `std`, which is most of them: a wobble, a
//! swirl, a radial blur are all `sin` and `cos`.
//!
//! [`FloatExt`] restores them, backed by the `libm` crate. It is in the prelude,
//! so a plugin that already writes `use renzora_plugin::prelude::*` needs no
//! change at all — `x.sin()` keeps working.
//!
//! **This module only exists in a `no_std` build.** Under `std` the inherent
//! methods are already there and the prelude does not export this, so there is
//! never a moment where both are in scope competing to resolve a call.

/// The `std`-only `f32` methods, reimplemented over `libm`.
///
/// Signatures match `std`'s exactly, so this is invisible at the call site. Any
/// difference in behaviour would be a bug — see [`FloatExt::rem_euclid`] for the
/// one place that took more than a direct forward.
pub trait FloatExt {
    /// Square root.
    fn sqrt(self) -> f32;
    /// Sine, radians.
    fn sin(self) -> f32;
    /// Cosine, radians.
    fn cos(self) -> f32;
    /// Tangent, radians.
    fn tan(self) -> f32;
    /// Sine and cosine together — one call, both results.
    fn sin_cos(self) -> (f32, f32);
    /// Arcsine, radians.
    fn asin(self) -> f32;
    /// Arccosine, radians.
    fn acos(self) -> f32;
    /// Arctangent, radians.
    fn atan(self) -> f32;
    /// Four-quadrant arctangent of `self / other`.
    fn atan2(self, other: f32) -> f32;
    /// `e` raised to `self`.
    fn exp(self) -> f32;
    /// Natural logarithm.
    fn ln(self) -> f32;
    /// Base-2 logarithm.
    fn log2(self) -> f32;
    /// Base-10 logarithm.
    fn log10(self) -> f32;
    /// `self` raised to a floating-point power.
    fn powf(self, n: f32) -> f32;
    /// `self` raised to an integer power.
    fn powi(self, n: i32) -> f32;
    /// Largest integer less than or equal to `self`.
    fn floor(self) -> f32;
    /// Smallest integer greater than or equal to `self`.
    fn ceil(self) -> f32;
    /// Nearest integer, halfway cases away from zero.
    fn round(self) -> f32;
    /// Integer part.
    fn trunc(self) -> f32;
    /// Fractional part.
    fn fract(self) -> f32;
    /// Cube root.
    fn cbrt(self) -> f32;
    /// Length of the hypotenuse of a right triangle with legs `self` and
    /// `other`, without the intermediate overflow of `(a*a + b*b).sqrt()`.
    fn hypot(self, other: f32) -> f32;
    /// `self * a + b` with a single rounding.
    fn mul_add(self, a: f32, b: f32) -> f32;
    /// Least non-negative remainder of `self (mod rhs)`.
    fn rem_euclid(self, rhs: f32) -> f32;
}

impl FloatExt for f32 {
    fn sqrt(self) -> f32 {
        libm::sqrtf(self)
    }
    fn sin(self) -> f32 {
        libm::sinf(self)
    }
    fn cos(self) -> f32 {
        libm::cosf(self)
    }
    fn tan(self) -> f32 {
        libm::tanf(self)
    }
    fn sin_cos(self) -> (f32, f32) {
        (libm::sinf(self), libm::cosf(self))
    }
    fn asin(self) -> f32 {
        libm::asinf(self)
    }
    fn acos(self) -> f32 {
        libm::acosf(self)
    }
    fn atan(self) -> f32 {
        libm::atanf(self)
    }
    fn atan2(self, other: f32) -> f32 {
        libm::atan2f(self, other)
    }
    fn exp(self) -> f32 {
        libm::expf(self)
    }
    fn ln(self) -> f32 {
        libm::logf(self)
    }
    fn log2(self) -> f32 {
        libm::log2f(self)
    }
    fn log10(self) -> f32 {
        libm::log10f(self)
    }
    fn powf(self, n: f32) -> f32 {
        libm::powf(self, n)
    }
    fn powi(self, n: i32) -> f32 {
        libm::powf(self, n as f32)
    }
    fn floor(self) -> f32 {
        libm::floorf(self)
    }
    fn ceil(self) -> f32 {
        libm::ceilf(self)
    }
    fn round(self) -> f32 {
        libm::roundf(self)
    }
    fn trunc(self) -> f32 {
        libm::truncf(self)
    }
    /// `std` defines this as `self - self.trunc()`, which keeps the sign of the
    /// input — `(-1.5).fract()` is `-0.5`, not `0.5`. Match that.
    fn fract(self) -> f32 {
        self - libm::truncf(self)
    }
    fn cbrt(self) -> f32 {
        libm::cbrtf(self)
    }
    fn hypot(self, other: f32) -> f32 {
        libm::hypotf(self, other)
    }
    fn mul_add(self, a: f32, b: f32) -> f32 {
        libm::fmaf(self, a, b)
    }
    /// The one method that is not a direct forward. `fmodf` is `std`'s `%`, which
    /// keeps the sign of the *dividend*, so `(-90.0).rem_euclid(360.0)` would come
    /// back `-90.0` instead of `270.0`. Shifting a negative result up by `|rhs|`
    /// restores `std`'s contract — the hue wrap in `Color` depends on it.
    fn rem_euclid(self, rhs: f32) -> f32 {
        let r = libm::fmodf(self, rhs);
        if r < 0.0 {
            r + libm::fabsf(rhs)
        } else {
            r
        }
    }
}
