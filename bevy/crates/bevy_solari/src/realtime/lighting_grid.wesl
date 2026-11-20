// RENZORA PATCH: the lighting grid, which lets Solari trace fewer rays than the
// view has pixels while the image keeps its full resolution.
//
// The G-buffer is rasterized at the view's full size, which is cheap, and only
// the ray-traced lighting, which is not, runs on a coarser grid of
// `lighting_size` cells. Each cell lights one view pixel, placed in its cell by
// the jitter (the cell's centre on every frame; see `LightingGrid` in
// prepare.rs for why it does not move). The denoiser runs on the grid too, and
// its composite upsamples the result to the full view, guided by the full-size
// G-buffer, before multiplying the materials back in. Lighting is demodulated
// (divided by its material factors) everywhere it is filtered, so what is
// upsampled carries no texture: textures stay as sharp as the view.
//
// With the grid the view's size and the jitter zero, every function here is the
// identity, which is what keeps a full-scale view on exactly the upstream path.

/// The view pixel lighting cell `cell` lights this frame.
fn lighting_to_view_pixel(cell: vec2<u32>, jitter: vec2<f32>, lighting_size: vec2<u32>, view_size: vec2<f32>) -> vec2<u32> {
    let cell_size = view_size / vec2<f32>(lighting_size);
    let pixel = floor((vec2<f32>(cell) + jitter) * cell_size);
    return vec2<u32>(clamp(pixel, vec2(0.0), view_size - 1.0));
}

/// Where a view pixel position (texel centres at whole numbers, as
/// `vec2<f32>(pixel_id)` gives) falls on the lighting grid, in the same
/// convention, for a frame lit with `jitter`: exactly `n` at the pixel cell `n`
/// traced, and linear between two cells' pixels.
///
/// Exact rather than `position / cell_size - jitter`, because on a grid whose
/// cell is not a whole number of pixels (two thirds of the view: 1.5 pixels)
/// the traced pixels are unevenly spaced (0, 2, 3, 5, ...), and that formula put
/// them up to half a cell from their own cells. The upsampler then blended the
/// wrong cells and the history reprojected to the wrong ones, and the two-thirds
/// grid scored far worse than the half one. Done per axis, since the grid is
/// separable.
fn view_to_lighting_position(position: vec2<f32>, jitter: vec2<f32>, lighting_size: vec2<u32>, view_size: vec2<f32>) -> vec2<f32> {
    return vec2(
        axis_to_lighting(position.x, jitter.x, lighting_size.x, view_size.x),
        axis_to_lighting(position.y, jitter.y, lighting_size.y, view_size.y),
    );
}

/// The pixel cell `cell` traces along one axis, before clamping to the view.
fn axis_traced_pixel(cell: f32, jitter: f32, cell_size: f32) -> f32 {
    return floor((cell + jitter) * cell_size);
}

fn axis_to_lighting(position: f32, jitter: f32, cells: u32, pixels: f32) -> f32 {
    let cell_size = pixels / f32(cells);
    // The cell whose traced pixel is the last at or before `position`. The
    // estimate is within one of it; step to it.
    var cell = floor(position / cell_size - jitter);
    if axis_traced_pixel(cell, jitter, cell_size) > position {
        cell -= 1.0;
    } else if axis_traced_pixel(cell + 1.0, jitter, cell_size) <= position {
        cell += 1.0;
    }
    let here = axis_traced_pixel(cell, jitter, cell_size);
    let next = axis_traced_pixel(cell + 1.0, jitter, cell_size);
    return cell + (position - here) / max(next - here, 1.0);
}

/// Whether the grid is coarser than the view, so the lighting passes must leave
/// the view's colour to the denoiser's composite.
fn lighting_is_scaled(lighting_size: vec2<u32>, view_size: vec2<f32>) -> bool {
    return any(lighting_size != vec2<u32>(view_size));
}
