//! SVG support: `.svg` / `.svgz` files load as an ordinary Bevy [`Image`].
//!
//! # Why one loader and nothing else
//!
//! Every consumer of an image in the engine (particle textures, material
//! texture slots, UI `ImageNode`s, sprites, tilesets, asset-browser
//! thumbnails) holds a `Handle<Image>` it got from `AssetServer::load`. So the
//! whole feature is an [`AssetLoader`](bevy::asset::AssetLoader) that turns
//! vector data into an `Image` at load time: no consumer has SVG code, and the
//! only feature-side change was teaching the extension filters (now one list,
//! `renzora::IMAGE_EXTENSIONS`) that `.svg` is an image.
//!
//! # Why it loads directly and is not baked to `.rmip` on import
//!
//! `.rmip` is what the model importer writes for textures it pulls out of a
//! model; images a user drops into a project are copied as-is and loaded by
//! their own loader. SVG follows the second path on purpose. Rasterizing at
//! import would freeze one resolution into the project, lose hot reload (an
//! edit in Inkscape would need a re-import), and keep a second file in sync
//! with the first. Rasterizing at load costs milliseconds for typical icon and
//! sprite art and keeps the vector file the single source of truth.
//!
//! # Module map
//!
//! - [`settings`]: the optional `<file>.svg.toml` sidecar (size, sRGB, mips,
//!   alpha, extra fonts) and why it is a sidecar rather than a `.meta`.
//! - [`refs`]: the pre-scan that finds external `<image>` files and font
//!   families, and the sandboxed resolution of those paths.
//! - [`fonts`]: the deterministic font database (bundled Noto Sans plus project
//!   fonts, never system fonts).
//! - [`raster`]: parse + rasterize, pure and synchronous, which is what the
//!   tests drive.
//! - [`loader`]: the async `AssetLoader` that fetches everything through the
//!   asset reader and registers it for hot reload.

use bevy::prelude::*;

pub mod fonts;
pub mod loader;
pub mod raster;
pub mod refs;
pub mod settings;

pub use loader::{SvgAssetLoader, SvgLoadError};
pub use settings::SvgRasterSettings;

/// Registers [`SvgAssetLoader`] for `.svg` and `.svgz`.
#[derive(Default)]
pub struct SvgPlugin;

impl Plugin for SvgPlugin {
    fn build(&self, app: &mut App) {
        // Unconditional, like the `.rmip` loader in `renzora_engine`: gating on
        // `Assets<Image>` existing would make SVG support depend on plugin
        // install order, and fail silently if that order ever changed.
        app.init_asset_loader::<SvgAssetLoader>();
    }
}

renzora::add!(SvgPlugin, Runtime);
