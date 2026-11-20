//! [Eigen decompositions] for [symmetric matrices](crate::symmetric_mat).
//!
//! [Eigen decompositions]: https://en.wikipedia.org/wiki/Eigendecomposition_of_a_matrix

mod symmetric_eigen2;
mod symmetric_eigen3;

pub use symmetric_eigen2::SymmetricEigen2;
pub use symmetric_eigen3::SymmetricEigen3;
