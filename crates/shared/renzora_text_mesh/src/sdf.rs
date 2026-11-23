//! Coverage bitmap → signed distance field.
//!
//! Bevy's font atlas stores each glyph as a *coverage* mask (alpha = how much of
//! the pixel the glyph covers). A coverage mask blurs when magnified. A signed
//! distance field instead stores, per texel, the distance to the glyph's
//! outline — which a shader can threshold sharply at any zoom. This converts one
//! to the other.
//!
//! For each texel: the distance to the closest texel of the opposite side
//! (inside vs outside), clamped to `spread` px. It is normalized so 0.5 == the
//! outline, rising to 1.0 `spread` px inside and falling to 0.0 `spread` px
//! outside.
//!
//! The distances come from an exact Euclidean distance transform (Felzenszwalb
//! and Huttenlocher's two-pass lower envelope), a few operations per texel. It
//! used to be a brute-force search of the (2 x spread + 1)² window around every
//! texel, about 290 lookups each, which made building one line of 3D text take
//! 46 ms on the main thread when a scene loaded. The two agree exactly: any
//! opposite texel within `spread` lies inside that window, and beyond it both
//! clamp to `spread` (the test below holds the old search as the reference).

/// Distance ramp radius, in coverage px. The shader's anti-alias band lives
/// inside this, so it needs a few px of room on each side of the outline — which
/// is why the glyph is padded by this much before the transform.
pub const SPREAD: i32 = 8;

/// Stand-in for "no feature texel": large enough never to be the nearest, small
/// enough that the envelope arithmetic below stays finite.
const FAR: f32 = 1.0e20;

/// Convert a padded coverage grid (`inside[y*w + x]`) to a normalized SDF byte
/// per texel. `w`/`h` are the padded dimensions; distances are clamped to
/// [`SPREAD`] px.
pub fn coverage_to_sdf(inside: &[bool], w: usize, h: usize) -> Vec<u8> {
    let r = SPREAD as f32;
    // Squared distance from each texel to the nearest outside texel, and to the
    // nearest inside one. A texel uses whichever names the opposite side.
    let to_outside = squared_distance(|i| !inside[i], w, h);
    let to_inside = squared_distance(|i| inside[i], w, h);
    (0..w * h)
        .map(|i| {
            let d2 = if inside[i] { to_outside[i] } else { to_inside[i] };
            let dist = d2.sqrt().min(r);
            let signed = if inside[i] { dist } else { -dist };
            // 0.5 = outline, +spread → 1.0, −spread → 0.0.
            let norm = (0.5 + signed / (2.0 * r)).clamp(0.0, 1.0);
            (norm * 255.0) as u8
        })
        .collect()
}

/// Exact squared Euclidean distance from every texel to the nearest texel for
/// which `feature` holds: the 1D transform down every column, then along every
/// row of the result.
fn squared_distance(feature: impl Fn(usize) -> bool, w: usize, h: usize) -> Vec<f32> {
    let mut grid: Vec<f32> = (0..w * h).map(|i| if feature(i) { 0.0 } else { FAR }).collect();
    let n = w.max(h);
    let mut f = vec![0.0f32; n];
    let mut d = vec![0.0f32; n];
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f32; n + 1];

    for x in 0..w {
        for y in 0..h {
            f[y] = grid[y * w + x];
        }
        transform_1d(&f[..h], &mut d[..h], &mut v, &mut z);
        for y in 0..h {
            grid[y * w + x] = d[y];
        }
    }
    for y in 0..h {
        f[..w].copy_from_slice(&grid[y * w..y * w + w]);
        transform_1d(&f[..w], &mut d[..w], &mut v, &mut z);
        grid[y * w..y * w + w].copy_from_slice(&d[..w]);
    }
    grid
}

/// One-dimensional squared distance transform of `f` into `d`: the lower
/// envelope of the parabolas rooted at each sample. `v` and `z` are scratch,
/// at least `f.len()` and `f.len() + 1` long.
fn transform_1d(f: &[f32], d: &mut [f32], v: &mut [usize], z: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let fq = f[q] + (q * q) as f32;
        let mut s;
        loop {
            let p = v[k];
            s = (fq - (f[p] + (p * p) as f32)) / (2.0 * q as f32 - 2.0 * p as f32);
            // `z[0]` is minus infinity, so this stops at the first parabola.
            if s > z[k] {
                break;
            }
            k -= 1;
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, out) in d.iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let p = v[k];
        let dq = q as f32 - p as f32;
        *out = dq * dq + f[p];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The search this replaced, kept as the definition the transform has to
    /// match byte for byte.
    fn brute_force(inside: &[bool], w: usize, h: usize) -> Vec<u8> {
        let r = SPREAD;
        let mut out = vec![0u8; w * h];
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let center = inside[y as usize * w + x as usize];
                let mut best_sq = ((r + 1) * (r + 1)) as f32;
                for dy in -r..=r {
                    let ny = y + dy;
                    if ny < 0 || ny >= h as i32 {
                        continue;
                    }
                    for dx in -r..=r {
                        let nx = x + dx;
                        if nx < 0 || nx >= w as i32 {
                            continue;
                        }
                        if inside[ny as usize * w + nx as usize] != center {
                            best_sq = best_sq.min((dx * dx + dy * dy) as f32);
                        }
                    }
                }
                let dist = best_sq.sqrt().min(r as f32);
                let signed = if center { dist } else { -dist };
                let norm = (0.5 + signed / (2.0 * r as f32)).clamp(0.0, 1.0);
                out[y as usize * w + x as usize] = (norm * 255.0) as u8;
            }
        }
        out
    }

    /// A deterministic scatter of shapes, so the comparison covers outlines at
    /// every angle, thin strokes, holes and empty margins.
    fn pattern(w: usize, h: usize, seed: u32) -> Vec<bool> {
        let mut state = seed.wrapping_mul(2654435761).max(1);
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            state
        };
        let blobs: Vec<(f32, f32, f32)> = (0..5)
            .map(|_| {
                (
                    (next() % w as u32) as f32,
                    (next() % h as u32) as f32,
                    2.0 + (next() % 9) as f32,
                )
            })
            .collect();
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f32, (i / w) as f32);
                let in_blob = blobs
                    .iter()
                    .filter(|(cx, cy, r)| (x - cx).powi(2) + (y - cy).powi(2) < r * r)
                    .count();
                // Odd overlap counts carve holes where blobs cross.
                in_blob % 2 == 1 || (x as usize + 3 * y as usize) % 37 == 0
            })
            .collect()
    }

    #[test]
    fn matches_the_brute_force_search_exactly() {
        for (seed, (w, h)) in [(40, 30), (17, 41), (64, 64), (5, 3), (1, 1), (33, 1)]
            .into_iter()
            .enumerate()
        {
            let grid = pattern(w, h, seed as u32 + 1);
            assert_eq!(coverage_to_sdf(&grid, w, h), brute_force(&grid, w, h), "{w}x{h}");
        }
    }

    #[test]
    fn an_empty_or_full_grid_clamps_to_the_spread() {
        let empty = vec![false; 20 * 10];
        assert!(coverage_to_sdf(&empty, 20, 10).iter().all(|&b| b == 0));
        let full = vec![true; 20 * 10];
        assert!(coverage_to_sdf(&full, 20, 10).iter().all(|&b| b == 255));
    }
}
