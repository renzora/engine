//! Rectangular matrix types and utilities.

mod mat23;
mod mat32;

#[cfg(feature = "f64")]
pub use mat23::DMat23;
#[cfg(feature = "f32")]
pub use mat23::Mat23;
#[cfg(feature = "f64")]
pub use mat32::DMat32;
#[cfg(feature = "f32")]
pub use mat32::Mat32;
