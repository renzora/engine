//! Turning the author's picked icon file into the two shapes an export needs.
//!
//! An exported game wants the same picture in two completely different places,
//! and until now it only ever reached one of them:
//!
//! * **The executable's resource table** — what Explorer, the taskbar's pinned
//!   entry, Alt-Tab and the Properties dialog read. This is baked in at *compile
//!   time* by the root `build.rs` from a file literally named `icon.ico`, so the
//!   only way to change it is to put a different `icon.ico` in front of the
//!   compiler. That is what [`to_ico`] produces and what
//!   [`crate::build::stage_branding`] writes into the export workspace.
//! * **The window icon** — set at runtime by `renzora_runtime::apply_window_icon`,
//!   which reads `assets/icon.png` out of the rpak and hands the pixels to winit.
//!
//! Both used to be fed the author's file *unconverted*, which is why the second
//! one quietly failed for anything but a PNG: the bytes were stored under the
//! name `assets/icon.png` whatever they actually were, and the runtime's `image`
//! has no `ico` feature to fall back on. Decoding once here and re-encoding for
//! each destination means the picker can accept any raster format the editor can
//! read without the runtime needing to know about it.

use std::path::Path;

use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::codecs::png::PngEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};

/// The square the source is fitted into before anything is derived from it.
///
/// 256 is the largest an ICO directory entry can describe (the width/height
/// fields are single bytes, with 0 meaning 256), so there is no point carrying
/// more, and every smaller frame is a downscale of this one rather than a
/// separate resample of the original.
const BASE: u32 = 256;

/// Frame sizes written into the `.ico`.
///
/// Windows picks per context — 16 in a list view, 32 on the desktop, 48 in
/// medium icons, 256 in the preview pane — and silently rescales the nearest
/// match when a size is absent, which is what makes a single-frame icon look
/// muddy in the places it did not anticipate. Downscaling is cheap and the whole
/// file lands around 100 KB, so we simply provide all of them.
const ICO_SIZES: [u32; 6] = [256, 128, 64, 48, 32, 16];

/// Extensions the icon picker offers.
///
/// Deliberately the intersection of "the editor's `image` can decode it" and
/// "someone might plausibly have their icon in it" — `ico` is in the list only
/// because this crate turns the `ico` feature on for the encoder and gets the
/// decoder with it. SVG is NOT here: `image` has no vector support at any
/// feature level, so offering it (as the picker used to) could only ever end in
/// a decode failure two steps later, with the warning buried in the export log.
pub const PICKABLE: &[&str] = &["png", "ico", "jpg", "jpeg", "bmp", "webp", "tga"];

/// Decode `path` and fit it into a `BASE`×`BASE` RGBA square.
///
/// Non-square sources are letterboxed with transparency rather than stretched or
/// cropped: an icon is displayed at whatever aspect the shell feels like, and a
/// distorted logo reads as a bug in a way that some empty margin does not.
pub fn load_square(path: &Path) -> Result<RgbaImage, String> {
    let img = image::ImageReader::open(path)
        .map_err(|e| format!("open {}: {e}", path.display()))?
        .with_guessed_format()
        .map_err(|e| format!("read {}: {e}", path.display()))?
        .decode()
        .map_err(|e| {
            format!(
                "{} is not an image this editor can read ({e}). Supported: {}.",
                path.display(),
                PICKABLE.join(", ")
            )
        })?
        .to_rgba8();

    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err(format!("{} has zero size", path.display()));
    }
    if w == h {
        return Ok(image::imageops::resize(&img, BASE, BASE, FilterType::Lanczos3));
    }

    // Scale the long edge to BASE, then centre it on a transparent canvas.
    let scale = BASE as f32 / w.max(h) as f32;
    let (nw, nh) = (
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
    );
    let scaled = image::imageops::resize(&img, nw, nh, FilterType::Lanczos3);
    let mut canvas = RgbaImage::new(BASE, BASE);
    image::imageops::replace(
        &mut canvas,
        &scaled,
        ((BASE - nw) / 2) as i64,
        ((BASE - nh) / 2) as i64,
    );
    Ok(canvas)
}

/// Encode a multi-size Windows `.ico` from a square RGBA base.
///
/// Every frame is a PNG payload, which is what `image`'s own single-frame
/// encoder emits and what Windows has understood since Vista. `rc.exe` and
/// `llvm-rc` both copy the frames into `RT_ICON` verbatim and synthesise the
/// `RT_GROUP_ICON` directory from the header, so neither cares which payload
/// form we chose.
pub fn to_ico(base: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut frames = Vec::with_capacity(ICO_SIZES.len());
    for size in ICO_SIZES {
        let scaled = if size == BASE {
            base.clone()
        } else {
            image::imageops::resize(base, size, size, FilterType::Lanczos3)
        };
        frames.push(
            IcoFrame::as_png(scaled.as_raw(), size, size, ExtendedColorType::Rgba8)
                .map_err(|e| format!("encode {size}px icon frame: {e}"))?,
        );
    }
    let mut out = Vec::new();
    IcoEncoder::new(&mut out)
        .encode_images(&frames)
        .map_err(|e| format!("encode .ico: {e}"))?;
    Ok(out)
}

/// A neutral 256×256 icon, for a project that has set none.
///
/// Needed because an AppImage cannot be built without one: the `.desktop` entry
/// must name an icon and `appimagetool` refuses an `AppDir` where that file is
/// absent — "test{.png,.svg,.xpm} defined in desktop file but not found" — so a
/// project with no artwork could not be packaged at all.
///
/// Deliberately NOT the engine's own `icon.png`. Shipping Renzora's logo as the
/// icon of somebody's game is a worse default than an obvious placeholder: it is
/// wrong in a way that looks intentional, and it would travel all the way to a
/// store page before anyone noticed.
///
/// A dark square with an inset border, so it reads as "no icon yet" rather than
/// as a rendering failure.
pub fn placeholder() -> RgbaImage {
    const EDGE: u32 = 18;
    RgbaImage::from_fn(BASE, BASE, |x, y| {
        let inset = x >= EDGE && y >= EDGE && x < BASE - EDGE && y < BASE - EDGE;
        if inset {
            image::Rgba([32, 34, 40, 255])
        } else {
            image::Rgba([58, 62, 74, 255])
        }
    })
}

/// Encode the square base as a PNG, for the rpak entry the runtime hands winit.
pub fn to_png(base: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    PngEncoder::new(&mut out)
        .write_image(base.as_raw(), BASE, BASE, ExtendedColorType::Rgba8)
        .map_err(|e| format!("encode icon PNG: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checkerboard(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            if (x / 8 + y / 8) % 2 == 0 {
                image::Rgba([220, 40, 90, 255])
            } else {
                image::Rgba([20, 30, 60, 255])
            }
        })
    }

    /// The `.ico` has to be readable back as an icon with every requested frame.
    /// A malformed one does not fail the export — `llvm-rc`/`rc.exe` would fail
    /// the *build*, minutes later, with a message about a resource file the
    /// author never asked for.
    #[test]
    fn ico_round_trips_with_every_frame() {
        let ico = to_ico(&checkerboard(BASE, BASE)).expect("encode");
        let decoded = image::ImageReader::with_format(
            std::io::Cursor::new(&ico),
            image::ImageFormat::Ico,
        )
        .decode()
        .expect("re-decode .ico");
        // The ICO decoder hands back the largest frame; that it found one at all
        // means the directory and offsets line up.
        assert_eq!(decoded.width(), 256);
        assert_eq!(decoded.height(), 256);
        // Header: reserved(0), type(1 = icon), then the frame count.
        assert_eq!(&ico[0..4], &[0, 0, 1, 0]);
        assert_eq!(
            u16::from_le_bytes([ico[4], ico[5]]) as usize,
            ICO_SIZES.len()
        );
    }

    /// A wide source must be letterboxed, not stretched — so the centre row
    /// keeps the source's colours and the top and bottom edges stay empty.
    #[test]
    fn non_square_is_letterboxed_not_stretched() {
        let mut src = std::io::Cursor::new(Vec::new());
        checkerboard(512, 128)
            .write_to(&mut src, image::ImageFormat::Png)
            .expect("encode source");
        let dir = std::env::temp_dir().join("renzora_icon_test");
        std::fs::create_dir_all(&dir).expect("mkdir");
        let path = dir.join("wide.png");
        std::fs::write(&path, src.into_inner()).expect("write source");

        let base = load_square(&path).expect("load");
        assert_eq!(base.dimensions(), (BASE, BASE));
        // 512×128 scales to 256×64, centred: rows 96..160 carry pixels.
        assert_eq!(base.get_pixel(0, 0)[3], 0, "top edge should be transparent");
        assert_eq!(base.get_pixel(0, BASE - 1)[3], 0, "bottom edge should be transparent");
        assert_eq!(base.get_pixel(BASE / 2, BASE / 2)[3], 255, "centre should be opaque");

        let _ = std::fs::remove_file(&path);
    }
}
