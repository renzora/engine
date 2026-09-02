//! Maths types with no boundary presence.
//!
//! `Vec2` and `Color` are plugin-side only: they are not in `sys`, cannot be a
//! component field, and never cross. That is deliberate — adding a type to the
//! frozen contract is permanent, and neither has earned it. They exist because a
//! plugin doing any 2D or colour work otherwise writes them itself, which is what
//! `plugins/hair` did for `Vec3` before this surface was filled in.

use super::component::Vec3;

// The colour conversions call `f32` methods that only exist in `std`. Under
// `no_std` the shim supplies them; under `std` the inherent methods are used.
#[cfg(not(feature = "std"))]
use crate::float::FloatExt as _;

/// Mirrors `glam::Vec2`. Plugin-side only — see the note above.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };
    pub const ONE: Vec2 = Vec2 { x: 1.0, y: 1.0 };
    pub const X: Vec2 = Vec2 { x: 1.0, y: 0.0 };
    pub const Y: Vec2 = Vec2 { x: 0.0, y: 1.0 };

    pub const fn new(x: f32, y: f32) -> Vec2 {
        Vec2 { x, y }
    }
    pub const fn splat(v: f32) -> Vec2 {
        Vec2 { x: v, y: v }
    }
    pub fn dot(self, o: Vec2) -> f32 {
        self.x * o.x + self.y * o.y
    }
    /// The 2D cross product: the z of the 3D one, i.e. a signed area.
    pub fn perp_dot(self, o: Vec2) -> f32 {
        self.x * o.y - self.y * o.x
    }
    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }
    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }
    pub fn distance(self, o: Vec2) -> f32 {
        (self - o).length()
    }
    /// `ZERO` rather than `NaN` for a zero-length vector — same reasoning as
    /// [`Vec3::normalize_or_zero`].
    pub fn normalize_or_zero(self) -> Vec2 {
        let len = self.length();
        if len > 1e-6 {
            self / len
        } else {
            Vec2::ZERO
        }
    }
    pub fn lerp(self, o: Vec2, t: f32) -> Vec2 {
        self + (o - self) * t
    }
    pub fn extend(self, z: f32) -> Vec3 {
        Vec3::new(self.x, self.y, z)
    }
}

impl core::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}
impl core::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}
impl core::ops::Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        Vec2::new(self.x * s, self.y * s)
    }
}
impl core::ops::Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f32) -> Vec2 {
        Vec2::new(self.x / s, self.y / s)
    }
}
impl core::ops::Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        Vec2::new(-self.x, -self.y)
    }
}
impl core::ops::AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        *self = *self + o;
    }
}
impl core::ops::SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        *self = *self - o;
    }
}

/// Linear RGBA. Mirrors the shape `add_material` and the material shaders take.
///
/// Linear, not sRGB — [`Color::srgb`] converts, and the distinction matters:
/// passing an sRGB triple where linear is expected washes everything out, which
/// is the single most common colour mistake and looks like a lighting bug.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

impl Color {
    pub const WHITE: Color = Color::linear_rgb(1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::linear_rgb(0.0, 0.0, 0.0);
    pub const NONE: Color = Color { red: 0.0, green: 0.0, blue: 0.0, alpha: 0.0 };

    pub const fn linear_rgb(red: f32, green: f32, blue: f32) -> Color {
        Color { red, green, blue, alpha: 1.0 }
    }
    pub const fn linear_rgba(red: f32, green: f32, blue: f32, alpha: f32) -> Color {
        Color { red, green, blue, alpha }
    }

    /// From sRGB components, which is what a colour picker and every hex code
    /// give you.
    pub fn srgb(red: f32, green: f32, blue: f32) -> Color {
        Color {
            red: srgb_to_linear(red),
            green: srgb_to_linear(green),
            blue: srgb_to_linear(blue),
            alpha: 1.0,
        }
    }

    /// Hue in degrees, saturation and lightness in `0..1`. Returns linear.
    pub fn hsl(hue: f32, saturation: f32, lightness: f32) -> Color {
        let h = hue.rem_euclid(360.0) / 60.0;
        let c = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
        let x = c * (1.0 - (h % 2.0 - 1.0).abs());
        let (r, g, b) = match h as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        let m = lightness - c / 2.0;
        Color::srgb(r + m, g + m, b + m)
    }

    /// The `[f32; 4]` that `add_material` and a material's uniform expect.
    pub const fn to_linear_array(self) -> [f32; 4] {
        [self.red, self.green, self.blue, self.alpha]
    }

    pub const fn with_alpha(mut self, alpha: f32) -> Color {
        self.alpha = alpha;
        self
    }
}

impl Default for Color {
    fn default() -> Self {
        Color::WHITE
    }
}

/// The sRGB transfer function, which is a piecewise curve rather than a plain
/// `powf(2.2)` — the linear segment near black is what keeps dark colours from
/// crushing.
fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}
