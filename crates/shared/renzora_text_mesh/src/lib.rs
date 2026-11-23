//! Shared SDF glyph-mesh machinery.
//!
//! Convert Bevy's *coverage* font atlas into a signed distance field, pack a
//! per-text strip, and render it crisp with [`SdfTextMaterial`]. Two callers
//! build glyph geometry: the world-space UI mesh emitter in `renzora_ember`, and
//! the `text3d` **native plugin**.
//!
//! # Why plugins reach it as `renzora::text_mesh`
//!
//! A native plugin is compiled by a bare `rustc` and handed exactly three engine
//! crates: `bevy`, `renzora` and `renzora_ember`. Letting the plugin resolve its
//! own copy of this crate would make [`SdfTextMaterial`] **two different types**:
//! two `TypeId`s, two `MaterialPlugin` registrations, and ember's text and the
//! plugin's text drawn by two unrelated pipelines.
//!
//! So it is a layer of the contract hierarchy: `renzora_sdk` re-exports it as
//! `renzora::text_mesh`, and `renzora_dylib` embeds the one copy every caller
//! shares. [`ensure_sdf_material`]'s idempotence guard relies on that: it is
//! genuinely one material for every caller. It lived inside `renzora` itself
//! for a while, which made every edit here rebuild the whole engine.
//!
//! Layout stays with each caller ([`build_text_mesh`] drives its own for a
//! standalone entity; the UI emitter already has bevy_ui's) — only the
//! coverage→SDF→strip step ([`pack_sdf_strip`]) and the material are shared.
//!
//! # Why the material is behind `render_3d` and the rest isn't
//!
//! Glyph geometry is a `Mesh` and an `Image`; both exist in a 2D-only Bevy. The
//! *material* implements `bevy::pbr::Material`, which does not — a lean export
//! with 3D rendering off strips `bevy_pbr`, and an ungated `impl Material` then
//! fails to compile in a crate that has nothing to do with 3D. Depending on this
//! crate says "this build has `bevy_text`"; it cannot also stand in for "this
//! build has `bevy_pbr`", which is why the material has a feature of its own. `renzora_ember` mirrors the split: its world-space UI (the only
//! caller here that draws in 3D) sits behind an ember `render_3d` of its own.

#[cfg(feature = "render_3d")]
mod material;
mod mesh;
mod pack;
mod sdf;

#[cfg(feature = "render_3d")]
pub use material::{ensure_sdf_material, SdfTextMaterial};
pub use mesh::{build_text_mesh, WORLD_UNITS_PER_PX};
pub use pack::{glyph_key, pack_sdf_strip, PackedGlyph};
pub use sdf::{coverage_to_sdf, SPREAD};
