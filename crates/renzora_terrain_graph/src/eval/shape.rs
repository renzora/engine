//! Shape nodes (combine and adjust heightfields) and the two output nodes.

use bevy::math::Vec2;
use renzora_terrain::sculpt::fbm;

use super::{Inputs, NodeCtx};
use crate::field::{self, Field};

pub(super) fn combine(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let (a, b, mask) = (i.get(0), i.get(1), i.get(2));
    let ratio = ctx.params.num("ratio");
    let mode = ctx.params.text("mode");
    let op: fn(f32, f32) -> f32 = match mode {
        "subtract" => |a, b| a - b,
        "max" => f32::max,
        "min" => f32::min,
        "multiply" => |a, b| a * b,
        "blend" => |_, b| b,
        "difference" => |a, b| (a - b).abs(),
        _ => |a, b| a + b,
    };
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let (va, vb) = (a.at(x, z), b.at(x, z));
        let t = (ratio * mask.at(x, z)).clamp(0.0, 1.0);
        va + (op(va, vb) - va) * t
    })])
}

/// Monotone cubic through `points` (sorted by x, clamped to 0..1): smooth,
/// and never overshoots between two points, so a curve that only rises never
/// grows a dip.
pub fn curve_eval(points: &[[f32; 2]], t: f32) -> f32 {
    let mut pts: Vec<[f32; 2]> = points.iter().map(|p| [p[0].clamp(0.0, 1.0), p[1]]).collect();
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    pts.dedup_by(|a, b| (a[0] - b[0]).abs() < 1e-6);
    match pts.len() {
        0 => return t,
        1 => return pts[0][1],
        _ => {}
    }
    if t <= pts[0][0] {
        return pts[0][1];
    }
    if t >= pts[pts.len() - 1][0] {
        return pts[pts.len() - 1][1];
    }
    let n = pts.len();
    let d: Vec<f32> = (0..n - 1).map(|k| (pts[k + 1][1] - pts[k][1]) / (pts[k + 1][0] - pts[k][0])).collect();
    let mut m = vec![0.0f32; n];
    m[0] = d[0];
    m[n - 1] = d[n - 2];
    for k in 1..n - 1 {
        m[k] = if d[k - 1] * d[k] <= 0.0 { 0.0 } else { (d[k - 1] + d[k]) * 0.5 };
    }
    // Fritsch-Carlson: limit the tangents so each piece stays monotone.
    for k in 0..n - 1 {
        if d[k] == 0.0 {
            m[k] = 0.0;
            m[k + 1] = 0.0;
            continue;
        }
        let (a, b) = (m[k] / d[k], m[k + 1] / d[k]);
        let s = a * a + b * b;
        if s > 9.0 {
            let tau = 3.0 / s.sqrt();
            m[k] = tau * a * d[k];
            m[k + 1] = tau * b * d[k];
        }
    }
    let k = pts.windows(2).position(|w| t >= w[0][0] && t <= w[1][0]).unwrap_or(n - 2);
    let h = pts[k + 1][0] - pts[k][0];
    let s = (t - pts[k][0]) / h;
    let (s2, s3) = (s * s, s * s * s);
    let h00 = 2.0 * s3 - 3.0 * s2 + 1.0;
    let h10 = s3 - 2.0 * s2 + s;
    let h01 = -2.0 * s3 + 3.0 * s2;
    let h11 = s3 - s2;
    h00 * pts[k][1] + h10 * h * m[k] + h01 * pts[k + 1][1] + h11 * h * m[k + 1]
}

pub(super) fn curve(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let (lo, hi) = src.min_max();
    let points = ctx.params.points("points").to_vec();
    if hi - lo < 1e-6 {
        return Ok(vec![src.clone()]);
    }
    Ok(vec![src.map(|h| lo + curve_eval(&points, (h - lo) / (hi - lo)) * (hi - lo))])
}

pub(super) fn terrace(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let step = p.num("step").max(0.01);
    let offset = p.num("offset");
    // The share of each step that is riser: the whole step at sharpness 0 (a
    // ramp with a slight S), a tenth at 1 (never a vertical wall, which the
    // mesh would draw as a smear).
    let riser = 1.0 - 0.9 * p.num("sharpness").clamp(0.0, 1.0);
    Ok(vec![i.get(0).map(|h| {
        let t = (h - offset) / step;
        let base = t.floor();
        let f = t - base;
        let s = field::smoothstep(1.0 - riser, 1.0, f);
        (base + s) * step + offset
    })])
}

pub(super) fn warp(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let strength = p.num("strength");
    let scale = p.num("scale").max(0.1);
    let src = i.get(0);
    let by_inputs = i.connected(1) || i.connected(2);
    let (wx, wz) = (i.get(1), i.get(2));
    let seed = ctx.seed;
    let grid = ctx.grid;
    Ok(vec![Field::from_fn(&grid, |x, z| {
        let pos = ctx.pos(x, z);
        let d = if by_inputs {
            Vec2::new(wx.at(x, z) - 0.5, wz.at(x, z) - 0.5) * 2.0
        } else {
            let q = pos / scale;
            Vec2::new(
                fbm(q.x, q.y, 4, 2.0, 0.5, seed) * 2.0 - 1.0,
                fbm(q.x + 5.2, q.y + 1.3, 4, 2.0, 0.5, seed ^ 0x2468) * 2.0 - 1.0,
            )
        };
        src.sample_metres(&grid, pos + d * strength)
    })])
}

pub(super) fn blur(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    Ok(vec![field::blur_metres(i.get(0), &ctx.grid, ctx.params.num("radius"))])
}

pub(super) fn sharpen(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let soft = field::blur_metres(src, &ctx.grid, ctx.params.num("radius"));
    let amount = ctx.params.num("amount");
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let h = src.at(x, z);
        h + amount * (h - soft.at(x, z))
    })])
}

pub(super) fn transform(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let scale = p.num("scale").max(0.01);
    // Clockwise seen from above, as a compass bearing turns.
    let (s, c) = (-p.num("rotate").to_radians()).sin_cos();
    let off = Vec2::new(p.num("offset_x"), p.num("offset_z"));
    let (hs, ho) = (p.num("height_scale"), p.num("height_offset"));
    let src = i.get(0);
    let grid = ctx.grid;
    let centre = grid.centre();
    Ok(vec![Field::from_fn(&grid, |x, z| {
        // Where this sample came from: undo the move, the turn, the scale.
        let q = (ctx.pos(x, z) - centre - off) / scale;
        let r = Vec2::new(q.x * c + q.y * s, -q.x * s + q.y * c);
        src.sample_metres(&grid, r + centre) * hs + ho
    })])
}

pub(super) fn clamp(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let (lo, hi) = (ctx.params.num("min"), ctx.params.num("max"));
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    Ok(vec![i.get(0).map(|h| h.clamp(lo, hi))])
}

pub(super) fn invert(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    if ctx.params.text("mode") == "mask" {
        return Ok(vec![src.map(|h| 1.0 - h)]);
    }
    let (lo, hi) = src.min_max();
    Ok(vec![src.map(|h| lo + hi - h)])
}

pub(super) fn height_range(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let (lo, hi) = src.min_max();
    let (min, max) = (ctx.params.num("min"), ctx.params.num("max"));
    if hi - lo < 1e-6 {
        return Ok(vec![Field::of(&ctx.grid, min)]);
    }
    Ok(vec![src.map(|h| min + (h - lo) / (hi - lo) * (max - min))])
}

pub(super) fn flatten(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let (src, mask) = (i.get(0), i.get(1));
    let (target, strength) = (ctx.params.num("height"), ctx.params.num("strength"));
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let h = src.at(x, z);
        h + (target - h) * (strength * mask.at(x, z)).clamp(0.0, 1.0)
    })])
}

pub(super) fn terrain_output(ctx: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    let src = i.get(0);
    let p = ctx.params;
    if p.text("mode") == "absolute" {
        let off = p.num("offset");
        return Ok(vec![src.map(|h| h + off)]);
    }
    let (floor, peak) = (p.num("floor"), p.num("peak"));
    let (lo, hi) = src.min_max();
    if hi - lo < 1e-6 {
        return Ok(vec![Field::of(&ctx.grid, floor)]);
    }
    Ok(vec![src.map(|h| floor + (h - lo) / (hi - lo) * (peak - floor))])
}

pub(super) fn mask_output(_: &NodeCtx, i: &Inputs) -> Result<Vec<Field>, String> {
    Ok(vec![i.get(0).map(|v| v.clamp(0.0, 1.0))])
}
