//! Import settings that control how models are converted to GLB.

/// Axis convention for the up direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpAxis {
    /// Detect from the source file (default).
    Auto,
    /// Y is up (Bevy / GLTF convention).
    YUp,
    /// Z is up (Blender default, many CAD tools).
    ZUp,
}

/// How the imported scene graph is shaped.
///
/// Importers disagree about what a "model" is: a DCC scene is a tree of named
/// objects, a renderer wants as few draw calls as possible, and an editor wants
/// things it can click. There is no answer that is right for all three, so it
/// is a choice rather than a decision baked into the converters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneStructure {
    /// Keep the hierarchy the converter produced.
    ///
    /// For glTF that is the source's own tree. For a transcoded format it is
    /// currently a single merged node — see [`Self::Combined`], which is what
    /// those converters do today regardless.
    Preserve,
    /// One node per mesh, all at the scene root, with group transforms folded
    /// into each leaf. Every chunk becomes separately selectable and
    /// independently culled, at the cost of the source's grouping.
    FlatPerMesh,
    /// Everything merged into a single mesh, one primitive per material.
    /// Fewest draw calls; nothing can be picked or culled individually.
    Combined,
}

/// Settings that control model import and GLB conversion.
///
/// `PartialEq` is load-bearing rather than incidental: the import window
/// reconverts on its own when any of this changes, and it decides that by
/// comparing the settings it last converted with against the ones now on screen.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportSettings {
    /// Uniform scale factor applied to all geometry.
    pub scale: f32,
    /// Up-axis convention.
    pub up_axis: UpAxis,
    /// How the scene graph is shaped once conversion is done.
    pub structure: SceneStructure,
    /// Flip the V texture coordinate (1.0 - v).
    pub flip_uvs: bool,
    /// Generate flat normals if the source has none.
    pub generate_normals: bool,
    /// Which sibling texture set to bind to a geometry-only model, by
    /// [`crate::sibling_textures::TextureSet::stem`]. `None` leaves the
    /// placeholder material untextured.
    ///
    /// Only formats that carry no materials of their own consult this — a model
    /// that names its own textures always wins over a folder full of guesses.
    /// It is a name rather than an index because the choice survives a reimport,
    /// and a folder gaining a file would otherwise shift every index under it.
    pub texture_set: Option<String>,
    /// Optimize vertex cache locality (reorders triangles for GPU).
    pub optimize_vertex_cache: bool,
    /// Optimize for reduced overdraw.
    pub optimize_overdraw: bool,
    /// Optimize vertex fetch (reorders vertices for cache).
    pub optimize_vertex_fetch: bool,

    // ─── Per-asset-type extraction toggles ──────────────────────────────
    // Let the user opt out of parts of a model they don't need. Mesh is
    // required; everything else is optional.
    /// Extract the skeleton + per-vertex skin weights when present.
    pub extract_skeleton: bool,
    /// Split animations out into sibling `.anim` files.
    pub extract_animations: bool,
    /// Dump embedded images to `<stem>/textures/`.
    pub extract_textures: bool,
    /// Emit `.material` files per PBR material (also controls GLTF material
    /// records in the GLB — off means the mesh references no material).
    pub extract_materials: bool,

    // ─── Texture baking (.rmip) ─────────────────────────────────────────
    /// GPU-block-compress baked textures (BC7/BC5/BC1/BC3). Cuts VRAM 4–8×
    /// and removes the runtime decode. Off stores uncompressed RGBA8 mips.
    pub texture_compression: bool,
    /// Prefer BC7 (best quality, 1 byte/px) over BC1/BC3 for color/data maps.
    /// Normal maps always use BC5 regardless. Off trades quality for size and
    /// faster import (mirrors Godot's non-"high quality" VRAM path).
    pub texture_high_quality: bool,
    /// Clamp each texture's longest side to this many texels at import
    /// (`0` = keep native resolution). 4K source maps are rarely needed at
    /// full res in-scene; downsampling is the single biggest VRAM win.
    pub texture_max_size: u32,
}

impl Default for ImportSettings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            up_axis: UpAxis::Auto,
            // Preserve is the honest default: it does not restructure anything
            // the converter produced. A transcoded format still arrives merged,
            // which is what the import window's "hierarchy flattened" finding
            // reports — picking `FlatPerMesh` is the way to undo it.
            structure: SceneStructure::Preserve,
            flip_uvs: false,
            texture_set: None,
            generate_normals: true,
            optimize_vertex_cache: true,
            optimize_overdraw: true,
            // Safe again: the optimizer now skips the vertex-fetch attribute
            // remap for primitives that share a vertex buffer (which was
            // scrambling shared geometry on assets like Sponza), and only
            // applies it to primitives that exclusively own their attributes.
            optimize_vertex_fetch: true,
            extract_skeleton: true,
            extract_animations: true,
            extract_textures: true,
            extract_materials: true,
            texture_compression: true,
            texture_high_quality: true,
            texture_max_size: 2048,
        }
    }
}

impl ImportSettings {
    /// Build the `.rmip` baker parameters for a texture of the given role
    /// from these import settings.
    pub fn bake_params(&self, role: renzora_rmip::bake::TextureRole) -> renzora_rmip::bake::BakeParams {
        renzora_rmip::bake::BakeParams {
            role,
            compress: self.texture_compression,
            high_quality: self.texture_high_quality,
            max_size: self.texture_max_size,
        }
    }
}
