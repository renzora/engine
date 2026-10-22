//! Parse and rasterize: SVG text plus already-fetched resources in, `Image` out.
//!
//! Pure and synchronous on purpose. Everything that touches the asset reader
//! is in [`crate::loader`]; this half has no I/O, so the tests drive it
//! directly and the loader can run it under `recoverable::catch`.

use std::collections::HashMap;
use std::sync::Arc;

use bevy::image::{Image, ImageLoaderSettings};
use bevy::render::render_resource::{
    Extent3d, TextureDataOrder, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use resvg::tiny_skia;
use resvg::usvg::{self, fontdb, ImageHrefResolver, ImageKind};

use crate::fonts::DEFAULT_FAMILY;
use crate::loader::SvgLoadError;
use crate::settings::{SvgRasterSettings, HARD_MAX_SIZE};

/// External files the loader fetched, keyed by the `href` exactly as the
/// document wrote it (trimmed). usvg hands the resolver that raw string, so
/// the key has to be the raw string too, not the resolved asset path.
pub type FetchedImages = HashMap<String, Arc<Vec<u8>>>;

/// Rasterized pixels: mip 0 first, then each smaller level, RGBA8.
#[derive(Clone, Debug)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub mip_levels: u32,
    pub data: Vec<u8>,
}

/// Parse `text` into a usvg tree, answering `<image>` references from
/// `images` and shaping text against `fontdb`.
pub fn parse(
    text: &str,
    images: &FetchedImages,
    fontdb: Arc<fontdb::Database>,
) -> Result<usvg::Tree, SvgLoadError> {
    // The resolver closures must be `'static`-free only for the lifetime of
    // `opts`, so borrowing `images` here is fine.
    let resolve_string = move |href: &str, opts: &usvg::Options| -> Option<ImageKind> {
        let data = images.get(href.trim())?.clone();
        image_kind(href, data, opts)
    };
    let opts = usvg::Options {
        font_family: DEFAULT_FAMILY.to_string(),
        fontdb,
        image_href_resolver: ImageHrefResolver {
            // `data:` URIs need no I/O, so usvg's own resolver is right for
            // them (PNG, JPEG, GIF, WebP and nested SVG by MIME or magic).
            resolve_data: ImageHrefResolver::default_data_resolver(),
            resolve_string: Box::new(resolve_string),
        },
        ..usvg::Options::default()
    };
    usvg::Tree::from_str(text, &opts).map_err(|e| SvgLoadError::Parse(e.to_string()))
}

/// Classify fetched bytes the way usvg's own file resolver does: by magic for
/// raster formats, by extension (or gzip magic) for SVG.
fn image_kind(href: &str, data: Arc<Vec<u8>>, opts: &usvg::Options) -> Option<ImageKind> {
    let d = data.as_slice();
    if d.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(ImageKind::PNG(data));
    }
    if d.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(ImageKind::JPEG(data));
    }
    if d.starts_with(b"GIF87a") || d.starts_with(b"GIF89a") {
        return Some(ImageKind::GIF(data));
    }
    if d.len() >= 12 && &d[0..4] == b"RIFF" && &d[8..12] == b"WEBP" {
        return Some(ImageKind::WEBP(data));
    }
    let lower = href.to_ascii_lowercase();
    let path = lower.split(['?', '#']).next().unwrap_or_default();
    if path.ends_with(".svg") || path.ends_with(".svgz") || d.starts_with(&[0x1f, 0x8b]) {
        // `from_data_nested` ignores the nested file's own external images, as
        // the SVG spec requires for an SVG used as an image. That also means a
        // nested file never reads anything this loader did not fetch.
        return usvg::Tree::from_data_nested(d, opts).ok().map(ImageKind::SVG);
    }
    None
}

/// The pixel size to rasterize at, given the tree's intrinsic size.
pub fn target_size(intrinsic: (f32, f32), s: &SvgRasterSettings) -> Result<(u32, u32), SvgLoadError> {
    let (iw, ih) = intrinsic;
    if !(iw.is_finite() && ih.is_finite() && iw > 0.0 && ih > 0.0) {
        return Err(SvgLoadError::Parse("the SVG has no usable size".into()));
    }
    let (w, h) = match (s.width, s.height) {
        (Some(0), _) | (_, Some(0)) => {
            return Err(SvgLoadError::Settings("width and height must be at least 1".into()))
        }
        (Some(w), Some(h)) => (w as f32, h as f32),
        (Some(w), None) => (w as f32, w as f32 * ih / iw),
        (None, Some(h)) => (h as f32 * iw / ih, h as f32),
        (None, None) => {
            let scale = s.scale.unwrap_or(1.0);
            if !(scale.is_finite() && scale > 0.0) {
                return Err(SvgLoadError::Settings(format!(
                    "scale must be a positive number, got {scale}"
                )));
            }
            (iw * scale, ih * scale)
        }
    };
    let cap = s.max_size.clamp(1, HARD_MAX_SIZE) as f32;
    let longest = w.max(h);
    let fit = if longest > cap { cap / longest } else { 1.0 };
    let w = ((w * fit).round() as u32).clamp(1, HARD_MAX_SIZE);
    let h = ((h * fit).round() as u32).clamp(1, HARD_MAX_SIZE);
    Ok((w, h))
}

/// Render `tree` at `width` x `height` (stretching to that box), then the mip
/// chain when asked for. Background is transparent.
pub fn render(tree: &usvg::Tree, width: u32, height: u32, s: &SvgRasterSettings) -> Result<Raster, SvgLoadError> {
    let levels = if s.mipmaps {
        32 - width.max(height).leading_zeros()
    } else {
        1
    };
    let size = tree.size();
    let mut data = Vec::new();
    for level in 0..levels {
        let lw = (width >> level).max(1);
        let lh = (height >> level).max(1);
        let mut pixmap = tiny_skia::Pixmap::new(lw, lh)
            .ok_or_else(|| SvgLoadError::Settings(format!("cannot allocate {lw}x{lh} pixels")))?;
        let transform = tiny_skia::Transform::from_scale(
            lw as f32 / size.width(),
            lh as f32 / size.height(),
        );
        resvg::render(tree, transform, &mut pixmap.as_mut());
        if s.premultiplied_alpha {
            data.extend_from_slice(pixmap.data());
        } else {
            // tiny-skia stores premultiplied RGBA; Bevy's `Image` convention is
            // straight alpha (see `SvgRasterSettings::premultiplied_alpha`).
            data.reserve(pixmap.data().len());
            for px in pixmap.pixels() {
                let c = px.demultiply();
                data.extend_from_slice(&[c.red(), c.green(), c.blue(), c.alpha()]);
            }
        }
    }
    Ok(Raster {
        width,
        height,
        mip_levels: levels,
        data,
    })
}

/// Wrap pixels in an `Image`, honouring the parts of `ImageLoaderSettings`
/// that mean something for an RGBA8 raster.
pub fn to_image(raster: Raster, srgb: bool, image_settings: &ImageLoaderSettings) -> Image {
    let format = match image_settings.texture_format {
        // Only the two layouts the bytes actually are; anything else (`R8`,
        // `Bgra8`, a float format) would reinterpret RGBA8 as garbage.
        Some(f @ (TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb)) => f,
        _ if srgb => TextureFormat::Rgba8UnormSrgb,
        _ => TextureFormat::Rgba8Unorm,
    };
    Image {
        data: Some(raster.data),
        // Mip 0 first, then each smaller level: Bevy's default `MipMajor`.
        data_order: TextureDataOrder::default(),
        texture_descriptor: TextureDescriptor {
            label: None,
            size: Extent3d {
                width: raster.width,
                height: raster.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: raster.mip_levels,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        sampler: image_settings.sampler.clone(),
        texture_view_descriptor: None,
        asset_usage: image_settings.asset_usage,
        copy_on_resize: false,
    }
}

/// Everything after the fetches, in one call: parse, size, render, wrap.
pub fn rasterize(
    text: &str,
    images: &FetchedImages,
    fontdb: Arc<fontdb::Database>,
    settings: &SvgRasterSettings,
    image_settings: &ImageLoaderSettings,
) -> Result<Image, SvgLoadError> {
    let tree = parse(text, images, fontdb)?;
    let size = tree.size();
    let (w, h) = target_size((size.width(), size.height()), settings)?;
    let raster = render(&tree, w, h, settings)?;
    let srgb = settings.srgb.unwrap_or(image_settings.is_srgb);
    Ok(to_image(raster, srgb, image_settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::base_database;

    const RED_SQUARE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">
        <rect x="0" y="0" width="10" height="10" fill="rgb(255,0,0)"/>
    </svg>"#;

    fn px(image: &Image, x: u32, y: u32) -> [u8; 4] {
        let w = image.texture_descriptor.size.width;
        let i = ((y * w + x) * 4) as usize;
        let d = image.data.as_ref().unwrap();
        [d[i], d[i + 1], d[i + 2], d[i + 3]]
    }

    fn raster(text: &str, s: &SvgRasterSettings, images: &FetchedImages) -> Image {
        rasterize(text, images, base_database(), s, &ImageLoaderSettings::default()).unwrap()
    }

    /// A tiny PNG encoded by tiny-skia, so the test needs no image crate.
    fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut p = tiny_skia::Pixmap::new(w, h).unwrap();
        p.fill(tiny_skia::Color::from_rgba8(rgba[0], rgba[1], rgba[2], rgba[3]));
        p.encode_png().unwrap()
    }

    #[test]
    fn intrinsic_size_and_pixels() {
        let img = raster(RED_SQUARE, &SvgRasterSettings::default(), &FetchedImages::new());
        assert_eq!(img.texture_descriptor.size.width, 20);
        assert_eq!(img.texture_descriptor.size.height, 10);
        assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba8UnormSrgb);
        assert_eq!(img.texture_descriptor.mip_level_count, 1);
        assert_eq!(img.data.as_ref().unwrap().len(), 20 * 10 * 4);
        assert_eq!(px(&img, 2, 5), [255, 0, 0, 255]);
        // Right half: transparent background, not white or black.
        assert_eq!(px(&img, 15, 5)[3], 0);
    }

    #[test]
    fn settings_change_the_size() {
        let empty = FetchedImages::new();
        let by_width = SvgRasterSettings { width: Some(40), ..Default::default() };
        let img = raster(RED_SQUARE, &by_width, &empty);
        assert_eq!((img.texture_descriptor.size.width, img.texture_descriptor.size.height), (40, 20));
        // Scaled up, the red half is still red at the new coordinates.
        assert_eq!(px(&img, 5, 10), [255, 0, 0, 255]);

        let by_scale = SvgRasterSettings { scale: Some(0.5), ..Default::default() };
        let img = raster(RED_SQUARE, &by_scale, &empty);
        assert_eq!((img.texture_descriptor.size.width, img.texture_descriptor.size.height), (10, 5));

        let both = SvgRasterSettings { width: Some(8), height: Some(8), ..Default::default() };
        let img = raster(RED_SQUARE, &both, &empty);
        assert_eq!((img.texture_descriptor.size.width, img.texture_descriptor.size.height), (8, 8));

        let capped = SvgRasterSettings { width: Some(10_000), max_size: 100, ..Default::default() };
        let img = raster(RED_SQUARE, &capped, &empty);
        assert_eq!((img.texture_descriptor.size.width, img.texture_descriptor.size.height), (100, 50));
    }

    #[test]
    fn bad_settings_are_errors() {
        let s = SvgRasterSettings { scale: Some(-1.0), ..Default::default() };
        assert!(target_size((10.0, 10.0), &s).is_err());
        let s = SvgRasterSettings { width: Some(0), ..Default::default() };
        assert!(target_size((10.0, 10.0), &s).is_err());
    }

    #[test]
    fn mipmaps_are_a_full_chain() {
        let s = SvgRasterSettings { mipmaps: true, ..Default::default() };
        let img = raster(RED_SQUARE, &s, &FetchedImages::new());
        // 20x10 -> 10x5 -> 5x2 -> 2x1 -> 1x1
        assert_eq!(img.texture_descriptor.mip_level_count, 5);
        let expected: usize = [(20, 10), (10, 5), (5, 2), (2, 1), (1, 1)]
            .iter()
            .map(|(w, h)| w * h * 4)
            .sum();
        assert_eq!(img.data.as_ref().unwrap().len(), expected);
    }

    #[test]
    fn straight_alpha_by_default_premultiplied_on_request() {
        let half = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2">
            <rect width="2" height="2" fill="rgb(255,0,0)" fill-opacity="0.5"/></svg>"#;
        let straight = raster(half, &SvgRasterSettings::default(), &FetchedImages::new());
        let p = px(&straight, 0, 0);
        assert_eq!(p[0], 255, "straight alpha keeps full red");
        assert!((p[3] as i32 - 128).abs() <= 1);

        let s = SvgRasterSettings { premultiplied_alpha: true, ..Default::default() };
        let pre = raster(half, &s, &FetchedImages::new());
        let p = px(&pre, 0, 0);
        assert!((p[0] as i32 - 128).abs() <= 1, "premultiplied red is scaled by alpha");
    }

    #[test]
    fn srgb_can_be_turned_off() {
        let s = SvgRasterSettings { srgb: Some(false), ..Default::default() };
        let img = raster(RED_SQUARE, &s, &FetchedImages::new());
        assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba8Unorm);
        let via_loader = ImageLoaderSettings { is_srgb: false, ..Default::default() };
        let img = rasterize(RED_SQUARE, &FetchedImages::new(), base_database(), &SvgRasterSettings::default(), &via_loader).unwrap();
        assert_eq!(img.texture_descriptor.format, TextureFormat::Rgba8Unorm);
    }

    /// Standard base64, written out so the test pulls in no encoder crate.
    fn base64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[test]
    fn embedded_data_uri_png_renders() {
        let uri = format!("data:image/png;base64,{}", base64(&png(4, 4, [0, 0, 255, 255])));
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
            <image href="{uri}" x="0" y="0" width="4" height="4"/></svg>"#
        );
        let img = raster(&svg, &SvgRasterSettings::default(), &FetchedImages::new());
        assert_eq!(px(&img, 1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn relative_png_renders_from_fetched_bytes() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4">
            <image href="img/green.png" x="0" y="0" width="4" height="4"/></svg>"#;
        let mut images = FetchedImages::new();
        images.insert("img/green.png".into(), Arc::new(png(4, 4, [0, 255, 0, 255])));
        let img = raster(svg, &SvgRasterSettings::default(), &images);
        assert_eq!(px(&img, 2, 2), [0, 255, 0, 255]);

        // Not fetched (refused or missing): the image is skipped, the rest of
        // the document still renders, and nothing panics.
        let img = raster(svg, &SvgRasterSettings::default(), &FetchedImages::new());
        assert_eq!(px(&img, 2, 2)[3], 0);
    }

    #[test]
    fn text_renders_with_the_bundled_font_even_for_a_missing_family() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="64" height="32">
            <text x="2" y="26" font-size="28" font-family="Definitely Not Installed" fill="black">Hi</text></svg>"#;
        let img = raster(svg, &SvgRasterSettings::default(), &FetchedImages::new());
        let d = img.data.as_ref().unwrap();
        assert!(d.chunks_exact(4).any(|p| p[3] > 0), "fallback font drew glyphs");
    }

    #[test]
    fn malformed_svg_is_an_error_not_a_panic() {
        let empty = FetchedImages::new();
        let s = SvgRasterSettings::default();
        let i = ImageLoaderSettings::default();
        assert!(rasterize("<svg><g></svg>", &empty, base_database(), &s, &i).is_err());
        assert!(rasterize("not xml at all", &empty, base_database(), &s, &i).is_err());
        // Well-formed XML with no size and no viewBox: usvg's InvalidSize.
        assert!(rasterize(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"/>"#,
            &empty,
            base_database(),
            &s,
            &i
        )
        .is_err());
    }
}
