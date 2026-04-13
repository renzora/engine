use thiserror::Error;

/// Error enum for [`ClothPlugin`]
///
/// [`ClothPlugin`]: crate::ClothPlugin
#[derive(Debug, Clone, Error)]
pub enum Error {
    /// The mesh associated to a cloth is missing a vertex attribute
    #[error("Mesh associated to cloth doesn't have `{0}` attribute set")]
    MissingMeshAttribute(String),
    /// The mesh associated to a cloth is missing a vertex attribute
    #[error("Mesh associated to cloth has invalid `{attribute}` attribute set: {message}")]
    InvalidMeshAttribute {
        /// Invalid attribute id
        attribute: String,
        /// Error message
        message: String,
    },
    /// The mesh associated to a cloth has an invalid vertex position attribute
    #[error("Unsupported vertex position attribute, only `Float32x3` is supported")]
    UnsupportedVertexPositionAttribute,
    /// The mesh associated to a cloth has no indices
    #[error("Cloth requires meshes with indexed geometry")]
    MissingIndices,
}
