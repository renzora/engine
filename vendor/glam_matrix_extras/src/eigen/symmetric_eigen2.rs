use crate::{
    SymmetricMat2,
    ops::{self, FloatPow},
};
use glam::{Mat2, Vec2, Vec2Swizzles};

/// The [eigen decomposition] of a [`SymmetricMat2`].
///
/// [eigen decomposition]: https://en.wikipedia.org/wiki/Eigendecomposition_of_a_matrix
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SymmetricEigen2 {
    /// The eigenvalues of the [`SymmetricMat2`].
    ///
    /// These should be in ascending order `eigen1 <= eigen2`.
    pub eigenvalues: Vec2,
    /// The eigenvectors of the [`SymmetricMat2`].
    /// They should be unit length and orthogonal to each other.
    ///
    /// The eigenvectors are ordered to correspond to the eigenvalues. For example,
    /// `eigenvectors.x_axis` corresponds to `eigenvalues.x`.
    pub eigenvectors: Mat2,
}

impl SymmetricEigen2 {
    /// Computes the eigen decomposition of the given [`SymmetricMat2`].
    ///
    /// The eigenvalues are returned in ascending order `eigen1 <= eigen2`.
    /// This can be reversed with the [`reverse`](Self::reverse) method.
    // TODO: Verify that the eigenvalues really are always returned in ascending order.
    pub fn new(mat: SymmetricMat2) -> Self {
        let eigenvalues = Self::eigenvalues(mat);
        let eigenvector1 = Self::eigenvector(mat, eigenvalues.x);
        let eigenvector2 = Self::eigenvector(mat, eigenvalues.y);

        Self {
            eigenvalues,
            eigenvectors: Mat2::from_cols(eigenvector1, eigenvector2),
        }
    }

    /// Reverses the order of the eigenvalues and their corresponding eigenvectors.
    pub fn reverse(&self) -> Self {
        Self {
            eigenvalues: self.eigenvalues.yx(),
            eigenvectors: Mat2::from_cols(self.eigenvectors.y_axis, self.eigenvectors.x_axis),
        }
    }

    /// Computes the eigenvalues of a [`SymmetricMat2`].
    ///
    /// Reference: <https://croninprojects.org/Vince/Geodesy/FindingEigenvectors.pdf>
    pub fn eigenvalues(mat: SymmetricMat2) -> Vec2 {
        let [a, b, c] = [
            1.0,
            -(mat.m00 + mat.m11),
            mat.m00 * mat.m11 - mat.m01 * mat.m01,
        ];
        // The eigenvalues are the roots of the quadratic equation:
        // ax^2 + bx + c = 0
        // x = (-b ± sqrt(b^2 - 4ac)) / 2a
        let sqrt_part = ops::sqrt(b.squared() - 4.0 * a * c);
        let eigen1 = (-b + sqrt_part) / (2.0 * a);
        let eigen2 = (-b - sqrt_part) / (2.0 * a);
        Vec2::new(eigen1, eigen2)
    }

    /// Computes the unit-length eigenvector corresponding to the given `eigenvalue`
    /// of the symmetric 2x2 `mat`.
    ///
    /// Reference: <https://croninprojects.org/Vince/Geodesy/FindingEigenvectors.pdf>
    pub fn eigenvector(mat: SymmetricMat2, eigenvalue: f32) -> Vec2 {
        Vec2::new(1.0, (eigenvalue - mat.m00) / mat.m01).normalize()
    }
}

#[cfg(test)]
mod test {
    use approx::assert_relative_eq;
    use glam::{Mat2, Vec2};

    use crate::SymmetricMat2;

    use super::SymmetricEigen2;

    #[test]
    fn eigen_2x2() {
        let mat = SymmetricMat2::new(6.0, 3.0, 4.0);
        let eigen = SymmetricEigen2::new(mat);

        assert_relative_eq!(
            eigen.eigenvalues,
            Vec2::new(8.16228, 1.83772),
            epsilon = 0.001
        );
        assert_relative_eq!(
            Mat2::from_cols(eigen.eigenvectors.x_axis, eigen.eigenvectors.y_axis,),
            Mat2::from_cols(Vec2::new(0.811242, 0.58471), Vec2::new(0.58471, -0.811242),),
            epsilon = 0.001
        );
    }
}
