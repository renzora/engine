//! `gradient="..."` and `shadow="..."` — native bevy_ui decoration attributes.
//!
//! Both are parsed at build time from `node.defs` (renzora_hui custom
//! attributes; bevy_hui's parser leaves unknown attrs as property strings) and
//! stamped as standard components — no per-frame system.
//!
//! ## `gradient`
//! CSS-ish, space separated (no commas — the markup parser splits on them):
//! - `gradient="linear 180deg #4C8BF5 #9B59B6"` — angle in degrees, then ≥2
//!   color stops spaced evenly.
//! - `gradient="radial #1B2838 #0B0E14"` — radial from the center.
//! - The `linear`/`radial` keyword and the `<n>deg` angle are optional;
//!   default is a linear top→bottom gradient.
//!
//! Renders over [`BackgroundColor`], so a solid `background` makes a safe
//! fallback. Maps to [`BackgroundGradient`].
//!
//! ## `shadow`
//! CSS `box-shadow` order — `<x> <y> <blur> [spread] <color>`:
//! - `shadow="0px 6px 16px #00000088"` — x, y, blur, color (spread 0).
//! - `shadow="0px 6px 16px 2px #00000088"` — x, y, blur, spread, color.
//!
//! Lengths accept `px`/`%`/bare-number (→ px). Maps to [`BoxShadow`].

use bevy::color::Color;
use bevy::prelude::*;
use bevy::ui::{
    BackgroundGradient, BoxShadow, ColorStop, Gradient, LinearGradient, RadialGradient, Val,
};

/// Parse a `#RGB` / `#RRGGBB` / `#RRGGBBAA` hex color (the form every demo
/// template uses). Returns `None` on anything else.
fn parse_hex_color(s: &str) -> Option<Color> {
    let h = s.trim().strip_prefix('#')?;
    let nib = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    let bytes = h.as_bytes();
    let (r, g, b, a) = match bytes.len() {
        3 => {
            let r = nib(bytes[0])?;
            let g = nib(bytes[1])?;
            let b = nib(bytes[2])?;
            (r * 17, g * 17, b * 17, 255)
        }
        6 | 8 => {
            let p = |i: usize| Some(nib(bytes[i])? * 16 + nib(bytes[i + 1])?);
            let a = if bytes.len() == 8 { p(6)? } else { 255 };
            (p(0)?, p(2)?, p(4)?, a)
        }
        _ => return None,
    };
    Some(Color::srgba_u8(r, g, b, a))
}

/// Parse a length token: `12px`, `25%`, or a bare number (→ px).
fn parse_val(s: &str) -> Option<Val> {
    let s = s.trim();
    if let Some(p) = s.strip_suffix('%') {
        return p.trim().parse::<f32>().ok().map(Val::Percent);
    }
    let n = s.strip_suffix("px").unwrap_or(s);
    n.trim().parse::<f32>().ok().map(Val::Px)
}

/// Build a [`BackgroundGradient`] from a `gradient="..."` value.
pub fn parse_gradient(value: &str) -> Option<BackgroundGradient> {
    let mut radial = false;
    let mut angle_deg = 180.0_f32; // default: top → bottom
    let mut stops: Vec<ColorStop> = Vec::new();

    for tok in value.split_whitespace() {
        match tok.to_ascii_lowercase().as_str() {
            "linear" => radial = false,
            "radial" => radial = true,
            t if t.ends_with("deg") => {
                if let Ok(a) = t.trim_end_matches("deg").parse::<f32>() {
                    angle_deg = a;
                }
            }
            _ => {
                if let Some(c) = parse_hex_color(tok) {
                    stops.push(ColorStop::auto(c));
                }
            }
        }
    }

    if stops.len() < 2 {
        return None;
    }

    let gradient = if radial {
        Gradient::Radial(RadialGradient {
            stops,
            ..default()
        })
    } else {
        Gradient::Linear(LinearGradient::new(angle_deg.to_radians(), stops))
    };
    Some(BackgroundGradient(vec![gradient]))
}

/// Build a [`BoxShadow`] from a `shadow="..."` value.
pub fn parse_shadow(value: &str) -> Option<BoxShadow> {
    let mut color = Color::srgba(0.0, 0.0, 0.0, 0.5);
    let mut vals: Vec<Val> = Vec::new();

    for tok in value.split_whitespace() {
        if let Some(c) = parse_hex_color(tok) {
            color = c;
        } else if let Some(v) = parse_val(tok) {
            vals.push(v);
        }
    }

    let (x, y, blur, spread) = match vals.as_slice() {
        [x, y, blur, spread] => (*x, *y, *blur, *spread),
        [x, y, blur] => (*x, *y, *blur, Val::ZERO),
        [x, y] => (*x, *y, Val::ZERO, Val::ZERO),
        _ => return None,
    };

    Some(BoxShadow::new(color, x, y, spread, blur))
}
