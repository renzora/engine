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
            "glb" | "gltf" | "obj" | "fbx" | "usd" | "usda" | "usdc"
            | "usdz" | "abc" | "dae" | "blend" => AssetKind::Model,
            "png" | "jpg" | "jpeg" | "bmp" | "tga" | "webp" | "hdr"
            | "exr" => AssetKind::Texture,
            "material" | "material_bp" => AssetKind::Material,
            "scene" => AssetKind::Scene,
            "wav" | "ogg" | "mp3" | "flac" | "opus" => AssetKind::Audio,
            "mp4" | "avi" | "mov" | "webm" => AssetKind::Video,
            "rhai" | "lua" | "js" | "ts" => AssetKind::Script,
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
    pub fn iter_kind(
        &self,
        kind: AssetKind,
    ) -> impl Iterator<Item = (&String, &AssetEntry)> {
        self.entries
            .iter()
            .filter(move |(_, e)| e.kind == kind)
    }
}

#[derive(Default)]
pub struct AssetRegistryPlugin;

impl Plugin for AssetRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<AssetRegistry>()
            .add_systems(
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
    let started = std::time::Instant::now();
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
        let name_lc = entry
            .file_name()
            .to_string_lossy()
            .to_ascii_lowercase();
        if name_lc.starts_with('.')
            || name_lc == "target"
            || name_lc == "node_modules"
        {
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

        let size_bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let kind = AssetKind::from_path(&path);

        out.insert(
            rel.clone(),
            AssetEntry {
                path: rel,
                kind,
                size_bytes,
            },
        );
    }
}
