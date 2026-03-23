//! 3D model import and GLB conversion.
//!
//! Supports GLTF/GLB (passthrough), OBJ, STL, PLY, and FBX formats.
//! All formats are converted to GLB for use in Bevy.

mod convert;
pub mod formats;
pub mod settings;
pub mod anim_extract;
pub mod optimize;

mod gltf_pass;
mod obj;
mod stl;
mod ply;
mod fbx;
mod fbx_ascii;

pub use convert::{convert_to_glb, ImportError, ImportResult};
pub use formats::{detect_format, supported_extensions, ModelFormat};
pub use settings::{ImportSettings, UpAxis};
pub use anim_extract::extract_animations_from_glb;
pub use optimize::{MeshOptSettings, optimize_glb};
