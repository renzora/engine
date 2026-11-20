//! Bevy `AssetLoader` for `.rvg` files.

use bevy::asset::{io::Reader, AssetLoader, LoadContext};
use bevy::reflect::TypePath;

use crate::{RvgFile, RvgLoadError};

/// Loads an `.rvg` as an [`RvgFile`] asset. Registered by the virtual geometry
/// runtime with `app.init_asset::<RvgFile>().init_asset_loader::<RvgLoader>()`.
#[derive(Default, TypePath)]
pub struct RvgLoader;

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
