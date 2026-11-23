//! Shader text tools.
//!
//! A layer of the contract hierarchy that depends on Bevy alone. Plugins reach
//! both modules at the paths they always had (`renzora::wgsl`,
//! `renzora::shader_source`) through `renzora_sdk`; the engine's own crates
//! depend on this crate directly.
//!
//! Kept apart from `renzora_material_api` on purpose: the UI toolkit builds its
//! theme shaders through `shader_source`, and while that lived beside the
//! material types, every material or render-setting edit rebuilt the toolkit
//! and the editor above it.

// The one WGSL/naga seam (parse/validate) for self-contained shaders.
pub mod wgsl;

// Shader text in any dialect (plain WGSL, WESL, or the pre-0.20 naga_oil one)
// to a Bevy `Shader`, translating the old dialect so it keeps working.
pub mod shader_source;
