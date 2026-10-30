//! Matrix types and utilities for [`glam`].

#![warn(missing_docs)]
#![no_std]

mod ops;

#[cfg(feature = "f32")]
mod eigen;
mod mat_ext;
mod rectangular;
mod symmetric;

pub use eigen::*;
pub use mat_ext::SquareMatExt;
pub use rectangular::*;
pub use symmetric::*;

/// An error that can occur when converting matrices to other representations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatConversionError {
    /// Tried to convert a matrix to a symmetric matrix type, but the matrix is not symmetric.
    Asymmetric,
}

impl core::fmt::Display for MatConversionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            MatConversionError::Asymmetric => write!(f, "Matrix is not symmetric"),
        }
    }
}
