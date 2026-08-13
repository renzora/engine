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

use std::path::{Path, PathBuf};

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
    /// A gaussian-splat cloud (`.gcloud`, or a `.ply` that is a 3DGS capture
    /// or a faceless point cloud). Copied — the splat renderer loads these
    /// directly (synthesizing splats for plain points), unlike mesh PLYs
    /// which convert to GLB.
    GaussianSplat,
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
pub const SCRIPT_EXTS: &[&str] = &["lua"];

/// True when a `.ply` file belongs to the splat renderer rather than the mesh
/// converter.
///
/// `.ply` is genuinely ambiguous: it's a classic mesh format (routed to GLB
/// conversion), the de-facto 3DGS splat format, AND a common plain point-cloud
/// container (CloudCompare / LiDAR / Sketchfab downloads). The header is ASCII
/// even in binary PLYs, so an 8 KiB header peek classifies reliably:
///
/// * `f_dc_0` (3DGS spherical-harmonics property no mesh PLY has) → splat;
/// * otherwise, no faces → point cloud → also splat: the splat loader
///   synthesizes isotropic gaussians for plain points, while the mesh
///   converter would have no triangles to convert;
/// * faces present → mesh → GLB conversion, the old behaviour.
///
/// Unreadable / truncated-header files fall back to `false` (mesh).
fn is_gaussian_ply(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut buf = [0u8; 8192];
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    let text = String::from_utf8_lossy(&buf[..n]);
    if !text.starts_with("ply") {
        return false;
    }
    let complete = text.contains("end_header");
    let header = text.split("end_header").next().unwrap_or("");
    if header.contains("f_dc_0") {
        return true;
    }
    // Faceless = point cloud. Only trust the absence of faces when the whole
    // header fit in the peek buffer.
    let face_count: u64 = header
        .lines()
        .find_map(|line| line.strip_prefix("element face "))
        .and_then(|count| count.trim().parse().ok())
        .unwrap_or(0);
    complete && face_count == 0
}

/// Classify a path by extension. Models are detected via the import backend so
/// the model list stays single-sourced; other kinds match the tables above.
/// Returns `None` for anything the importer doesn't accept.
pub fn detect_kind(path: &Path) -> Option<AssetKind> {
    // Splat clouds first: `.ply` is ALSO a model extension, so a splat `.ply`
    // (detected by header sniff) must claim the file before the model check
    // routes it into GLB conversion, which would garble point-cloud data.
    let sniffed_ext = path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase());
    match sniffed_ext.as_deref() {
        Some("gcloud" | "sog" | "ssog") => return Some(AssetKind::GaussianSplat),
        Some("ply") if is_gaussian_ply(path) => return Some(AssetKind::GaussianSplat),
        _ => {}
    }
    // Models next — `renzora_import` owns the authoritative model extension
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
    // "ply" is already advertised by the model extension list above.
    v.push("gcloud");
    v.push("sog");
    v.push("ssog");
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
        Some(AssetKind::GaussianSplat) => ("cloud", (190, 150, 255)),
    }
}

/// A path queued for import, optionally carrying the source-tree subdirectory
/// it should recreate under the destination.
///
/// Folder imports (Browse folder / drop a directory) fill [`relative_dir`] so
/// `Pack/textures/a.png` lands as `<target>/Pack/textures/a.png` instead of
/// flattening everything into `<target>/`. Single-file picks leave it empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedAsset {
    pub path: PathBuf,
    /// Forward-slashed directory under the import target (includes the selected
    /// folder's name + any subfolders). Empty = land directly in the target.
    /// Does **not** include the filename.
    pub relative_dir: String,
}

impl QueuedAsset {
    pub fn flat(path: PathBuf) -> Self {
        Self {
            path,
            relative_dir: String::new(),
        }
    }
}

/// Collect every importable file under `path`. A single file is returned as a
/// flat queue entry when it passes [`is_importable`]; a directory is walked
/// recursively and every matching file keeps its path relative to that
/// directory (including the directory's own name), so the import worker can
/// mirror the tree under the destination.
pub(crate) fn expand_importables(path: &Path) -> Vec<QueuedAsset> {
    if path.is_file() {
        return if is_importable(path) {
            vec![QueuedAsset::flat(path.to_path_buf())]
        } else {
            Vec::new()
        };
    }
    if !path.is_dir() {
        return Vec::new();
    }
    // Include the selected folder's name so two packs don't collide when both
    // have a top-level `textures/` — Unreal-style "import this folder as a
    // subtree".
    let root_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("import");
    let mut out = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if is_importable(&p) {
                let Ok(rel) = p.strip_prefix(path) else {
                    continue;
                };
                let relative_dir = match rel.parent().filter(|par| !par.as_os_str().is_empty()) {
                    Some(par) => {
                        let sub = par.to_string_lossy().replace('\\', "/");
                        format!("{root_name}/{sub}")
                    }
                    None => root_name.to_string(),
                };
                out.push(QueuedAsset {
                    path: p,
                    relative_dir,
                });
            }
        }
    }
    // Stable order so a folder re-import queues the same way twice (read_dir
    // order is filesystem-dependent).
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
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
        .add_filter("Gaussian Splats", &["ply", "gcloud", "sog", "ssog"])
        .add_filter("All Files", &["*"])
        .pick_files()
        .filter(|p| !p.is_empty())
}

/// Open the OS folder picker and expand it to every importable file underneath,
/// preserving the folder tree via [`QueuedAsset::relative_dir`]. Returns `None`
/// if the user cancelled or the folder held nothing importable.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn pick_importable_folder() -> Option<Vec<QueuedAsset>> {
    let dir = rfd::FileDialog::new()
        .set_title("Select folder to import")
        .pick_folder()?;
    let files = expand_importables(&dir);
    if files.is_empty() {
        None
    } else {
        Some(files)
    }
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
        assert_eq!(detect_kind(Path::new("g.lua")), Some(AssetKind::Script));
        for k in [AssetKind::Image, AssetKind::Audio, AssetKind::Scene] {
            assert!(!k.is_model());
        }
    }

    #[test]
    fn gaussian_splats_route_by_extension_and_header() {
        assert_eq!(detect_kind(Path::new("scan.gcloud")), Some(AssetKind::GaussianSplat));
        // An unreadable .ply can't be header-sniffed → mesh/model routing, the
        // pre-splat behaviour.
        assert_eq!(detect_kind(Path::new("mesh.ply")), Some(AssetKind::Model));

        // A .ply whose header carries the 3DGS `f_dc_0` property is a splat.
        let p = std::env::temp_dir().join("renzora_kinds_test_splat.ply");
        std::fs::write(
            &p,
            "ply\nformat binary_little_endian 1.0\nelement vertex 1\n\
             property float x\nproperty float f_dc_0\nend_header\n",
        )
        .unwrap();
        assert_eq!(detect_kind(&p), Some(AssetKind::GaussianSplat));
        std::fs::remove_file(&p).ok();

        // A faceless colored point cloud (CloudCompare-style) also routes to
        // the splat renderer, which synthesizes splats for plain points.
        let p = std::env::temp_dir().join("renzora_kinds_test_points.ply");
        std::fs::write(
            &p,
            "ply\nformat binary_little_endian 1.0\nelement vertex 3\n\
             property float x\nproperty float y\nproperty float z\n\
             property uchar red\nproperty uchar green\nproperty uchar blue\n\
             end_header\n",
        )
        .unwrap();
        assert_eq!(detect_kind(&p), Some(AssetKind::GaussianSplat));
        std::fs::remove_file(&p).ok();

        // A ply WITH faces is a mesh → model/GLB conversion.
        let p = std::env::temp_dir().join("renzora_kinds_test_mesh.ply");
        std::fs::write(
            &p,
            "ply\nformat binary_little_endian 1.0\nelement vertex 3\n\
             property float x\nproperty float y\nproperty float z\n\
             element face 1\nproperty list uchar int vertex_indices\n\
             end_header\n",
        )
        .unwrap();
        assert_eq!(detect_kind(&p), Some(AssetKind::Model));
        std::fs::remove_file(&p).ok();
    }

    #[test]
    fn expand_importables_walks_folders() {
        let root = std::env::temp_dir().join("renzora_kinds_expand_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.png"), b"x").unwrap();
        std::fs::write(root.join("sub").join("b.glb"), b"x").unwrap();
        std::fs::write(root.join("skip.txt"), b"x").unwrap();
        std::fs::write(root.join("sub").join("also.wav"), b"x").unwrap();

        let got = expand_importables(&root);
        assert_eq!(got.len(), 3, "expected png+glb+wav, got {:?}", got);
        assert!(got.iter().all(|q| is_importable(&q.path)));

        let root_name = root.file_name().unwrap().to_str().unwrap();
        let a = got.iter().find(|q| q.path.ends_with("a.png")).unwrap();
        assert_eq!(a.relative_dir, root_name);
        let b = got.iter().find(|q| q.path.ends_with("b.glb")).unwrap();
        assert_eq!(b.relative_dir, format!("{root_name}/sub"));

        // Single file path stays flat (no mirrored subtree).
        let flat = expand_importables(&root.join("a.png"));
        assert_eq!(flat.len(), 1);
        assert!(flat[0].relative_dir.is_empty());
        assert!(expand_importables(&root.join("skip.txt")).is_empty());

        let _ = std::fs::remove_dir_all(&root);
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
