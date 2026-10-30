//! [Symmetric matrix] types and utilities.
//!
//! [Symmetric matrix]: https://en.wikipedia.org/wiki/Symmetric_matrix

mod symmetric_mat2;
mod symmetric_mat3;
mod symmetric_mat4;
mod symmetric_mat5;
mod symmetric_mat6;

pub use symmetric_mat2::Mat2Ext;
#[cfg(feature = "f64")]
pub use symmetric_mat2::SymmetricDMat2;
#[cfg(feature = "f32")]
pub use symmetric_mat2::SymmetricMat2;
pub use symmetric_mat3::Mat3Ext;
#[cfg(feature = "f64")]
pub use symmetric_mat3::SymmetricDMat3;
#[cfg(feature = "f32")]
pub use symmetric_mat3::SymmetricMat3;
pub use symmetric_mat4::Mat4Ext;
#[cfg(feature = "f64")]
pub use symmetric_mat4::SymmetricDMat4;
#[cfg(feature = "f32")]
pub use symmetric_mat4::SymmetricMat4;
#[cfg(feature = "f64")]
pub use symmetric_mat5::SymmetricDMat5;
#[cfg(feature = "f32")]
pub use symmetric_mat5::SymmetricMat5;
#[cfg(feature = "f64")]
pub use symmetric_mat6::SymmetricDMat6;
#[cfg(feature = "f32")]
pub use symmetric_mat6::SymmetricMat6;
