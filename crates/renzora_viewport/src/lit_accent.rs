//! The accent colour, adjusted to be *drawn in* rather than filled with.
//!
//! Two controls in this crate report a persistent on-state — the nav cluster's
//! Grid toggle and the toolbar's Snap trigger — and neither should take the
//! solid accent fill. A fill is the language of "this is the picked one" or
//! "you are holding this right now", so a permanently filled button among its
//! neighbours reads as a selection rather than a switch. What they want instead
//! is to look *lit*: the accent in the glyph, over a wash of it.
//!
//! That turns out to need more care than `rgb(accent())`, for a reason that is
//! structural rather than a bad constant. The accent is chosen to carry
//! `on_accent` text **on top of it**, which pins its lightness to the middle of
//! the range — the shipped dark blue is `l = 0.66` against white glyphs at
//! `l ≈ 0.92`, and the light one is `l = 0.47` against near-black text. Either
//! way a glyph drawn *in* the accent is closer to its background than the plain
//! glyphs beside it, which is the exact opposite of what a lit control should
//! say.

use bevy::color::Hsla;
use bevy::prelude::*;

/// How much accent sits behind a lit control.
///
/// Deliberately a *wash*, not a fill: at full strength it reads as "the
/// selected one", which is not what a toggle is saying. At about a third of
/// that it reads as a lit key on a keyboard.
///
/// This and [`accent_glyph_on`] are balanced against each other — raising the
/// wash darkens (or lightens) what the glyph is sitting on, so it cannot go
/// much further without eating the contrast it exists to add. At `0.32` the
/// tile clears about 1.6:1 against its surface while the glyph still clears
/// 5.5:1 on the tile, in both shipped themes.
pub(crate) const ACTIVE_WASH: f32 = 0.32;

/// Rec. 709 luma, for deciding which way is "away from" a surface.
fn is_light(c: (u8, u8, u8)) -> bool {
    0.2126 * c.0 as f32 + 0.7152 * c.1 as f32 + 0.0722 * c.2 as f32 > 140.0
}

/// The accent, pushed away from `surface` far enough to read as lit on it.
///
/// **The direction is not fixed.** Lightening unconditionally is the mistake
/// `renzora_ember::theme::mix`'s docs warn about, and it inverts exactly where
/// you would expect: on the Light theme the accent is `(38, 108, 200)` on a
/// `(244, 245, 248)` panel, and lightening it takes the contrast from 4.74
/// *down* to 1.90 — measurably worse than leaving the accent alone. Darkening
/// it there gives 10.09 instead.
///
/// In **HSL**, not `Luminance::lighter`: that works in Lab, where changing
/// lightness pulls the colour toward white (or black) and takes the chroma with
/// it. The result is a pale grey-blue that reads washed out rather than lit —
/// brighter, and less blue, when the hue is the one thing this colour has to
/// keep. Nudging saturation alongside is what keeps a less-saturated theme
/// accent recognisable once it has moved this far.
///
/// The clamps are the real limit, and they are why the wash exists: a saturated
/// blue cannot get much brighter without going pale, so past `l ≈ 0.8` every
/// further step buys contrast by spending colour.
pub(crate) fn accent_glyph_on(accent: (u8, u8, u8), surface: (u8, u8, u8)) -> Color {
    let mut hsl = Hsla::from(Color::srgb_u8(accent.0, accent.1, accent.2));
    hsl.lightness = if is_light(surface) {
        (hsl.lightness - 0.24).max(0.24)
    } else {
        (hsl.lightness + 0.28).min(0.82)
    };
    hsl.saturation = (hsl.saturation + 0.12).min(1.0);
    Color::from(hsl)
}

/// The accent at [`ACTIVE_WASH`], for the background of a lit control.
pub(crate) fn accent_wash(accent: (u8, u8, u8)) -> Color {
    Color::srgba(
        accent.0 as f32 / 255.0,
        accent.1 as f32 / 255.0,
        accent.2 as f32 / 255.0,
        ACTIVE_WASH,
    )
}
