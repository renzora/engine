//! Materials and shaders.
//!
//! A layer of the contract hierarchy, above `renzora_render_api` (a `.wgsl`
//! post effect is not a material, and `MaterialRef` has to tell them apart).
//! Plugins reach every item at the path it always had (`renzora::MaterialRef`,
//! `renzora::GraphComment`, ...) through `renzora_sdk`; the
//! engine's own crates depend on this crate directly.

pub mod graph; // node-graph vocabulary (material / particle)
pub mod material_nodes; // the material node catalogue, as data
pub mod material_ref; // pointing an entity at a .material file
pub use graph::*;
pub use material_nodes::*;
pub use material_ref::*;

// `wgsl` and `shader_source` are in `renzora_shader_api`, which sits on Bevy
// alone so that crates which only build shaders do not rebuild on a material
// or render edit.
