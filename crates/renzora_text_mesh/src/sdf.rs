//! Coverage bitmap → signed distance field.
//!
//! Bevy's font atlas stores each glyph as a *coverage* mask (alpha = how much of
//! the pixel the glyph covers). A coverage mask blurs when magnified. A signed
//! distance field instead stores, per texel, the distance to the glyph's
//! outline — which a shader can threshold sharply at any zoom. This converts one
//! to the other.
//!
//! The transform is a bounded brute-force nearest-opposite search: for each
//! texel, the distance to the closest texel of the opposite side (inside vs
//! outside) within `spread` px. Exact within the spread, and simple — glyphs are
//! small and this runs only when the text (re)builds, not per frame. The result
//! is normalized so 0.5 == the outline, rising to 1.0 `spread` px inside and
//! falling to 0.0 `spread` px outside.

/// Distance ramp radius, in coverage px. The shader's anti-alias band lives
/// inside this, so it needs a few px of room on each side of the outline — which
/// is why the glyph is padded by this much before the transform.
pub const SPREAD: i32 = 8;

/// Convert a padded coverage grid (`inside[y*w + x]`) to a normalized SDF byte
/// per texel. `w`/`h` are the padded dimensions; the transform searches up to
/// [`SPREAD`] px for the nearest opposite texel.
pub fn coverage_to_sdf(inside: &[bool], w: usize, h: usize) -> Vec<u8> {
    let r = SPREAD;
    let mut out = vec![0u8; w * h];

    for y in 0..h as i32 {
        for x in 0..w as i32 {
            let center = inside[y as usize * w + x as usize];

            // Nearest texel of the opposite side within the search radius.
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
                    let n = inside[ny as usize * w + nx as usize];
                    if n != center {
                        let d2 = (dx * dx + dy * dy) as f32;
                        if d2 < best_sq {
                            best_sq = d2;
                        }
                    }
                }
            }

            let dist = best_sq.sqrt().min(r as f32);
            // Inside → positive, outside → negative; 0 at the outline.
            let signed = if center { dist } else { -dist };
            // 0.5 = outline, +spread → 1.0, −spread → 0.0.
            let norm = (0.5 + signed / (2.0 * r as f32)).clamp(0.0, 1.0);
            out[y as usize * w + x as usize] = (norm * 255.0) as u8;
        }
    }

    out
}
