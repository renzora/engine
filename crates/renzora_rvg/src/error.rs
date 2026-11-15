//! Why an `.rvg` did not read.
//!
//! Its own module, not the loader's, because [`crate::RvgFile::from_bytes`]
//! returns it, and that is built without Bevy (the standalone companion reads
//! and bakes `.rvg` files) while the loader is not.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum RvgLoadError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic; expected RVG1")]
    BadMagic,
    #[error("unsupported version {0}; this build reads {1}. Rebuild the virtual geometry")]
    UnsupportedVersion(u32, u32),
    #[error("file is truncated")]
    Truncated,
    #[error("file is corrupt: {0}")]
    Corrupt(&'static str),
}
