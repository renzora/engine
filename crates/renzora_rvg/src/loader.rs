//! Bevy `AssetLoader` for `.rvg` files.

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::reflect::TypePath;
use thiserror::Error;

use crate::RvgFile;

/// Loads an `.rvg` as an [`RvgFile`] asset. Registered by the virtual geometry
/// runtime with `app.init_asset::<RvgFile>().init_asset_loader::<RvgLoader>()`.
#[derive(Default, TypePath)]
pub struct RvgLoader;

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

impl AssetLoader for RvgLoader {
    type Asset = RvgFile;
    type Settings = ();
    type Error = RvgLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<RvgFile, RvgLoadError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        RvgFile::from_bytes(&bytes)
    }

    fn extensions(&self) -> &[&str] {
        &["rvg"]
    }
}
