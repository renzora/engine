//! Shared SDF glyph-mesh machinery.
//!
//! Convert Bevy's *coverage* font atlas into a signed distance field, pack a
//! per-text strip, and render it crisp with [`SdfTextMaterial`]. Both the 3D-text
//! plugin (`renzora_text3d`, a cdylib) and the world-space UI mesh emitter (in
//! `renzora_ember`, the binary) build glyph geometry — but ember can't link the
//! text3d cdylib, so the reusable pieces live here in a plain rlib both link
//! statically.
//!
//! Why an rlib is safe where a shared dylib wasn't (see the avian saga): an rlib
//! is statically absorbed into whatever links it and introduces no second
//! `bevy_dylib`. Its types still get a *stable* `TypeId` across the binary and the
//! plugin, because the same rlib build is reused for both — so [`SdfTextMaterial`]
//! registers once via [`ensure_sdf_material`]'s guard.
//!
//! Layout stays with each caller ([`build_text_mesh`] drives its own for a
//! standalone entity; the UI emitter already has bevy_ui's) — only the
//! coverage→SDF→strip step ([`pack_sdf_strip`]) and the material are shared.

mod material;
mod mesh;
mod pack;
mod sdf;

pub use material::{ensure_sdf_material, SdfTextMaterial};
pub use mesh::{build_text_mesh, WORLD_UNITS_PER_PX};
pub use pack::{glyph_key, pack_sdf_strip, PackedGlyph};
pub use sdf::{coverage_to_sdf, SPREAD};
