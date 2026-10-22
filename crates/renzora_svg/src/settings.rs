//! Per-file rasterization settings, read from an optional `<file>.svg.toml`.
//!
//! # Why a sidecar and not a Bevy `.meta`
//!
//! Two facts about this engine rule `.meta` out:
//!
//! 1. The engine's asset reader (`renzora_engine::asset_reader`) answers every
//!    `read_meta` with `NotFound`, so a `.meta` beside an SVG is never read in
//!    the editor or in a shipped game.
//! 2. Sprites, tilesets and pixelated UI images all load with
//!    `with_settings(|s: &mut ImageLoaderSettings| ..)` to pick a sampler. Bevy
//!    hands those closures to the loader only when its `Settings` type is
//!    exactly `ImageLoaderSettings`; a custom type makes Bevy log a type
//!    mismatch on every such load and drop the sampler. `.rmip` hit the same
//!    wall with the glTF loader and made the same choice.
//!
//! So the loader's `Settings` is `ImageLoaderSettings` (its `is_srgb`,
//! `sampler`, `asset_usage` and `texture_format` are honoured), and the options
//! only a vector file has live here, in a small TOML file read through the same
//! asset reader as the SVG. It is a loader dependency once it exists, so
//! editing it re-rasterizes the image while the editor runs.
//!
//! ```toml
//! # heart.svg.toml
//! width = 512          # height follows the SVG's aspect ratio
//! mipmaps = true
//! fonts = ["fonts/Brand-Bold.ttf"]
//! ```

use serde::{Deserialize, Serialize};

/// The longest side an SVG rasterizes to when nothing asks for more. 4096 is
/// a size every desktop GPU and WebGPU accepts, and an SVG authored at a
/// poster size (A4 at 300 dpi is ~3500 px) should not quietly allocate 100 MB.
pub const DEFAULT_MAX_SIZE: u32 = 4096;

/// Hard ceiling even when `max_size` asks for more: wgpu's default
/// `max_texture_dimension_2d` is 8192, and a texture over it fails to upload.
pub const HARD_MAX_SIZE: u32 = 8192;

/// Rasterization options for one SVG. Every field is optional in the file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SvgRasterSettings {
    /// Output width in pixels. Alone, the height follows the aspect ratio;
    /// with `height`, the image is stretched to exactly that size.
    pub width: Option<u32>,
    /// Output height in pixels. See `width`.
    pub height: Option<u32>,
    /// Multiplier on the SVG's intrinsic size (its `width`/`height`, or its
    /// `viewBox` when those are missing). Ignored when `width` or `height` is
    /// set. `2.0` is the usual "HiDPI" choice for UI icons.
    pub scale: Option<f32>,
    /// The longest side is scaled down (aspect kept) to fit this, after
    /// `width`/`height`/`scale`. Capped at [`HARD_MAX_SIZE`].
    pub max_size: u32,
    /// Store the pixels as sRGB (`Rgba8UnormSrgb`). Overrides the loader's
    /// `ImageLoaderSettings::is_srgb` when set. Turn it off for data textures
    /// (a vector-drawn mask or flow map) that a shader reads as linear values.
    pub srgb: Option<bool>,
    /// Build a full mip chain. Each level is rasterized from the vector data
    /// at that level's size rather than downsampled from level 0, so small mips
    /// stay sharp. Off by default: UI and sprites draw near 1:1 and the chain
    /// costs a third more memory; turn it on for a texture seen at a distance
    /// (a material or a particle that shrinks).
    pub mipmaps: bool,
    /// Output premultiplied alpha instead of straight alpha.
    ///
    /// Bevy's `Image` convention, and what every PNG loads as, is straight
    /// (non-premultiplied) alpha: `AlphaMode::Blend` and UI blending multiply by
    /// alpha themselves. tiny-skia renders premultiplied, so the loader divides
    /// it back out by default. Only set this for a material using
    /// `AlphaMode::Premultiplied`, which expects the multiply already done.
    pub premultiplied_alpha: bool,
    /// Extra `.ttf`/`.otf` files for `<text>`, as paths relative to the SVG
    /// (or `/`-rooted at the project). Added to the bundled fonts.
    pub fonts: Vec<String>,
}

impl Default for SvgRasterSettings {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            scale: None,
            max_size: DEFAULT_MAX_SIZE,
            srgb: None,
            mipmaps: false,
            premultiplied_alpha: false,
            fonts: Vec::new(),
        }
    }
}

impl SvgRasterSettings {
    /// Parse a sidecar. An unknown key is an error rather than ignored, so a
    /// typo (`widht = 512`) is reported instead of silently doing nothing.
    pub fn from_toml(text: &str) -> Result<Self, String> {
        toml::from_str(text).map_err(|e| e.to_string())
    }
}

/// The sidecar path for an SVG asset path: `ui/heart.svg` -> `ui/heart.svg.toml`.
pub fn sidecar_path(svg_path: &str) -> String {
    format!("{svg_path}.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_sidecar_is_the_default() {
        assert_eq!(SvgRasterSettings::from_toml("").unwrap(), SvgRasterSettings::default());
    }

    #[test]
    fn sidecar_fields_parse() {
        let s = SvgRasterSettings::from_toml(
            "width = 64\nmipmaps = true\nsrgb = false\nfonts = [\"a.ttf\"]\n",
        )
        .unwrap();
        assert_eq!(s.width, Some(64));
        assert!(s.mipmaps);
        assert_eq!(s.srgb, Some(false));
        assert_eq!(s.fonts, vec!["a.ttf".to_string()]);
        assert_eq!(s.max_size, DEFAULT_MAX_SIZE);
    }

    #[test]
    fn a_typo_is_an_error() {
        assert!(SvgRasterSettings::from_toml("widht = 64").is_err());
    }

    #[test]
    fn sidecar_sits_beside_the_svg() {
        assert_eq!(sidecar_path("ui/heart.svg"), "ui/heart.svg.toml");
    }
}
