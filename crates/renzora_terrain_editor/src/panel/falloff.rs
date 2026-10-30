//! The brush falloff: a named curve picker, the soft-edge amount, and a drawn
//! preview of the curve the brush actually uses.
//!
//! The curve was five letter buttons (S, L, O, T, F), which said nothing about
//! what each did until you had tried them all. The preview is drawn from
//! `renzora_terrain::data::compute_brush_falloff`, the function the brushes
//! call, so it is the brush's real profile from the centre (left) to the rim
//! (right), not an illustration of it.

use bevy::prelude::*;

use renzora_ember::font::EmberFonts;
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;

use renzora_terrain::data::{compute_brush_falloff, BrushFalloffType, TerrainSettings};
use renzora_terrain::paint::SurfacePaintSettings;

use super::canvas::{attach_canvas, rgba8, write_pixels, Pixels};
use super::widgets::{num_field, row, unit};
use super::{set_paint, set_settings};

/// The curves in the order the picker lists them, with the name each goes by.
pub(crate) const CURVES: [(BrushFalloffType, &str); 5] = [
    (BrushFalloffType::Smooth, "Smooth"),
    (BrushFalloffType::Linear, "Linear"),
    (BrushFalloffType::Spherical, "Round"),
    (BrushFalloffType::Tip, "Sharp"),
    (BrushFalloffType::Flat, "Hard"),
];

const CURVE_W: u32 = 192;
const CURVE_H: u32 = 36;

/// Which brush a falloff control edits. Sculpt and paint keep separate brushes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum FalloffTarget {
    Sculpt,
    Paint,
}

/// The preview image of one target's curve, with what it was last drawn from.
#[derive(Component)]
pub(crate) struct FalloffCurve {
    target: FalloffTarget,
    drawn: Option<(BrushFalloffType, u32, [u8; 4])>,
}

fn read(w: &Rx, target: FalloffTarget) -> (BrushFalloffType, f32) {
    match target {
        FalloffTarget::Sculpt => w
            .get_resource::<TerrainSettings>()
            .map(|s| (s.falloff_type, s.falloff))
            .unwrap_or((BrushFalloffType::Smooth, 0.7)),
        FalloffTarget::Paint => w
            .get_resource::<SurfacePaintSettings>()
            .map(|s| (s.falloff_type, s.brush_falloff))
            .unwrap_or((BrushFalloffType::Smooth, 0.6)),
    }
}

fn write(w: &mut World, target: FalloffTarget, ft: Option<BrushFalloffType>, amount: Option<f32>) {
    match target {
        FalloffTarget::Sculpt => set_settings(w, |s| {
            if let Some(ft) = ft {
                s.falloff_type = ft;
            }
            if let Some(a) = amount {
                s.falloff = a;
            }
        }),
        FalloffTarget::Paint => set_paint(w, |s| {
            if let Some(ft) = ft {
                s.falloff_type = ft;
            }
            if let Some(a) = amount {
                s.brush_falloff = a;
            }
        }),
    }
}

pub(crate) fn curve_index(ft: BrushFalloffType) -> usize {
    CURVES.iter().position(|(c, _)| *c == ft).unwrap_or(0)
}

/// The rows for one brush's falloff: curve and soft edge, then the preview.
pub(super) fn falloff_card_rows(commands: &mut Commands, fonts: &EmberFonts, target: FalloffTarget) -> Vec<Entity> {
    let names: Vec<&str> = CURVES.iter().map(|(_, n)| *n).collect();
    let (picker, content) = row(commands, fonts, "Falloff");
    let dd = renzora_ember::widgets::dropdown_compact(commands, fonts, &names, 0, 92.0);
    renzora_ember::reactive::tracked::bind_2way(
        commands,
        dd,
        move |w| curve_index(read(w, target).0),
        move |w, i: &usize| write(w, target, Some(CURVES[(*i).min(CURVES.len() - 1)].0), None),
    );
    // How much of the radius fades: 0 is a hard edge, 100% fades from the
    // centre out.
    let amount = num_field(commands, fonts, "", 0.0, 100.0, 0.5, None,
        move |w| read(w, target).1 * 100.0,
        move |w, v| write(w, target, None, Some((*v / 100.0).clamp(0.0, 1.0))),
    );
    let u = unit(commands, fonts, "% soft");
    commands.entity(content).add_children(&[dd, amount, u]);

    let (preview, pc) = row(commands, fonts, "");
    let curve = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                height: Val::Px(CURVE_H as f32),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            FalloffCurve { target, drawn: None },
            Name::new("terrain-falloff-curve"),
        ))
        .id();
    attach_canvas(commands, curve, CURVE_W, CURVE_H);
    commands.entity(pc).add_child(curve);
    vec![picker, preview]
}

/// The curve's pixels: an accent line with a light fill beneath, centre on the
/// left, rim on the right.
pub(crate) fn curve_pixels(ft: BrushFalloffType, amount: f32, line: [u8; 4], fill: [u8; 4]) -> Pixels {
    let mut px = Pixels::new(CURVE_W, CURVE_H);
    let top = 4.0;
    let span = CURVE_H as f32 - 8.0;
    let y_at = |x: u32| {
        let t = x as f32 / (CURVE_W - 1) as f32;
        // `compute_brush_falloff` returns 0 at t = 1 exactly; sample just
        // inside so the rim is drawn where the brush still acts.
        let v = compute_brush_falloff(t.min(0.999), amount, ft);
        top + (1.0 - v) * span
    };
    let mut prev = y_at(0);
    for x in 0..CURVE_W {
        let y = y_at(x);
        for yy in (y.round() as i32)..CURVE_H as i32 {
            px.put(x as i32, yy, fill);
        }
        // Join to the previous column so steep drops stay a line, not dots.
        let (a, b) = if prev < y { (prev, y) } else { (y, prev) };
        for yy in (a.floor() as i32)..=(b.ceil() as i32) {
            px.put(x as i32, yy, line);
            px.put(x as i32, yy + 1, line);
        }
        prev = y;
    }
    px
}

/// Redraw each preview when its brush's curve or the theme accent changed.
pub(crate) fn draw_falloff_curves(
    mut curves: Query<(&mut FalloffCurve, &ImageNode)>,
    sculpt: Option<Res<TerrainSettings>>,
    paint: Option<Res<SurfacePaintSettings>>,
    mut images: ResMut<Assets<Image>>,
) {
    let accent_px = rgba8(rgb(accent()), 1.0);
    for (mut curve, node) in &mut curves {
        let (ft, amount) = match curve.target {
            FalloffTarget::Sculpt => sculpt.as_ref().map(|s| (s.falloff_type, s.falloff)),
            FalloffTarget::Paint => paint.as_ref().map(|s| (s.falloff_type, s.brush_falloff)),
        }
        .unwrap_or((BrushFalloffType::Smooth, 0.7));
        let key = (ft, amount.to_bits(), accent_px);
        if curve.drawn == Some(key) {
            continue;
        }
        let px = curve_pixels(ft, amount, accent_px, rgba8(rgb(accent()), 0.2));
        write_pixels(&mut images, node, px.w, px.h, px.data);
        curve.drawn = Some(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_curve_has_a_distinct_name_and_index() {
        for (i, (ft, _)) in CURVES.iter().enumerate() {
            assert_eq!(curve_index(*ft), i);
        }
        let mut names: Vec<&str> = CURVES.iter().map(|(_, n)| *n).collect();
        names.dedup();
        assert_eq!(names.len(), CURVES.len());
    }

    /// The drawing follows the brush: a hard curve stays at the top across the
    /// whole width, a linear one ends near the bottom at the rim.
    #[test]
    fn the_preview_is_the_brush_profile() {
        let line = [255, 0, 0, 255];
        let column_top = |px: &Pixels, x: u32| (0..px.h).find(|y| px.data[((y * px.w + x) * 4) as usize] == 255).unwrap();
        let hard = curve_pixels(BrushFalloffType::Flat, 0.7, line, [0, 0, 0, 0]);
        assert!(column_top(&hard, CURVE_W - 1) <= 5);
        let linear = curve_pixels(BrushFalloffType::Linear, 1.0, line, [0, 0, 0, 0]);
        assert!(column_top(&linear, 0) <= 5);
        assert!(column_top(&linear, CURVE_W - 1) >= CURVE_H - 6);
    }
}
