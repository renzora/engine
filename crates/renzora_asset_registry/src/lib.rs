//! Renzora Asset Registry — metadata-only index of every asset in the
//! current project's `assets/` tree.
//!
//! Built once when entering [`SplashState::Loading`] by walking the project
//! directory and recording each file's path, kind, and size. The registry
//! is consulted by the asset browser, drag-and-drop preview, and (in a
//! future PR) the lazy warm cache that pre-loads heavy assets the moment
//! the user starts a drag.
//!
//! What this crate is **not**: it does not load asset bytes, decode
//! textures, or instantiate scenes. That stays with Bevy's `AssetServer`.
//! Following Unity/Unreal: an asset database knows *about* every asset
//! at edit time, but only loads what the running scene actually needs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bevy::prelude::*;
use renzora::core::CurrentProject;
use renzora_splash::SplashState;

/// Coarse classification of an asset by file extension. Used by the
/// asset browser's icon picker, the drag-and-drop preview's loader
/// dispatch, and the warm-cache prioritization logic. Variants are kept
/// deliberately broad — "Texture" covers every image format Bevy can
/// decode, not one variant per extension.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AssetKind {
    /// 3D model: `glb`, `gltf`, `obj`, `fbx`, `usd*`, `dae`, `abc`,
    /// `blend`. Drag-drop spawns these via `AssetServer::load::<Gltf>`.
    Model,
    /// Image format Bevy can decode at runtime. Includes HDR/EXR.
    Texture,
    /// Renzora `.material` file consumed by `renzora_shader`.
    Material,
    /// Renzora scene file (the format `scene_io::save_scene` writes).
    Scene,
    /// Audio sample.
    Audio,
    /// Video clip.
    Video,
    /// Source-level script (Rhai/Lua/JS/TS).
    Script,
    /// Hand-authored shader source (WGSL/GLSL/HLSL).
    Shader,
    /// Anything else — config, docs, unrecognised extensions.
    Other,
}

impl AssetKind {
    /// Classify a path by its lower-cased extension. Matches the same
    /// extension table the asset browser uses for icon picking, so a
    /// file that shows up as "Image" in the browser also shows up as
    /// `Texture` here.
    pub fn from_path(path: &Path) -> Self {
        let Some(ext) = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_ascii_lowercase())
        else {
            return AssetKind::Other;
        };
        match ext.as_str() {
            "glb" | "gltf" | "obj" | "fbx" | "usd" | "usda" | "usdc" | "usdz" | "abc" | "dae"
            | "blend" => AssetKind::Model,
            "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr" | "exr" => AssetKind::Texture,
            "material" | "material_bp" => AssetKind::Material,
            "scene" => AssetKind::Scene,
            "wav" | "ogg" | "mp3" | "flac" | "opus" => AssetKind::Audio,
            "mp4" | "avi" | "mov" | "webm" => AssetKind::Video,
            "lua" | "js" | "ts" => AssetKind::Script,
            "wgsl" | "glsl" | "vert" | "frag" | "hlsl" => AssetKind::Shader,
            _ => AssetKind::Other,
        }
    }
}

/// One row in the registry. The `path` is asset-relative — i.e. what
/// you'd pass to `AssetServer::load`.
#[derive(Clone, Debug)]
pub struct AssetEntry {
    pub path: String,
    pub kind: AssetKind,
    pub size_bytes: u64,
    /// Last-modified time as Unix seconds. `None` if the underlying
    /// filesystem doesn't expose mtime (rare). Used as a cache-bust key
    /// for derived artefacts (thumbnails, decoded textures, etc.) — when
    /// the source file changes, every cache entry keyed on the old
    /// `mtime_secs` becomes naturally stale without an explicit
    /// invalidation pass.
    pub mtime_secs: Option<u64>,
}

/// Metadata index of every file under the current project's root.
/// Cleared and rebuilt whenever the user opens (or re-opens) a project.
#[derive(Resource, Default)]
pub struct AssetRegistry {
    entries: HashMap<String, AssetEntry>,
}

impl AssetRegistry {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, asset_path: &str) -> Option<&AssetEntry> {
        self.entries.get(asset_path)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &AssetEntry)> {
        self.entries.iter()
    }

    /// Iterate every entry whose `kind` matches.
    pub fn iter_kind(&self, kind: AssetKind) -> impl Iterator<Item = (&String, &AssetEntry)> {
        self.entries.iter().filter(move |(_, e)| e.kind == kind)
    }
}

#[derive(Default)]
pub struct AssetRegistryPlugin;

impl Plugin for AssetRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetRegistry>().add_systems(
            OnEnter(SplashState::Loading),
            build_asset_registry_on_loading,
        );
    }
}

/// Build the registry by walking the project root. Runs as a one-shot
/// system on `OnEnter(SplashState::Loading)` — the splash bar holds the
/// editor open until the loading task is done.
fn build_asset_registry_on_loading(
    project: Option<Res<CurrentProject>>,
    mut registry: ResMut<AssetRegistry>,
) {
    registry.entries.clear();

    let Some(project) = project else {
        warn!("[asset_registry] no CurrentProject — skipping index build");
        return;
    };

    let root = project.path.clone();
    // `bevy::platform::time::Instant`, never `std`'s — std's panics on wasm.
    let started = bevy::platform::time::Instant::now();
    let mut entries = HashMap::new();
    walk_into(&root, &root, &mut entries);
    registry.entries = entries;

    info!(
        "[asset_registry] indexed {} assets under {} in {:?}",
        registry.entries.len(),
        root.display(),
        started.elapsed()
    );
}

/// Recursive worker for [`build_asset_registry_on_loading`]. Skips
/// hidden directories (anything starting with `.`) and the conventional
/// build/cache directories so the index doesn't balloon with garbage.
fn walk_into(root: &Path, dir: &PathBuf, out: &mut HashMap<String, AssetEntry>) {
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        // Skip dotfiles/dotdirs and well-known noise directories. These
        // would otherwise drag in node_modules-sized trees on projects
        // that happen to have build outputs sitting in the root.
        let name_lc = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if name_lc.starts_with('.') || name_lc == "target" || name_lc == "node_modules" {
            continue;
        }

        if file_type.is_dir() {
            walk_into(root, &path, out);
            continue;
        }

        if !file_type.is_file() {
            continue;
        }

        // Asset path = path relative to project root, with `/` separators
        // — what AssetServer::load expects.
        let rel = match path.strip_prefix(root) {
            Ok(r) => r.to_string_lossy().replace('\\', "/"),
            Err(_) => continue,
        };

        let metadata = entry.metadata().ok();
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let mtime_secs = metadata
            .as_ref()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs());
        let kind = AssetKind::from_path(&path);

        out.insert(
            rel.clone(),
            AssetEntry {
                path: rel,
                kind,
                size_bytes,
                mtime_secs,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::ecs::system::RunSystemOnce;

    fn kind(name: &str) -> AssetKind {
        AssetKind::from_path(Path::new(name))
    }

    #[test]
    fn every_extension_group_classifies() {
        for name in ["a.glb", "a.gltf", "a.obj", "a.fbx", "a.usd", "a.usda", "a.usdc", "a.usdz",
                     "a.abc", "a.dae", "a.blend"] {
            assert_eq!(kind(name), AssetKind::Model, "{name}");
        }
        for name in ["a.png", "a.jpg", "a.jpeg", "a.bmp", "a.tga", "a.webp", "a.hdr", "a.exr"] {
            assert_eq!(kind(name), AssetKind::Texture, "{name}");
        }
        for name in ["a.material", "a.material_bp"] {
            assert_eq!(kind(name), AssetKind::Material, "{name}");
        }
        assert_eq!(kind("a.scene"), AssetKind::Scene);
        for name in ["a.wav", "a.ogg", "a.mp3", "a.flac", "a.opus"] {
            assert_eq!(kind(name), AssetKind::Audio, "{name}");
        }
        for name in ["a.mp4", "a.avi", "a.mov", "a.webm"] {
            assert_eq!(kind(name), AssetKind::Video, "{name}");
        }
        for name in ["a.lua", "a.js", "a.ts"] {
            assert_eq!(kind(name), AssetKind::Script, "{name}");
        }
        for name in ["a.wgsl", "a.glsl", "a.vert", "a.frag", "a.hlsl"] {
            assert_eq!(kind(name), AssetKind::Shader, "{name}");
        }
    }

    /// The doc comment promises the table matches the browser's icon picker,
    /// which lower-cases. A `.PNG` from a Windows tool is the common real case.
    #[test]
    fn classification_ignores_extension_case() {
        assert_eq!(kind("Hero.PNG"), AssetKind::Texture);
        assert_eq!(kind("Level.GLTF"), AssetKind::Model);
    }

    #[test]
    fn unknown_and_missing_extensions_are_other() {
        assert_eq!(kind("notes.txt"), AssetKind::Other);
        assert_eq!(kind("Makefile"), AssetKind::Other);
        // A dotfile's "extension" is the whole name to `Path`, so this also
        // guards the `.gitignore`-style case reaching the match arm at all.
        assert_eq!(kind(".gitignore"), AssetKind::Other);
    }

    fn write(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn walk(root: &Path) -> HashMap<String, AssetEntry> {
        let mut out = HashMap::new();
        walk_into(root, &root.to_path_buf(), &mut out);
        out
    }

    #[test]
    fn walk_indexes_nested_files_under_forward_slash_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "models/hero.glb", b"12345");
        write(root, "textures/ui/button.png", b"ab");

        let out = walk(root);
        assert_eq!(out.len(), 2);

        // Separator normalization is the load-bearing part: the key is what
        // gets handed to `AssetServer::load`, which only accepts `/` — on
        // Windows the raw relative path would come back with backslashes.
        let hero = out.get("models/hero.glb").expect("nested file indexed");
        assert_eq!(hero.kind, AssetKind::Model);
        assert_eq!(hero.size_bytes, 5);
        assert_eq!(hero.path, "models/hero.glb");
        assert!(out.contains_key("textures/ui/button.png"));
    }

    #[test]
    fn walk_skips_dot_and_build_directories() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        write(root, "keep.png", b"x");
        write(root, ".git/objects/blob", b"x");
        write(root, "target/dist/renzora.exe", b"x");
        write(root, "node_modules/pkg/index.js", b"x");
        write(root, ".hidden_asset.png", b"x");

        let out = walk(root);
        assert_eq!(
            out.keys().collect::<Vec<_>>(),
            vec!["keep.png"],
            "only the non-hidden, non-build file should be indexed"
        );
    }

    #[test]
    fn walk_records_an_mtime_it_can_read() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "a.png", b"x");
        let out = walk(tmp.path());
        // Every filesystem this runs on exposes mtime; `None` here would mean
        // the cache-bust key is silently absent and derived thumbnails would
        // never invalidate.
        assert!(out["a.png"].mtime_secs.is_some());
    }

    #[test]
    fn walk_of_a_missing_directory_is_empty_rather_than_a_panic() {
        let out = walk(Path::new("no/such/directory/anywhere"));
        assert!(out.is_empty());
    }

    #[test]
    fn registry_accessors_report_the_indexed_set() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "a.png", b"x");
        write(tmp.path(), "b.png", b"x");
        write(tmp.path(), "c.glb", b"x");

        let registry = AssetRegistry {
            entries: walk(tmp.path()),
        };
        assert_eq!(registry.len(), 3);
        assert!(!registry.is_empty());
        assert!(registry.get("a.png").is_some());
        assert!(registry.get("missing.png").is_none());
        assert_eq!(registry.iter().count(), 3);
        assert_eq!(registry.iter_kind(AssetKind::Texture).count(), 2);
        assert_eq!(registry.iter_kind(AssetKind::Model).count(), 1);
        assert_eq!(registry.iter_kind(AssetKind::Audio).count(), 0);
    }

    #[test]
    fn empty_registry_is_empty() {
        let registry = AssetRegistry::default();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn the_build_system_indexes_the_current_project() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "scenes/main.scene", b"{}");

        let mut world = World::new();
        world.init_resource::<AssetRegistry>();
        world.insert_resource(CurrentProject {
            path: tmp.path().to_path_buf(),
            config: Default::default(),
        });

        world.run_system_once(build_asset_registry_on_loading).unwrap();

        let registry = world.resource::<AssetRegistry>();
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("scenes/main.scene").unwrap().kind, AssetKind::Scene);
    }

    /// Opening a second project must not leave the first one's entries behind —
    /// the browser would show files that are no longer on disk.
    #[test]
    fn rebuilding_clears_the_previous_project() {
        let first = tempfile::tempdir().unwrap();
        write(first.path(), "old.png", b"x");
        let second = tempfile::tempdir().unwrap();
        write(second.path(), "new.png", b"x");

        let mut world = World::new();
        world.init_resource::<AssetRegistry>();
        world.insert_resource(CurrentProject {
            path: first.path().to_path_buf(),
            config: Default::default(),
        });
        world.run_system_once(build_asset_registry_on_loading).unwrap();
        assert!(world.resource::<AssetRegistry>().get("old.png").is_some());

        world.insert_resource(CurrentProject {
            path: second.path().to_path_buf(),
            config: Default::default(),
        });
        world.run_system_once(build_asset_registry_on_loading).unwrap();

        let registry = world.resource::<AssetRegistry>();
        assert!(registry.get("old.png").is_none(), "stale entry survived");
        assert!(registry.get("new.png").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn no_current_project_clears_rather_than_panics() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "a.png", b"x");

        let mut world = World::new();
        world.init_resource::<AssetRegistry>();
        world.insert_resource(CurrentProject {
            path: tmp.path().to_path_buf(),
            config: Default::default(),
        });
        world.run_system_once(build_asset_registry_on_loading).unwrap();
        assert_eq!(world.resource::<AssetRegistry>().len(), 1);

        world.remove_resource::<CurrentProject>();
        world.run_system_once(build_asset_registry_on_loading).unwrap();
        assert!(world.resource::<AssetRegistry>().is_empty());
    }
}
