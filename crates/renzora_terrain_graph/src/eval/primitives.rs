//! Primitives: the nodes that make a heightfield from nothing.
//!
//! Every one samples at terrain-local metres, never at grid cells, so the same
//! node gives the same ground at any resolution. Offsets are from the
//! terrain's centre; bearings are compass degrees, 0 north (+Z) and 90 east
//! (+X).

use bevy::math::Vec2;
use renzora_terrain::data::NoiseMode;
use renzora_terrain::sculpt::{eval_noise, fbm, ridge_noise};

use super::{Inputs, NodeCtx};
use crate::field::{smoothstep, Field};

/// The Noise node's stored name for a sculpt / Generate noise mode.
pub fn noise_mode_key(mode: NoiseMode) -> &'static str {
    match mode {
        NoiseMode::Eroded => "eroded",
        NoiseMode::Fbm => "fbm",
        NoiseMode::Ridge => "ridge",
        NoiseMode::Billow => "billow",
        NoiseMode::Warped => "warped",
        NoiseMode::Hybrid => "hybrid",
    }
}

/// The inverse of [`noise_mode_key`]; an unknown name reads as Eroded.
pub fn parse_noise_mode(key: &str) -> NoiseMode {
    match key {
        "fbm" => NoiseMode::Fbm,
        "ridge" | "ridged" => NoiseMode::Ridge,
        "billow" => NoiseMode::Billow,
        "warped" => NoiseMode::Warped,
        "hybrid" => NoiseMode::Hybrid,
        _ => NoiseMode::Eroded,
    }
}

/// Unit vector of a compass bearing in the terrain's x/z plane.
pub(super) fn bearing(degrees: f32) -> Vec2 {
    let r = degrees.to_radians();
    Vec2::new(r.sin(), r.cos())
}

/// A point relative to the node's centre (the terrain's centre plus the
/// Offset X / Offset Z parameters).
fn from_centre(ctx: &NodeCtx, p: Vec2) -> Vec2 {
    let off = Vec2::new(ctx.params.num("offset_x"), ctx.params.num("offset_z"));
    p - ctx.grid.centre() - off
}

pub(super) fn noise(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let mode = parse_noise_mode(p.text("mode"));
    let scale = p.num("scale").max(0.1);
    let (height, octaves, lac, pers, warp, exp) =
        (p.num("height"), p.int("octaves").clamp(1, 10), p.num("lacunarity"), p.num("persistence"), p.num("warp"), p.num("exponent"));
    let off = Vec2::new(p.num("offset_x"), p.num("offset_z"));
    let seed = ctx.seed;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let q = (ctx.pos(x, z) - off) / scale;
        let n = eval_noise(q.x, q.y, mode, octaves, lac, pers, seed, warp).clamp(0.0, 1.0);
        n.powf(exp.max(0.05)) * height
    })])
}

pub(super) fn mountain_range(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let scale = p.num("scale").max(1.0);
    let height = p.num("height");
    let along = bearing(p.num("direction"));
    let across = Vec2::new(along.y, -along.x);
    let elong = p.num("elongation").max(1.0);
    let warp = p.num("warp");
    let octaves = p.int("octaves").clamp(1, 10);
    let sharp = p.num("sharpness").max(0.1);
    let seed = ctx.seed;
    let centre = ctx.grid.centre();
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let q = ctx.pos(x, z) - centre;
        // Stretched along the bearing, so the ridges run that way.
        let (u, v) = (q.dot(along) / (scale * elong), q.dot(across) / scale);
        // Domain warp: the ridgelines wander and branch rather than running
        // as straight lattice-aligned facets.
        let wu = fbm(u * 0.7 + 11.3, v * 0.7 - 3.1, 3, 2.0, 0.5, seed ^ 0x5a17) * 2.0 - 1.0;
        let wv = fbm(u * 0.7 - 7.9, v * 0.7 + 5.3, 3, 2.0, 0.5, seed ^ 0x3c91) * 2.0 - 1.0;
        let r = ridge_noise(u + wu * warp * 0.5, v + wv * warp * 0.5, octaves, 2.0, 0.5, seed);
        // A broad envelope so ranges rise and fall along their length, with
        // passes between the high massifs.
        let env = fbm(u * 0.35 + 7.1, v * 0.35 + 2.9, 3, 2.0, 0.5, seed ^ 0x0abc);
        let h = r.clamp(0.0, 1.0).powf(sharp * 1.6) * (0.35 + 0.65 * smoothstep(0.25, 0.75, env));
        h * height
    })])
}

pub(super) fn plateau(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (radius, height, edge) = (p.num("radius"), p.num("height"), p.num("edge").max(0.01));
    let (rag, ns) = (p.num("edge_noise"), p.num("noise_scale").max(0.1));
    let seed = ctx.seed;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let pos = ctx.pos(x, z);
        let d = from_centre(ctx, pos).length();
        let n = fbm(pos.x / ns, pos.y / ns, 4, 2.0, 0.5, seed) * 2.0 - 1.0;
        let d = d + n * rag * radius * 0.5;
        height * (1.0 - smoothstep(radius, radius + edge, d))
    })])
}

pub(super) fn crater(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (radius, depth, rim_h, rim_w) = (p.num("radius").max(0.01), p.num("depth"), p.num("rim_height"), p.num("rim_width").max(0.01));
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let d = from_centre(ctx, ctx.pos(x, z)).length();
        let t = d / radius;
        let bowl = if t < 1.0 { -depth * (1.0 - t * t) } else { 0.0 };
        let r = (d - radius) / rim_w;
        bowl + rim_h * (-r * r).exp()
    })])
}

pub(super) fn volcano(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (radius, height) = (p.num("radius").max(0.01), p.num("height"));
    let (cr, cd) = (p.num("crater_radius"), p.num("crater_depth"));
    let (curve, rough) = (p.num("slope").max(0.1), p.num("roughness"));
    let seed = ctx.seed;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let q = from_centre(ctx, ctx.pos(x, z));
        let d = q.length();
        // Lobes and gullies: the distance itself is pushed around, so the
        // cone's outline and its contours wobble together.
        let n = fbm(q.x / (radius * 0.25), q.y / (radius * 0.25), 4, 2.0, 0.5, seed) * 2.0 - 1.0;
        let d2 = d * (1.0 + rough * 0.3 * n);
        let t = (1.0 - d2 / radius).max(0.0);
        let cone = height * t.powf(curve);
        let bowl = if cr > 0.0 { cd * (1.0 - smoothstep(cr * 0.2, cr, d)) } else { 0.0 };
        let fine = fbm(q.x / (radius * 0.08), q.y / (radius * 0.08), 3, 2.0, 0.5, seed ^ 0x77) - 0.5;
        cone * (1.0 + rough * 0.2 * fine) - bowl
    })])
}

pub(super) fn dunes(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let wl = p.num("wavelength").max(0.1);
    let height = p.num("height");
    let wind = bearing(p.num("direction"));
    let asym = p.num("asymmetry").clamp(0.5, 0.95);
    let meander = p.num("meander");
    let seed = ctx.seed;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let pos = ctx.pos(x, z);
        let m = fbm(pos.x / (wl * 3.0), pos.y / (wl * 3.0), 3, 2.0, 0.5, seed) * 2.0 - 1.0;
        let u = pos.dot(wind) / wl + meander * m;
        let f = u - u.floor();
        // Rising gently into the wind, then a short steep lee face downwind.
        let s = if f < asym { f / asym } else { (1.0 - f) / (1.0 - asym) };
        let s = s * s * (3.0 - 2.0 * s);
        let amp = 0.55 + 0.45 * fbm(pos.x / (wl * 5.0) + 3.7, pos.y / (wl * 5.0) - 1.1, 3, 2.0, 0.5, seed ^ 0x99);
        height * s * amp
    })])
}

fn hash(x: i32, z: i32, seed: u32, k: u32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (z as u32).wrapping_mul(0xd816_3841) ^ seed.wrapping_mul(0xcb1a_b31f) ^ k;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2c1b_3c6d);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297a_2d39);
    h ^= h >> 15;
    (h >> 8) as f32 / (1u32 << 24) as f32
}

pub(super) fn cells(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let scale = p.num("scale").max(0.1);
    let height = p.num("height");
    let jitter = p.num("jitter").clamp(0.0, 1.0);
    let mode = p.text("mode").to_string();
    let seed = ctx.seed;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let c = ctx.pos(x, z) / scale;
        let (ix, iz) = (c.x.floor() as i32, c.y.floor() as i32);
        let (mut d1, mut d2, mut id) = (f32::MAX, f32::MAX, (0, 0));
        for dz in -1..=1 {
            for dx in -1..=1 {
                let (cx, cz) = (ix + dx, iz + dz);
                let feature = Vec2::new(
                    cx as f32 + 0.5 + jitter * (hash(cx, cz, seed, 1) - 0.5),
                    cz as f32 + 0.5 + jitter * (hash(cx, cz, seed, 2) - 0.5),
                );
                let d = feature.distance(c);
                if d < d1 {
                    d2 = d1;
                    d1 = d;
                    id = (cx, cz);
                } else if d < d2 {
                    d2 = d;
                }
            }
        }
        let t = match mode.as_str() {
            "ridges" => 1.0 - ((d2 - d1) * 2.0).clamp(0.0, 1.0),
            "cells" => hash(id.0, id.1, seed, 3),
            _ => (d1 / 0.85).clamp(0.0, 1.0),
        };
        t * height
    })])
}

pub(super) fn gradient(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let (start, end) = (p.num("start"), p.num("end"));
    let dir = bearing(p.num("direction"));
    let radial = p.text("mode") == "radial";
    let centre = ctx.grid.centre();
    let half = ctx.grid.extent * 0.5;
    // Half the extent's width measured along the bearing, so the ramp runs
    // exactly edge to edge whatever the direction.
    let span = (half.x * dir.x.abs() + half.y * dir.y.abs()).max(1e-3);
    let corner = half.length().max(1e-3);
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let q = ctx.pos(x, z) - centre;
        let t = if radial { q.length() / corner } else { (q.dot(dir) / span + 1.0) * 0.5 };
        start + (end - start) * t.clamp(0.0, 1.0)
    })])
}

pub(super) fn constant(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    Ok(vec![Field::of(&ctx.grid, ctx.params.num("height"))])
}

/// Decoded heightmaps by path, kept while the file is unchanged: decoding a
/// 4096 square PNG on every preview of every edit would dominate the graph.
fn image(path: &std::path::Path) -> Result<std::sync::Arc<renzora_terrain::heightmap_import::HeightmapImage>, String> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    type Images = Mutex<HashMap<std::path::PathBuf, (Option<std::time::SystemTime>, Arc<renzora_terrain::heightmap_import::HeightmapImage>)>>;
    static CACHE: OnceLock<Images> = OnceLock::new();
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let cache = CACHE.get_or_init(Default::default);
    if let Ok(map) = cache.lock() {
        if let Some((m, img)) = map.get(path) {
            if *m == modified {
                return Ok(img.clone());
            }
        }
    }
    let img = Arc::new(renzora_terrain::heightmap_import::load_heightmap_file(
        path,
        &renzora_terrain::heightmap_import::HeightmapFormat::Auto,
    )?);
    if let Ok(mut map) = cache.lock() {
        map.insert(path.to_path_buf(), (modified, img.clone()));
    }
    Ok(img)
}

pub(super) fn import_heightmap(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    let p = ctx.params;
    let raw = p.text("path").trim();
    if raw.is_empty() {
        return Err("pick a heightmap file".into());
    }
    let path = std::path::Path::new(raw);
    let path = match (path.is_absolute(), ctx.root) {
        (false, Some(root)) => root.join(path),
        _ => path.to_path_buf(),
    };
    let img = image(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (lo, hi) = img.range();
    let (normalize, invert, height) = (p.flag("normalize"), p.flag("invert"), p.num("height"));
    let extent = ctx.grid.extent;
    Ok(vec![Field::from_fn(&ctx.grid, |x, z| {
        let pos = ctx.pos(x, z);
        let mut s = img.sample_uv(pos.x / extent.x.max(1e-6), pos.y / extent.y.max(1e-6));
        if normalize && hi > lo {
            s = (s - lo) / (hi - lo);
        }
        if invert {
            s = 1.0 - s;
        }
        s * height
    })])
}

pub(super) fn current_terrain(ctx: &NodeCtx, _: &Inputs) -> Result<Vec<Field>, String> {
    match ctx.current {
        Some(f) => Ok(vec![f.clone()]),
        None => Err("no terrain to read: this graph is not being built for a terrain".into()),
    }
}
