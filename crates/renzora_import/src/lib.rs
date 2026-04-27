//! 3D model import and GLB conversion.
//!
//! Supports GLTF/GLB (passthrough), OBJ, STL, PLY, FBX, and USD/USDZ formats.
//! All formats are converted to GLB for use in Bevy.

mod convert;
pub mod formats;
pub mod settings;
pub mod anim_extract;
pub mod optimize;
pub mod glb_compat;

mod gltf_pass;
mod obj;
mod stl;
mod ply;
mod fbx;
mod fbx_ufbx;
// Legacy FBX parser retained for unit detection in `units.rs`; the mesh/anim
// conversion paths now go through `fbx_ufbx` (ufbx crate).
mod fbx_legacy;
// Unused dead-code FBX modules kept temporarily for reference; will be deleted
// once the ufbx path proves stable.
#[allow(dead_code)]
mod fbx_ascii;
#[allow(dead_code)]
mod fbx_anim;
#[allow(dead_code)]
mod fbx_skin;
pub mod usd;
mod abc;
mod dae;
mod bvh;
mod blend;
pub mod units;

pub use convert::{convert_to_glb, ImportError, ImportResult};
pub use formats::{detect_format, supported_extensions, ModelFormat};
pub use settings::{ImportSettings, UpAxis};
pub use anim_extract::extract_animations_from_glb;
pub use optimize::{MeshOptSettings, optimize_glb};
pub use fbx_ufbx::extract_animations as extract_animations_from_fbx;
pub use usd::extract_animations_from_usd;
pub use bvh::extract_animations_from_bvh;
