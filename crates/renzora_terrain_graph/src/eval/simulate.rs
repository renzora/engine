//! Simulation nodes: erosion, weathering, rivers, lakes, snow.
//!
//! Erosion runs the terrain crate's own hydraulic and thermal code
//! (`renzora_terrain::erosion`) on the node's heightfield, the same code the
//! Erosion brush and the MCP erode tool run, so a graph and a brush weather
//! ground alike.

use renzora_terrain::erosion::{self, HeightWindow, Hydraulic};

use super::{Inputs, NodeCtx};
use crate::field::{self, smoothstep, Field};

/// Raindrops per erosion call. Rain is split into batches this size so a
/// cancelled build stops within one batch instead of after every drop.
const DROPLET_BATCH: u32 = 150_000;

fn window(src: &Field, ctx: &NodeCtx) -> HeightWindow {
    HeightWindow { x0: 0, z0: 0, w: src.w, h: src.h, spacing: ctx.grid.cell(), data: src.data.clone() }
}

/// A 0..1 mask of where a field rose, scaled to the larger rises so it reads
/// at any relief. The reference is half the highest rise, so the strongest
/// deposits saturate and the faint ones still show.
fn rise_mask(before: &[f32], after: &[f32], w: usize, h: usize) -> Field {
    let rise: Vec<f32> = after.iter().zip(before).map(|(a, b)| (a - b).max(0.0)).collect();
    let peak = rise.iter().copied().fold(0.0f32, f32::max);
    let r = (peak * 0.5).max(1e-4);
    Field { w, h, data: rise.iter().map(|v| (v / r).clamp(0.0, 1.0)).collect() }
}

pub(super) fn erosion(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let src = i.get(0);
    let mask = i.get(1);
    let mut win = window(src, ctx);
    let spacing = win.spacing;
    // Rain is counted per droplet-grid cell (about 3 m, see
    // `erosion::DROPLET_CELL_METRES`), which is what keeps a preview and a
    // build carving alike: both put the same number of drops on each patch
    // of ground the droplets run on.
    let factor = erosion::droplet_factor(spacing, win.w.min(win.h)).max(1);
    let cells = ((win.w - 1) / factor + 1) * ((win.h - 1) / factor + 1);
    let total = ((p.num("rain") * cells as f32) as u64).min(4_000_000) as u32;
    let thermal = p.int("thermal");
    let repose = p.num("repose");
    let base = Hydraulic {
        erosion_radius: p.num("radius"),
        erode_speed: p.num("strength"),
        deposit_speed: p.num("deposition"),
        capacity: p.num("capacity"),
        ..Hydraulic::default()
    };
    let batches = total.div_ceil(DROPLET_BATCH).max(1);
    let mut left = total;
    for b in 0..batches {
        if ctx.cancelled() {
            return Err("cancelled".into());
        }
        let n = left.min(DROPLET_BATCH);
        left -= n;
        let last = b + 1 == batches;
        let settings = Hydraulic { droplets: n, seed: ctx.seed.wrapping_add(b.wrapping_mul(7919)), ..base };
        erosion::erode_window(&mut win, None, settings, if last { thermal } else { 0 }, repose);
    }
    // The mask fades the result back to the input where it is low.
    let (w, h) = (src.w, src.h);
    let out = Field {
        w,
        h,
        data: src.data.iter().zip(&win.data).zip(&mask.data).map(|((a, b), m)| a + (b - a) * m.clamp(0.0, 1.0)).collect(),
    };
    // Clamped again after the blur: its running sums leave a hair below 0.
    let deposits = field::blur(&rise_mask(&src.data, &out.data, w, h), 1, 2).map(|v| v.clamp(0.0, 1.0));
    // White at a fiftieth of the terrain's area: a fixed area would make the
    // channels of a small terrain too faint to see and a large one's all white.
    let area = ctx.grid.extent.x * ctx.grid.extent.y;
    let flow = field::flow_mask(&field::flow_accumulation(&out, &ctx.grid), 200.0, (area * 0.02).clamp(2_000.0, 200_000.0));
    Ok(vec![out, flow, deposits])
}

pub(super) fn thermal(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let mut win = window(src, ctx);
    let passes = ctx.params.int("passes").max(1);
    let repose = ctx.params.num("repose");
    // In slices, for the same reason erosion batches its rain.
    let mut left = passes;
    while left > 0 {
        if ctx.cancelled() {
            return Err("cancelled".into());
        }
        let n = left.min(25);
        left -= n;
        erosion::thermal(&mut win, repose, n, |_, _| 1.0);
    }
    let out = Field { w: src.w, h: src.h, data: win.data };
    let talus = rise_mask(&src.data, &out.data, src.w, src.h).map(|v| v.clamp(0.0, 1.0));
    Ok(vec![out, talus])
}

pub(super) fn rivers(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let p = ctx.params;
    let catchment = p.num("catchment").max(1.0);
    let (depth, width) = (p.num("depth"), p.num("width"));
    let area = field::flow_accumulation(src, &ctx.grid);
    let (lo, hi) = (catchment.ln(), (catchment * 10.0).ln());
    let strength = area.map(|a| smoothstep(lo, hi, a.max(1e-6).ln()));
    // Spread over the river's width: the accumulated flow is a line one
    // sample wide, the bed is not.
    let bed = field::blur_metres(&strength, &ctx.grid, width * 0.5);
    let peak = bed.data.iter().copied().fold(0.0f32, f32::max).max(1e-4);
    // Normalised so a full river reaches its full depth however much the
    // blur spread it.
    let bed = bed.map(|v| (v / peak.min(0.5)).clamp(0.0, 1.0));
    let out = Field::from_fn(&ctx.grid, |x, z| src.at(x, z) - depth * bed.at(x, z));
    let water = bed.map(|v| smoothstep(0.25, 0.6, v));
    Ok(vec![out, water])
}

pub(super) fn lakes(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let min_depth = ctx.params.num("min_depth");
    let filled = field::fill_basins(src);
    let water = Field::from_fn(&ctx.grid, |x, z| {
        let d = filled.at(x, z) - src.at(x, z);
        smoothstep(min_depth, min_depth * 2.0 + 0.05, d)
    });
    let out = if ctx.params.flag("fill") { filled } else { src.clone() };
    Ok(vec![out, water])
}

pub(super) fn snow(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let p = ctx.params;
    let (alt, blend, max_slope, depth) = (p.num("altitude"), p.num("blend").max(0.01), p.num("max_slope"), p.num("depth"));
    let slope = field::slope_degrees(src, &ctx.grid);
    let snow = Field::from_fn(&ctx.grid, |x, z| {
        let high = smoothstep(alt - blend * 0.5, alt + blend * 0.5, src.at(x, z));
        let gentle = 1.0 - smoothstep(max_slope - 5.0, max_slope + 5.0, slope.at(x, z));
        high * gentle
    });
    let out = Field::from_fn(&ctx.grid, |x, z| src.at(x, z) + depth * snow.at(x, z));
    Ok(vec![out, snow])
}
