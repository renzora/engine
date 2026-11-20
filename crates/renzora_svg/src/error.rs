//! Why an SVG failed to load or draw.
//!
//! Its own module, not the loader's, because the rasterizer returns it too,
//! and the rasterizer is built without Bevy (the standalone companion draws
//! SVG thumbnails with it) while the loader is not.

use thiserror::Error;

/// Why an SVG failed to load.
#[derive(Debug, Error)]
pub enum SvgLoadError {
    #[error("could not read the file: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a valid SVG: {0}")]
    Parse(String),
    #[error("invalid settings: {0}")]
    Settings(String),
    #[error("the SVG renderer panicked: {0}")]
    Panicked(String),
}
