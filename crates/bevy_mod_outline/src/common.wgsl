#define_import_path bevy_mod_outline::common

struct OutlineViewUniform {
    @align(16)
    clip_from_world: mat4x4<f32>,
    world_from_view_a: mat2x4<f32>,
    world_from_view_b: f32,
    aspect: f32,
    scale_clip_from_logical: vec2<f32>,
    scale_physical_from_logical: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
#ifdef FLAT_DEPTH
    @location(0) @interpolate(flat) flat_depth: f32,
#endif
#ifdef VOLUME
    @location(1) @interpolate(flat) volume_colour: vec4<f32>,
#endif
#ifdef ALPHA_MASK_TEXTURE
    @location(2) @interpolate(flat) alpha_mask_threshold: f32,
    @location(3) uv: vec2<f32>,
#endif
};