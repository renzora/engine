//! Asset-kind classification for the importer.
//!
//! The importer now accepts more than 3D models. Every permitted file falls
//! into one of two buckets:
//!
//! * **Models** (`AssetKind::Model`) — run through the full glTF/GLB conversion
//!   pipeline (`renzora_import`) with the model-only options (scale, up-axis,
//!   extract, optimize).
//! * **Everything else** (images, audio, scenes, particles, materials, fonts,
//!   scripts) — has no conversion step. "Importing" one just **copies the file
//!   verbatim** into the destination folder the user picks. There's nothing to
//!   transform, so the overlay hides the model-only panes and the worker does a
//!   plain `fs::copy`.
//!
//! Keeping this classification in the UI crate (rather than `renzora_import`)
//! avoids growing the import *backend*'s public surface for what is really a
//! UI-side routing decision — model detection still delegates to
//! `renzora_import::formats`.

use std::path::Path;

/// The category a to-be-imported file belongs to. Only [`AssetKind::Model`]
/// needs conversion; every other variant is copied as-is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    /// A 3D model (glTF/GLB/FBX/OBJ/…). Converted to GLB on import.
    Model,
    /// A raster/HDR image. Copied as a texture source.
    Image,
    /// An audio clip. Copied.
    Audio,
    /// A `.bsn` scene / prefab. Copied.
    Scene,
    /// A `.particle` effect asset. Copied.
    Particle,
    /// A `.material` graph asset. Copied.
    Material,
    /// A font (`.ttf` / `.otf`). Copied.
    Font,
    /// A script (`.lua` / `.rhai`). Copied.
    Script,
}

impl AssetKind {
    /// True for the one kind that goes through GLB conversion; the rest are
    /// copied. The worker and the overlay both branch on this.
    pub fn is_model(self) -> bool {
        matches!(self, AssetKind::Model)
    }
}

/// Image extensions the importer will copy in as textures. Broader than the
/// asset browser's *thumbnail* set (which excludes EXR/KTX2/DDS because Bevy's
/// loaders choke on some of them) — importing only copies bytes, so any texture
/// container the engine can consume at runtime is fair game.
pub const IMAGE_EXTS: &[&str] = &[
    "png", "jpg", "jpeg", "bmp", "tga", "webp", "hdr", "exr", "ktx2", "dds",
];
/// Audio extensions the importer will copy in.
pub const AUDIO_EXTS: &[&str] = &["wav", "ogg", "mp3", "flac"];
/// Scene / prefab extensions.
pub const SCENE_EXTS: &[&str] = &["bsn"];
/// Font extensions.
pub const FONT_EXTS: &[&str] = &["ttf", "otf"];
/// Script extensions.
pub const SCRIPT_EXTS: &[&str] = &["lua", "rhai"];

/// Classify a path by extension. Models are detected via the import backend so
/// the model list stays single-sourced; other kinds match the tables above.
/// Returns `None` for anything the importer doesn't accept.
pub fn detect_kind(path: &Path) -> Option<AssetKind> {
    // Models first — `renzora_import` owns the authoritative model extension
    // list, so we never duplicate it here.
    if renzora_import::formats::is_supported(path) {
        return Some(AssetKind::Model);
    }
    let ext = path.extension()?.to_str()?.to_lowercase();
    let ext = ext.as_str();
    let has = |set: &[&str]| set.contains(&ext);
    Some(if has(IMAGE_EXTS) {
        AssetKind::Image
    } else if has(AUDIO_EXTS) {
        AssetKind::Audio
    } else if has(SCENE_EXTS) {
        AssetKind::Scene
    } else if ext == "particle" {
        AssetKind::Particle
    } else if ext == "material" {
        AssetKind::Material
    } else if has(FONT_EXTS) {
        AssetKind::Font
    } else if has(SCRIPT_EXTS) {
        AssetKind::Script
    } else {
        return None;
    })
}

/// True if the importer accepts this file at all (model or copyable asset).
/// Used to filter both OS-dialog picks and drag-and-drop.
pub fn is_importable(path: &Path) -> bool {
    detect_kind(path).is_some()
}

/// Every accepted extension, flattened — the "All importable" dialog filter.
pub fn all_importable_extensions() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = renzora_import::supported_extensions().to_vec();
    v.extend_from_slice(IMAGE_EXTS);
    v.extend_from_slice(AUDIO_EXTS);
    v.extend_from_slice(SCENE_EXTS);
    v.push("particle");
    v.push("material");
    v.extend_from_slice(FONT_EXTS);
    v.extend_from_slice(SCRIPT_EXTS);
    v
}

/// A phosphor icon name + accent colour for a queued file, chosen by kind so the
/// file list reads at a glance (a texture, a sound and a model look distinct).
pub fn kind_icon(path: &Path) -> (&'static str, (u8, u8, u8)) {
    match detect_kind(path) {
        Some(AssetKind::Model) | None => ("cube", (255, 170, 100)),
        Some(AssetKind::Image) => ("image", (120, 180, 255)),
        Some(AssetKind::Audio) => ("music-notes", (200, 140, 255)),
        Some(AssetKind::Scene) => ("stack", (140, 220, 180)),
        Some(AssetKind::Particle) => ("sparkle", (255, 210, 120)),
        Some(AssetKind::Material) => ("circle-half", (180, 185, 205)),
        Some(AssetKind::Font) => ("text-aa", (205, 205, 210)),
        Some(AssetKind::Script) => ("code", (150, 205, 150)),
    }
}

/// Open the OS file picker filtered to everything the importer accepts. Returns
/// the chosen paths (empty/`None` if the user cancelled). Blocking — the caller
/// runs it on `&mut World`, same as the old model-only Browse button.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pick_importable_files() -> Option<Vec<std::path::PathBuf>> {
    let all = all_importable_extensions();
    rfd::FileDialog::new()
        .set_title("Select files to import")
        .add_filter("All importable", &all)
        .add_filter("3D Models", renzora_import::supported_extensions())
        .add_filter("Images", IMAGE_EXTS)
        .add_filter("Audio", AUDIO_EXTS)
        .add_filter("Scenes / Prefabs", SCENE_EXTS)
        .add_filter("Particles", &["particle"])
        .add_filter("Materials", &["material"])
        .add_filter("Fonts", FONT_EXTS)
        .add_filter("Scripts", SCRIPT_EXTS)
        .add_filter("All Files", &["*"])
        .pick_files()
        .filter(|p| !p.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn models_route_to_model_kind() {
        assert_eq!(detect_kind(Path::new("a.glb")), Some(AssetKind::Model));
        assert_eq!(detect_kind(Path::new("a.fbx")), Some(AssetKind::Model));
        assert!(detect_kind(Path::new("a.glb")).unwrap().is_model());
    }

    #[test]
    fn non_models_route_to_copy_kinds() {
        assert_eq!(detect_kind(Path::new("t.png")), Some(AssetKind::Image));
        assert_eq!(detect_kind(Path::new("s.WAV")), Some(AssetKind::Audio));
        assert_eq!(detect_kind(Path::new("lvl.bsn")), Some(AssetKind::Scene));
        assert_eq!(detect_kind(Path::new("fx.particle")), Some(AssetKind::Particle));
        assert_eq!(detect_kind(Path::new("m.material")), Some(AssetKind::Material));
        assert_eq!(detect_kind(Path::new("f.ttf")), Some(AssetKind::Font));
        assert_eq!(detect_kind(Path::new("g.rhai")), Some(AssetKind::Script));
        for k in [AssetKind::Image, AssetKind::Audio, AssetKind::Scene] {
            assert!(!k.is_model());
        }
    }

    #[test]
    fn unknown_extensions_are_rejected() {
        assert_eq!(detect_kind(Path::new("a.txt")), None);
        assert_eq!(detect_kind(Path::new("noext")), None);
        assert!(!is_importable(Path::new("a.txt")));
    }

    #[test]
    fn all_importable_covers_every_kind() {
        // Each advertised extension must classify to *some* kind.
        for ext in all_importable_extensions() {
            let name = format!("file.{}", ext);
            assert!(
                detect_kind(Path::new(&name)).is_some(),
                "extension {} advertised but not classified",
                ext
            );
        }
    }
}
