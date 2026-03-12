//! 3D model import and GLB conversion.
//!
//! Supports GLTF/GLB (passthrough), OBJ, STL, PLY, and FBX formats.
//! All formats are converted to GLB for use in Bevy.

mod convert;
pub mod formats;
pub mod settings;

mod gltf_pass;
mod obj;
mod stl;
mod ply;
mod fbx;

pub use convert::{convert_to_glb, ImportError, ImportResult};
pub use formats::{detect_format, supported_extensions, ModelFormat};
pub use settings::{ImportSettings, UpAxis};
