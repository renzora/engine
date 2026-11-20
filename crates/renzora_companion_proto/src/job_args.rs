//! The arguments of each job kind. Both ends need one definition: the crate
//! that submits builds one, the companion's handler reads it back.
//!
//! Re-exported as `renzora::job_args`, the path every submitting crate uses.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// `"texture_thumbnail"`: decode `source`, shrink it to fit `size` square
/// (Lanczos3), write it as a PNG to `cache`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TextureThumbnail {
    pub source: PathBuf,
    pub cache: PathBuf,
    pub size: u32,
}

/// `"rust_script"`: compile the script `source` of `project` against the
/// SDK in `sdk_dir`. Answers with the built library's path, which the
/// editor then loads itself (a library is loaded by the process that runs
/// it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RustScriptBuild {
    pub sdk_dir: PathBuf,
    pub project: PathBuf,
    pub source: PathBuf,
}

/// `"rvg_bake"`: bring the virtual geometry beside the GLB `model` up to
/// date. Answers with an [`RvgBakeOutcome`] as JSON.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RvgBake {
    pub model: PathBuf,
}

/// `"texture_bake"`: bake the image file `source` into a mipmapped,
/// block-compressed `.rmip` at `target` (`renzora_rmip::bake::bake_image`
/// with its default settings). `role` is `"color"`, `"normal"` or
/// `"linear"`. Answers with `target`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TextureBake {
    pub source: PathBuf,
    pub target: PathBuf,
    pub role: String,
}

/// `"rvg_migrate"`: turn the `.glb` model `glb` into an `.rvg` model of
/// the same name (`renzora_rvg::bake::convert_to_model`). Answers with an
/// [`RvgMigrateOutcome`] as JSON. The `.glb` is left for the editor to
/// delete, once every reference to it is rewritten.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RvgMigrate {
    pub glb: PathBuf,
}

/// What an `"rvg_migrate"` did; mirrors `renzora_rvg::bake::Converted`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum RvgMigrateOutcome {
    Written { rvg: PathBuf, bytes: u64, before: u64 },
    AlreadyConverted { rvg: PathBuf },
    NameTaken { rvg: PathBuf },
}

/// `"model_thumbnail"`: render `model` (a `.glb`, `.gltf` or `.rvg` in
/// `project`) to its cached thumbnail PNG, with the editor's own capture
/// code. Answers with the PNG's path; the editor then loads it as it
/// loads any cached thumbnail.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ModelThumbnail {
    pub project: PathBuf,
    pub model: PathBuf,
}

/// `"material_thumbnail"`: render `material` (a `.material` in `project`)
/// to its cached thumbnail PNG, with the material editor's own capture
/// code. Answers with the PNG's path.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MaterialThumbnail {
    pub project: PathBuf,
    pub material: PathBuf,
}

/// `"shape_thumbnail"`: render the built-in shape `id` on a background of
/// `clear` (linear RGBA) and write it as a PNG to `path`. Answers with the
/// path. The editor names the file, from a hash of the shape's mesh, so a
/// changed shape gets a new file rather than a stale picture.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShapeThumbnail {
    pub id: String,
    pub path: PathBuf,
    pub clear: [f32; 4],
}

/// `"project_index"`: walk `root` and answer with its files as JSON
/// (`Vec<IndexedFile>`, see `renzora::core::project_index`). Also tells the
/// companion which project is open, so it can watch it and keep its
/// caches (thumbnails) in step with it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ProjectIndexWalk {
    pub root: PathBuf,
}

/// `"profiler.record"`: start or stop recording. With `game`, connect to
/// the game whose name contains it first (the editor itself is "Editor").
/// Answers in words.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ProfilerRecord {
    pub on: bool,
    #[serde(default)]
    pub game: Option<String>,
}

/// `"profiler.status"`, `"profiler.summary"` and `"profiler.spikes"`: a
/// question about the last `seconds` of the companion's capture. Answers
/// with JSON.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct ProfilerQuery {
    #[serde(default)]
    pub seconds: Option<f32>,
    /// How many rows to list.
    #[serde(default)]
    pub limit: Option<usize>,
    /// `"profiler.spikes"` only: frames slower than this. Without it, the
    /// slowest frames in the window.
    #[serde(default)]
    pub over_ms: Option<f32>,
}

/// What an `"rvg_bake"` did.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct RvgBakeOutcome {
    /// A new `.rvg` was written; false when it was already current or
    /// there was nothing to convert.
    pub built: bool,
    pub primitives: usize,
    pub bytes: usize,
    pub seconds: f32,
    /// Why each primitive that stayed ordinary geometry did, one line each.
    pub skipped: Vec<String>,
}
