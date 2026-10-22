//! Which file extensions are images, as one table every crate reads.
//!
//! # Why this exists
//!
//! "Is this file an image?" was answered by a dozen hand-written lists: the
//! asset browser's icon table, the material editor's three texture slots, the
//! particle emitter's texture slot, the sprite drop target, the UI image field,
//! the project watcher's [`AssetKind`](super::AssetKind). They had drifted (one
//! accepted `.rmip`, another `.exr`, a third neither), and adding a format meant
//! finding all of them. SVG support was the change that made that cost obvious,
//! so the lists that mean "anything that loads as a Bevy `Image`" now read this
//! one instead, and the next format is a one-line edit here.
//!
//! Lists that deliberately mean something narrower stay where they are: the
//! model importer's sibling-texture scan (it bakes `.rmip` through the `image`
//! crate, which cannot read a vector file), the export window-icon picker, the
//! terrain heightmap pickers and the reflection-probe source (equirect HDR or a
//! cube container only).

use std::path::Path;

/// Raster formats the engine loads as an `Image` through Bevy's own loaders or
/// `renzora_rmip`. Kept in step with the root manifest's Bevy image features.
pub const RASTER_IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "tga", "webp", "hdr", "exr", "ktx2", "dds", "rmip",
];

/// Vector formats, rasterized to an `Image` at load time by `renzora_svg`.
///
/// Listed apart from the raster set because a few callers care: a vector file
/// has no fixed pixel size, so a thumbnail or a "native size" button has to ask
/// the loaded `Image` for its dimensions rather than reading a header.
pub const VECTOR_IMAGE_EXTENSIONS: &[&str] = &["svg", "svgz"];

/// Every extension that loads as an `Image`: the raster set, then the vector
/// set. This is the list a texture slot, a drop target or a file picker wants.
pub const IMAGE_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "tga", "webp", "hdr", "exr", "ktx2", "dds", "rmip", "svg",
    "svgz",
];

/// True when `ext` (without the dot, any case) loads as an `Image`.
pub fn is_image_extension(ext: &str) -> bool {
    IMAGE_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// True when `ext` (without the dot, any case) is a vector image.
pub fn is_vector_image_extension(ext: &str) -> bool {
    VECTOR_IMAGE_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// True when `path`'s extension loads as an `Image`.
pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(is_image_extension)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_full_list_is_raster_then_vector() {
        // `IMAGE_EXTENSIONS` is spelled out (a `const` cannot concatenate
        // slices), so this is what keeps it from drifting off its two halves.
        let joined: Vec<&str> = RASTER_IMAGE_EXTENSIONS
            .iter()
            .chain(VECTOR_IMAGE_EXTENSIONS)
            .copied()
            .collect();
        assert_eq!(joined, IMAGE_EXTENSIONS);
    }

    #[test]
    fn svg_is_an_image_in_any_case() {
        assert!(is_image_extension("svg"));
        assert!(is_image_extension("SVG"));
        assert!(is_image_extension("svgz"));
        assert!(is_vector_image_extension("Svg"));
        assert!(!is_vector_image_extension("png"));
        assert!(is_image_path(Path::new("ui/icons/heart.svg")));
        assert!(!is_image_path(Path::new("ui/icons/heart.svg.toml")));
        assert!(!is_image_path(Path::new("README")));
    }
}
