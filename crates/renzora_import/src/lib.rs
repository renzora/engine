//! 3D model import and GLB conversion.
//!
//! Supports GLTF/GLB (passthrough), OBJ, STL, PLY, FBX, and USD/USDZ formats.
//! All formats are converted to GLB for use in Bevy.

pub mod anim_extract;
mod convert;
pub mod formats;
pub mod glb_compat;
pub mod optimize;
pub mod settings;

mod fbx;
mod fbx_ufbx;
mod gltf_pass;
mod obj;
mod ply;
mod stl;
// Legacy FBX parser retained for unit detection in `units.rs`; the mesh/anim
// conversion paths now go through `fbx_ufbx` (ufbx crate).
mod fbx_legacy;
// Unused dead-code FBX modules kept temporarily for reference; will be deleted
// once the ufbx path proves stable.
mod abc;
mod blend;
mod bvh;
mod dae;
#[allow(dead_code)]
mod fbx_anim;
#[allow(dead_code)]
mod fbx_ascii;
#[allow(dead_code)]
mod fbx_skin;
pub mod units;
pub mod usd;

pub use anim_extract::extract_animations_from_glb;
pub use bvh::extract_animations_from_bvh;
pub use convert::{
    convert_to_glb, convert_to_glb_with_progress, ExtractedAlphaMode, ExtractedPbrMaterial,
    ImportError, ImportResult, ProgressFn,
};
pub use fbx_ufbx::extract_animations as extract_animations_from_fbx;
pub use formats::{detect_format, supported_extensions, ModelFormat};
pub use optimize::{optimize_glb, MeshOptSettings};
pub use settings::{ImportSettings, UpAxis};
pub use usd::extract_animations_from_usd;
