//! The per-blade instance record, and the component that carries a chunk's worth
//! of them to the renderer.
//!
//! Foliage used to bake every blade's geometry into one giant `Mesh` per chunk:
//! 10 vertices of position + normal + two UVs + colour, ~560 bytes for a blade
//! that is fundamentally described by a position and four scalars. That is what
//! forced a blade budget, made a rebuild expensive enough to need a paced live
//! preview, and put a ceiling on how dense grass could get.
//!
//! Now the blade *shape* lives in the vertex shader — derived from
//! `@builtin(vertex_index)`, so it costs no memory at all — and the CPU produces
//! only this 48-byte record per blade, drawn with one instanced call per chunk.

use bevy::camera::visibility::{self, VisibilityClass};
use bevy::prelude::*;
use bevy::render::sync_world::SyncToRenderWorld;
use bytemuck::{Pod, Zeroable};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Segments along a blade. Four gives a smooth enough curve; the vertex shader
/// reconstructs the strip from this, so it is shared with `grass.wgsl` and the
/// two must agree.
pub const BLADE_SEGMENTS: u32 = 4;
/// Vertices emitted per blade: six per segment, as a non-indexed triangle list.
/// There is no vertex or index buffer — the shader derives everything from the
/// vertex index — so this is purely the draw call's vertex count.
pub const VERTS_PER_BLADE: u32 = BLADE_SEGMENTS * 6;

/// One blade, as the GPU sees it.
///
/// Three `vec4`s, because that is the granularity vertex attributes come in and
/// nothing here is spare: everything derivable from something else has already
/// been left out, and the rotation is stored as its sine and cosine so the
/// vertex shader doesn't run trig a quarter of a million times a frame.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct GrassInstance {
    /// Chunk-local position (xyz) and blade height in metres (w).
    pub position_height: [f32; 4],
    /// Blade width in metres, wind phase, bend factor, colour variation.
    pub width_phase_bend_var: [f32; 4],
    /// Lean along x and z, then the sine and cosine of the blade's Y rotation.
    pub lean_rotation: [f32; 4],
}

/// Identifies one generated blade set.
///
/// The render world holds its own GPU buffer per chunk and must re-upload only
/// when the blades actually change — not every frame, and not merely because the
/// component was extracted again. Comparing this is how it tells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BladeSetId(pub u64);

impl BladeSetId {
    pub fn next() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        BladeSetId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

/// A chunk's grass, ready to draw.
///
/// The blades are behind an `Arc` so extraction into the render world costs a
/// refcount bump rather than copying a multi-megabyte vector every frame.
#[derive(Component, Clone)]
#[require(Transform, Visibility, VisibilityClass, SyncToRenderWorld)]
#[component(on_add = visibility::add_visibility_class::<GrassChunk>)]
pub struct GrassChunk {
    pub id: BladeSetId,
    pub blades: Arc<[GrassInstance]>,
    pub color_base: LinearRgba,
    pub color_tip: LinearRgba,
    pub wind_strength: f32,
}

impl GrassChunk {
    pub fn len(&self) -> usize {
        self.blades.len()
    }

    pub fn is_empty(&self) -> bool {
        self.blades.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The record is uploaded straight into a vertex buffer whose layout is
    /// three `Float32x4`s, so its size has to match exactly or every blade after
    /// the first reads someone else's data.
    #[test]
    fn instance_matches_its_vertex_layout() {
        assert_eq!(std::mem::size_of::<GrassInstance>(), 48);
        assert_eq!(std::mem::align_of::<GrassInstance>(), 4);
    }

    /// The shader rebuilds the blade strip from the vertex index alone, so it
    /// has to be told the same segment count this constant describes.
    #[test]
    fn blade_vertex_count_is_six_per_segment() {
        assert_eq!(VERTS_PER_BLADE, BLADE_SEGMENTS * 6);
    }

    #[test]
    fn blade_set_ids_are_unique() {
        let a = BladeSetId::next();
        let b = BladeSetId::next();
        assert_ne!(a, b);
    }
}
