//! Data nodes: masks measured from a heightfield, for paint layers, foliage
//! and other nodes to read.

use super::primitives::bearing;
use super::{Inputs, NodeCtx};
use crate::field::{self, band, smoothstep, Field};

pub(super) fn slope(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (lo, hi, fall) = (p.num("min"), p.num("max"), p.num("falloff"));
    let s = field::slope_degrees(i.get(0), &ctx.grid);
    Ok(vec![s.map(|d| band(d, lo, hi, fall))])
}

pub(super) fn height_mask(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (lo, hi, fall) = (p.num("min"), p.num("max"), p.num("falloff"));
    Ok(vec![i.get(0).map(|h| band(h, lo, hi, fall))])
}

pub(super) fn curvature(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let p = ctx.params;
    let range = p.num("range").max(1e-3);
    // The blurred ground minus the ground: positive in a hollow (the
    // surroundings are higher), negative on a ridge.
    let soft = field::blur_metres(src, &ctx.grid, p.num("radius"));
    let mode = p.text("mode");
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let l = soft.at(x, z) - src.at(x, z);
        match mode {
            "concave" => (l / range).clamp(0.0, 1.0),
            "both" => (0.5 + l / (2.0 * range)).clamp(0.0, 1.0),
            _ => (-l / range).clamp(0.0, 1.0),
        }
    })])
}

pub(super) fn flow(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let area = field::flow_accumulation(i.get(0), &ctx.grid);
    Ok(vec![field::flow_mask(&area, ctx.params.num("min_area"), ctx.params.num("max_area"))])
}

pub(super) fn wetness(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let area = field::flow_accumulation(src, &ctx.grid);
    let flow = field::flow_mask(&area, ctx.params.num("min_area"), ctx.params.num("max_area"));
    let soft = field::blur_metres(src, &ctx.grid, 15.0);
    let slope = field::slope_degrees(src, &ctx.grid);
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let hollow = ((soft.at(x, z) - src.at(x, z)) / 2.0).clamp(0.0, 1.0);
        let flat = 1.0 - smoothstep(2.0, 25.0, slope.at(x, z));
        (0.5 * flow.at(x, z) + 0.25 * hollow + 0.25 * flat).clamp(0.0, 1.0)
    })])
}

pub(super) fn aspect(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let want = bearing(ctx.params.num("direction"));
    let spread = ctx.params.num("spread").max(1.0);
    let grad = field::gradient(src, &ctx.grid);
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let g = grad[z * src.w + x];
        let steep = g.length();
        if steep < 1e-5 {
            return 0.0;
        }
        // A slope faces downhill: against its gradient.
        let facing = -g / steep;
        let diff = facing.dot(want).clamp(-1.0, 1.0).acos().to_degrees();
        let along = 1.0 - smoothstep(spread * 0.5, spread, diff);
        // Flat ground faces nowhere.
        let tilt = smoothstep(1.0, 6.0, steep.atan().to_degrees());
        along * tilt
    })])
}

pub(super) fn select_range(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let p = ctx.params;
    let (lo, hi) = src.min_max();
    let span = (hi - lo).max(1e-6);
    let (a, b, fall) = (p.num("low"), p.num("high"), p.num("falloff"));
    Ok(vec![src.map(|h| band((h - lo) / span, a, b, fall))])
}
