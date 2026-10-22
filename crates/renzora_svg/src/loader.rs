//! The `AssetLoader`: fetch everything an SVG needs through the asset reader,
//! then rasterize.
//!
//! # Hot reload
//!
//! Every file this loader reads besides the SVG (the sidecar, each embedded
//! image, each project font) goes through `LoadContext::read_asset_bytes`,
//! which records it as a *loader dependency*. Bevy reloads an asset when one of
//! its loader dependencies changes, so editing `logo.svg`, `logo.svg.toml` or a
//! PNG the SVG embeds all re-rasterize the image while the editor runs, and
//! every material, particle and UI node holding the handle picks it up.
//!
//! A file that did *not* exist at load time is not a dependency (there was
//! nothing to record), so creating a sidecar or a font afterwards takes effect
//! on the next save of the SVG itself.
//!
//! # Failure
//!
//! A load never panics the asset task: parsing and rendering run under
//! `renzora::core::recoverable::catch`, and every error is returned to Bevy
//! (which marks the handle failed and logs it) and written to the editor
//! Console with the file's path. Problems that do not stop the image (a missing
//! or refused embedded file, a bad sidecar) are Console warnings, and the
//! rest of the document still renders.

use std::collections::BTreeSet;
use std::sync::Arc;

use bevy::asset::io::{AssetReaderError, Reader};
use bevy::asset::{AssetLoader, AssetPath, LoadContext, ReadAssetBytesError};
use bevy::image::{Image, ImageLoaderSettings};
use bevy::log::warn;
use bevy::reflect::TypePath;
use renzora::core::console_log::{console_error, console_warn};
use resvg::usvg;
use thiserror::Error;

use crate::fonts;
use crate::raster::{self, FetchedImages};
use crate::refs::{self, font_file_candidates, resolve_href};
use crate::settings::{sidecar_path, SvgRasterSettings};

/// Console category for every line this crate writes.
const CATEGORY: &str = "SVG";

/// Bevy `AssetLoader` for `.svg` and `.svgz`. Produces a plain [`Image`].
///
/// `Settings` is Bevy's own `ImageLoaderSettings` so that every caller that
/// loads images with a sampler override (sprites, tilesets, pixelated UI)
/// works on an SVG unchanged; the vector-only options live in the sidecar.
/// See [`crate::settings`] for the full reasoning.
#[derive(Default, TypePath)]
pub struct SvgAssetLoader;

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

impl AssetLoader for SvgAssetLoader {
    type Asset = Image;
    type Settings = ImageLoaderSettings;
    type Error = SvgLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &ImageLoaderSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Image, SvgLoadError> {
        let path = load_context.path().to_string();
        let result = load_svg(reader, settings, load_context).await;
        if let Err(e) = &result {
            console_error(CATEGORY, format!("{path}: {e}"));
        }
        result
    }

    fn extensions(&self) -> &[&str] {
        &["svg", "svgz"]
    }
}

async fn load_svg(
    reader: &mut dyn Reader,
    image_settings: &ImageLoaderSettings,
    ctx: &mut LoadContext<'_>,
) -> Result<Image, SvgLoadError> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    let text = decode_text(&bytes)?;

    // Scope the scan so no XML borrow lives across the awaits below: the load
    // future must be `Send` on native targets.
    let scan = refs::scan(&text).map_err(|e| SvgLoadError::Parse(e.to_string()))?;

    let svg_path = ctx.path().path().to_path_buf();
    let svg_display = ctx.path().to_string();
    let source = ctx.path().source().clone_owned();
    let asset_path = |p: String| AssetPath::from(p).with_source(source.clone());

    // Sidecar settings.
    let mut settings = SvgRasterSettings::default();
    let sidecar = sidecar_path(&svg_path.to_string_lossy().replace('\\', "/"));
    match ctx.read_asset_bytes(asset_path(sidecar.clone())).await {
        Ok(raw) => match std::str::from_utf8(&raw)
            .map_err(|e| e.to_string())
            .and_then(SvgRasterSettings::from_toml)
        {
            Ok(s) => settings = s,
            Err(e) => report_warning(format!(
                "{sidecar}: {e}. Using default settings for {svg_display}."
            )),
        },
        Err(e) if is_not_found(&e) => {}
        Err(e) => report_warning(format!("{sidecar}: {e}")),
    }

    // Embedded images.
    let mut images = FetchedImages::new();
    for href in &scan.image_hrefs {
        match resolve_href(&svg_path, href) {
            Ok(resolved) => match ctx.read_asset_bytes(asset_path(resolved.clone())).await {
                Ok(data) => {
                    images.insert(href.clone(), Arc::new(data));
                }
                Err(e) => report_warning(format!(
                    "{svg_display}: embedded image `{href}` ({resolved}) could not be read: {e}"
                )),
            },
            Err(why) => report_warning(format!(
                "{svg_display}: embedded image `{href}` was refused: {why}"
            )),
        }
    }

    // Fonts. Explicit sidecar entries first, then guesses by family name for
    // any family the database cannot already answer.
    let mut fontdb = fonts::base_database();
    let mut loaded_paths = BTreeSet::new();
    for entry in &settings.fonts {
        match resolve_href(&svg_path, entry) {
            Ok(resolved) => {
                if !loaded_paths.insert(resolved.clone()) {
                    continue;
                }
                match ctx.read_asset_bytes(asset_path(resolved.clone())).await {
                    Ok(data) => Arc::make_mut(&mut fontdb).load_font_data(data),
                    Err(e) => report_warning(format!(
                        "{sidecar}: font `{entry}` ({resolved}) could not be read: {e}"
                    )),
                }
            }
            Err(why) => report_warning(format!("{sidecar}: font `{entry}` was refused: {why}")),
        }
    }
    if scan.has_text {
        for family in &scan.font_families {
            if fonts::is_known_family(&fontdb, family) {
                continue;
            }
            let mut found = false;
            'search: for file in font_file_candidates(family) {
                // `/fonts/..` is the project's fonts folder; the bare name is
                // beside the SVG. Project folder first: it is the convention.
                for href in [format!("/fonts/{file}"), file.clone()] {
                    let Ok(resolved) = resolve_href(&svg_path, &href) else {
                        continue;
                    };
                    if !loaded_paths.insert(resolved.clone()) {
                        continue;
                    }
                    if let Ok(data) = ctx.read_asset_bytes(asset_path(resolved)).await {
                        Arc::make_mut(&mut fontdb).load_font_data(data);
                        found = true;
                        break 'search;
                    }
                }
            }
            if !found {
                // Not an error: the text still renders, in the bundled font.
                warn!(
                    "{svg_display}: no font file for family `{family}`; drawing it in {}",
                    fonts::DEFAULT_FAMILY
                );
            }
        }
    }

    let rendered = renzora::core::recoverable::catch(|| {
        raster::rasterize(&text, &images, fontdb, &settings, image_settings)
    });
    match rendered {
        Ok(result) => result,
        Err(panic) => Err(SvgLoadError::Panicked(panic.to_string())),
    }
}

/// UTF-8 text from `.svg` bytes, or from gzip-compressed `.svgz` bytes
/// (detected by magic, so a misnamed file still loads).
fn decode_text(bytes: &[u8]) -> Result<String, SvgLoadError> {
    let raw = if bytes.starts_with(&[0x1f, 0x8b]) {
        usvg::decompress_svgz(bytes).map_err(|e| SvgLoadError::Parse(e.to_string()))?
    } else {
        bytes.to_vec()
    };
    let mut text = String::from_utf8(raw)
        .map_err(|_| SvgLoadError::Parse("the file is not UTF-8 text".into()))?;
    // A UTF-8 byte-order mark is legal at the start of an XML file and common
    // from Windows editors, but roxmltree rejects it before `<?xml`.
    if text.starts_with('\u{feff}') {
        text.remove(0);
    }
    Ok(text)
}

fn is_not_found(e: &ReadAssetBytesError) -> bool {
    matches!(
        e,
        ReadAssetBytesError::AssetReaderError(AssetReaderError::NotFound(_))
    )
}

/// A problem worth telling the author about that does not stop the load.
/// Console for the editor, the log for a shipped game's output.
fn report_warning(message: String) {
    warn!("{message}");
    console_warn(CATEGORY, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_handles_plain_bom_and_gzip() {
        assert_eq!(decode_text(b"<svg/>").unwrap(), "<svg/>");
        assert_eq!(decode_text("\u{feff}<svg/>".as_bytes()).unwrap(), "<svg/>");
        assert!(decode_text(&[0xff, 0xfe, 0x00]).is_err());
        // A gzip header with a corrupt body: an error, not a panic.
        assert!(decode_text(&[0x1f, 0x8b, 0x08, 0x00, 0x01]).is_err());
    }
}
