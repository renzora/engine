//! The Paint mode: the layers of the terrain's landscape material, and the
//! weight brush.
//!
//! The list is the material's layers (`TerrainLandscape::layers`), in the
//! order its Layer Blend nodes name them, each with its blend type and how
//! much of the terrain it covers. There is nothing to add here: a layer is
//! added in the material graph and appears in the list when the material is
//! saved. Selecting a row is what the brush paints.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use renzora_shader::material::landscape::LandscapeBlend;
use renzora_terrain::landscape::{LandscapeStatus, TerrainLandscape};
use renzora_terrain::paint::{PaintBrushType, SurfacePaintSettings, PAINT_RADIUS_RANGE};

use super::falloff::{falloff_card_rows, FalloffTarget};
use super::sculpt::{shape_at, shape_index};
use super::widgets::{
    bind_choice_border, card, choice_bg, column, group_label_live, hint, live_text, num_field, num_row, row,
    segmented_row, switch_row, unit,
};
use super::{arm, hasher, set_paint, LayerRow};
use crate::terrain_inspector::TerrainInspectorTab;

const MODES: [PaintBrushType; 4] = [PaintBrushType::Paint, PaintBrushType::Erase, PaintBrushType::Smooth, PaintBrushType::Fill];

fn ps<T: Default>(w: &Rx, f: impl Fn(&SurfacePaintSettings) -> T) -> T {
    w.get_resource::<SurfacePaintSettings>().map(f).unwrap_or_default()
}

pub(crate) fn blend_label(b: LandscapeBlend) -> &'static str {
    match b {
        LandscapeBlend::Weight => "Weight blend",
        LandscapeBlend::Height => "Height blend",
        LandscapeBlend::Alpha => "Alpha blend",
    }
}

/// A swatch colour for a layer, from its name.
///
/// The material graph decides what a layer looks like, and reading that back
/// would mean evaluating the graph per layer. The common ground names get the
/// colour they are; anything else gets a stable muted hue from its name, so
/// two layers never share a swatch by accident and a layer keeps its colour.
pub(crate) fn layer_swatch(name: &str) -> Color {
    let n = name.to_ascii_lowercase();
    let known: [(&str, (f32, f32, f32)); 9] = [
        ("grass", (0.37, 0.54, 0.26)),
        ("moss", (0.33, 0.45, 0.22)),
        ("rock", (0.54, 0.54, 0.53)),
        ("cliff", (0.48, 0.47, 0.45)),
        ("snow", (0.91, 0.93, 0.95)),
        ("sand", (0.80, 0.72, 0.52)),
        ("dirt", (0.45, 0.35, 0.25)),
        ("gravel", (0.49, 0.42, 0.32)),
        ("mud", (0.36, 0.28, 0.20)),
    ];
    if let Some((_, (r, g, b))) = known.iter().find(|(k, _)| n.contains(k)) {
        return Color::srgb(*r, *g, *b);
    }
    let mut h = hasher();
    n.hash(&mut h);
    Color::hsl((h.finish() % 360) as f32, 0.35, 0.5)
}

pub(super) fn paint_content(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let root = column(commands, 10.0);

    let layers = column(commands, 6.0);
    let label = group_label_live(commands, fonts, "Layers", move |w| {
        w.get::<TerrainLandscape>(terrain)
            .filter(|l| !l.path.is_empty())
            .map(|l| format!("from {}", super::file_stem(&l.path)))
            .unwrap_or_default()
    });
    let list = column(commands, 3.0);
    keyed_list(commands, list, move |w| layers_snapshot(w, terrain));
    let status = live_text(commands, fonts, 11.0, text_muted(), move |w| status_text(w, terrain));
    bind_display(commands, status, move |w| !status_text(w, terrain).is_empty());
    let note = hint(commands, fonts, "Layers come from the material's Layer Blend nodes. Add one in the graph and it appears here.");
    commands.entity(layers).add_children(&[label, list, status, note]);

    let brush_col = column(commands, 6.0);
    let brush_label = super::widgets::group_label(commands, fonts, "Brush", "scroll wheel resizes");
    let brush = brush_card(commands, fonts);
    commands.entity(brush_col).add_children(&[brush_label, brush]);

    commands.entity(root).add_children(&[layers, brush_col]);
    root
}

/// What to say instead of (or under) the list, by the material's state.
fn status_text(w: &Rx, terrain: Entity) -> String {
    let Some(l) = w.get::<TerrainLandscape>(terrain) else {
        return "No landscape material yet. Choose one above, or use Alpine Landscape.".into();
    };
    match &l.status {
        LandscapeStatus::None => "No landscape material yet. Choose one above, or use Alpine Landscape.".into(),
        LandscapeStatus::Loading => "Loading the material...".into(),
        LandscapeStatus::Plain => "This material has no Landscape layers, so there is nothing to paint.".into(),
        LandscapeStatus::Failed(why) => format!("The material could not be used: {why}"),
        LandscapeStatus::Ready if l.layers.is_empty() => "The material names no layers.".into(),
        LandscapeStatus::Ready => String::new(),
    }
}

fn layers_snapshot(w: &Rx, terrain: Entity) -> KeyedSnapshot {
    let layers: Vec<(String, LandscapeBlend)> = w
        .get::<TerrainLandscape>(terrain)
        .filter(|l| l.status == LandscapeStatus::Ready)
        .map(|l| l.layers.iter().map(|x| (x.name.clone(), x.blend)).collect())
        .unwrap_or_default();
    // Keyed and hashed on structure (index, name, blend), never on coverage or
    // selection, which bind per row: painting must not rebuild the list.
    let items = layers
        .iter()
        .enumerate()
        .map(|(i, (name, blend))| {
            let mut k = hasher();
            (i, name).hash(&mut k);
            let mut h = hasher();
            (i, name, blend_label(*blend)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| layer_row(c, f, terrain, i, &layers[i].0, layers[i].1)),
    }
}

fn layer_row(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity, index: usize, name: &str, blend: LandscapeBlend) -> Entity {
    let chosen = move |w: &Rx| {
        let n = w.get::<TerrainLandscape>(terrain).map_or(0, |l| l.layers.len());
        n > 0 && ps(w, |s| s.active_layer).min(n - 1) == index
    };
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(8.0),
                padding: UiRect::axes(Val::Px(6.0), Val::Px(5.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(Color::NONE),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            LayerRow { index },
            Name::new(format!("terrain-layer:{name}")),
        ))
        .id();
    bind_bg(commands, row, move |w| choice_bg(w, row, chosen(w)));
    bind_choice_border(commands, row, chosen);

    let swatch = commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(28.0),
                flex_shrink: 0.0,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(layer_swatch(name)),
            PASS_THROUGH,
        ))
        .id();

    let mid = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(2.0),
                ..default()
            },
            PASS_THROUGH,
        ))
        .id();
    let nm = commands
        .spawn((Text::new(name.to_string()), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), PASS_THROUGH))
        .id();
    let md = commands
        .spawn((Text::new(blend_label(blend)), ui_font(&fonts.ui, 10.0), TextColor(rgb(placeholder())), PASS_THROUGH))
        .id();
    let bar = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(3.0),
                border_radius: BorderRadius::all(Val::Px(2.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(border())),
            PASS_THROUGH,
        ))
        .id();
    let fill = commands
        .spawn((Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() }, BackgroundColor(rgb(accent())), PASS_THROUGH))
        .id();
    commands.entity(bar).add_child(fill);
    let coverage = move |w: &Rx| w.get::<TerrainLandscape>(terrain).and_then(|l| l.layers.get(index)).map_or(0.0, |l| l.coverage);
    bind_with(commands, fill, move |w| (coverage(w) * 1000.0).round() as i32, |world, e, v: &i32| {
        if let Some(mut n) = world.get_mut::<Node>(e) {
            n.width = Val::Percent(*v as f32 / 10.0);
        }
    });
    commands.entity(mid).add_children(&[nm, md, bar]);

    let pc = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.mono, 11.0),
            TextColor(rgb(text_muted())),
            Node { min_width: Val::Px(36.0), flex_shrink: 0.0, ..default() },
            PASS_THROUGH,
        ))
        .id();
    bind_text(commands, pc, move |w| format!("{:.0}%", coverage(w) * 100.0));

    commands.entity(row).add_children(&[swatch, mid, pc]);
    row
}

fn brush_card(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let c = card(commands);
    let mode = segmented_row(commands, fonts, "Mode", &["Paint", "Erase", "Smooth", "Fill"],
        |w| MODES.iter().position(|m| *m == ps(w, |s| s.brush_type)).unwrap_or(0),
        |w, i| {
            set_paint(w, |s| s.brush_type = MODES[(*i).min(3)]);
            // Picking a mode is picking up the brush.
            arm(w, TerrainInspectorTab::Paint);
        },
    );
    let size = num_row(commands, fonts, "Size", "m", PAINT_RADIUS_RANGE.0, PAINT_RADIUS_RANGE.1, 0.25,
        |w| ps(w, |s| s.brush_radius), |w, v| set_paint(w, |s| s.brush_radius = *v));
    let strength = num_row(commands, fonts, "Strength", "%", 1.0, 100.0, 0.5,
        |w| ps(w, |s| s.brush_strength) * 100.0, |w, v| set_paint(w, |s| s.brush_strength = (*v / 100.0).clamp(0.01, 1.0)));
    let target = num_row(commands, fonts, "Target weight", "%", 0.0, 100.0, 0.5,
        |w| ps(w, |s| s.target_weight) * 100.0, |w, v| set_paint(w, |s| s.target_weight = *v / 100.0));
    let falloff = falloff_card_rows(commands, fonts, FalloffTarget::Paint);
    let shape = segmented_row(commands, fonts, "Shape", &["Circle", "Square", "Diamond"],
        |w| shape_index(ps(w, |s| s.brush_shape)), |w, i| set_paint(w, |s| s.brush_shape = shape_at(*i)));

    let slope = mask_rows(commands, fonts, "Slope mask", "°", (0.0, 90.0), (0.0, 30.0),
        |s| s.slope_mask, |s, v| s.slope_mask = v);
    let height = mask_rows(commands, fonts, "Height mask", "m", (-2000.0, 8000.0), (0.0, 100.0),
        |s| s.height_mask, |s, v| s.height_mask = v);
    let note = hint(commands, fonts, "A mask limits the brush to ground in its range: rock only on steep slopes, snow only up high. Heights are world metres.");

    let mut kids = vec![mode, size, strength, target];
    kids.extend(falloff);
    kids.push(shape);
    kids.extend(slope);
    kids.extend(height);
    kids.push(note);
    commands.entity(c).add_children(&kids);
    c
}

/// A mask: an on switch, and the range it keeps while on. The range rows
/// hide while the mask is off; turning it on starts from `default`.
#[allow(clippy::too_many_arguments)]
fn mask_rows(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    unit_text: &str,
    bounds: (f32, f32),
    default: (f32, f32),
    get: fn(&SurfacePaintSettings) -> Option<Vec2>,
    put: fn(&mut SurfacePaintSettings, Option<Vec2>),
) -> Vec<Entity> {
    let (toggle, _) = switch_row(commands, fonts, label,
        move |w| ps(w, |s| get(s).is_some()),
        move |w, on| set_paint(w, |s| put(s, on.then(|| get(s).unwrap_or(Vec2::new(default.0, default.1))))),
    );
    let (range, content) = row(commands, fonts, "");
    let lo = num_field(commands, fonts, "min", bounds.0, bounds.1, 0.5, None,
        move |w| ps(w, |s| get(s).map_or(default.0, |v| v.x)),
        move |w, v| set_paint(w, |s| if let Some(m) = get(s) { put(s, Some(Vec2::new(*v, m.y.max(*v)))) }),
    );
    let hi = num_field(commands, fonts, "max", bounds.0, bounds.1, 0.5, None,
        move |w| ps(w, |s| get(s).map_or(default.1, |v| v.y)),
        move |w, v| set_paint(w, |s| if let Some(m) = get(s) { put(s, Some(Vec2::new(m.x.min(*v), *v))) }),
    );
    let u = unit(commands, fonts, unit_text);
    commands.entity(content).add_children(&[lo, hi, u]);
    bind_display(commands, range, move |w| ps(w, |s| get(s).is_some()));
    vec![toggle, range]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_layers_get_their_colour_and_others_a_stable_one() {
        let snow = layer_swatch("Snow").to_srgba();
        assert!(snow.red > 0.85 && snow.blue > 0.85);
        let grass = layer_swatch("Meadow grass").to_srgba();
        assert!(grass.green > grass.red && grass.green > grass.blue);
        assert_eq!(layer_swatch("Lava"), layer_swatch("lava"));
    }

    #[test]
    fn every_paint_mode_is_on_the_segmented_control() {
        for m in [PaintBrushType::Paint, PaintBrushType::Erase, PaintBrushType::Smooth, PaintBrushType::Fill] {
            assert!(MODES.contains(&m));
        }
    }
}
