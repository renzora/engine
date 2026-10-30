//! The Terrain component's **Foliage** tab: the foliage types as tiles, the
//! chosen type's properties, and the brush.
//!
//! Mounted by `renzora_terrain_editor` beside Sculpt and Paint, because it is a
//! third way of painting one terrain, not a feature of its own. The dependency
//! runs terrain-editor to here only.
//!
//! Everything is in real units: brush size in metres (the brush stores a
//! fraction of a chunk side, converted here against the terrain's chunk size),
//! density per square metre, blade heights in metres and widths in
//! centimetres. A type's tile shows how many blades it has on the terrain, an
//! estimate from the painted density ([`FoliageCounts`]).
//!
//! Every control writes [`FoliagePaintSettings`] or [`FoliageConfig`], the
//! resources the foliage systems read.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;

use renzora_editor_framework::{ActiveTool, SplashState};
use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_display, bind_text, bind_with, keyed_list};
use renzora_ember::reactive::{KeyedSnapshot, Rx};
use renzora_ember::theme::*;
use renzora_ember::widgets::{bind_text_input, drag_value, segmented, text_input, toggle_switch, DragRange, DragSnap};
use renzora_ember::UiInteraction;

use renzora_terrain::data::{TerrainChunkOf, TerrainData};
use renzora_terrain::foliage::{
    FoliageBrushType, FoliageConfig, FoliageDensityMap, FoliagePaintSettings, FoliageType, MAX_FOLIAGE_TYPES,
    MAX_HEIGHT_SCALE, MIN_HEIGHT_SCALE,
};

const LABEL_W: f32 = 92.0;
/// The brush radius limits, as the fraction of a chunk side it is stored as.
const RADIUS_FRACTION: (f32, f32) = (0.01, 0.5);
const MODES: [FoliageBrushType; 4] = [FoliageBrushType::Paint, FoliageBrushType::Erase, FoliageBrushType::Grow, FoliageBrushType::Trim];

pub struct FoliagePanel;

impl Plugin for FoliagePanel {
    fn build(&self, app: &mut App) {
        app.init_resource::<FoliageCounts>().add_systems(
            Update,
            (foliage_tile_click, foliage_add_type, foliage_remove_type, count_foliage).run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State ────────────────────────────────────────────────────────────────────

fn settings(w: &Rx) -> FoliagePaintSettings {
    w.get_resource::<FoliagePaintSettings>().cloned().unwrap_or_default()
}

fn set_settings(w: &mut World, f: impl FnOnce(&mut FoliagePaintSettings)) {
    let mut s = w.get_resource::<FoliagePaintSettings>().cloned().unwrap_or_default();
    f(&mut s);
    w.insert_resource(s);
}

fn config(w: &Rx) -> FoliageConfig {
    w.get_resource::<FoliageConfig>().cloned().unwrap_or_default()
}

fn set_config(w: &mut World, f: impl FnOnce(&mut FoliageConfig)) {
    let mut c = w.get_resource::<FoliageConfig>().cloned().unwrap_or_default();
    f(&mut c);
    w.insert_resource(c);
}

fn active_type(w: &Rx) -> usize {
    settings(w).active_type
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

/// Metres per unit of brush radius: the terrain's chunk side.
fn chunk_size(w: &Rx, terrain: Entity) -> f32 {
    w.get::<TerrainData>(terrain).map_or(64.0, |t| t.chunk_size)
}

/// Brush radius in metres from the stored fraction, and back.
pub fn radius_metres(fraction: f32, chunk_size: f32) -> f32 {
    fraction * chunk_size
}

pub fn radius_fraction(metres: f32, chunk_size: f32) -> f32 {
    (metres / chunk_size.max(1e-3)).clamp(RADIUS_FRACTION.0, RADIUS_FRACTION.1)
}

/// Estimated blades on the terrain per foliage type, from the painted
/// density. Recounted only when a density map changed.
#[derive(Resource, Default, PartialEq)]
pub struct FoliageCounts {
    pub blades: [u64; MAX_FOLIAGE_TYPES],
}

/// The blades a type's painted weight stands for: mean weight over the chunk
/// times its area times the type's density and blades per clump.
pub fn estimate_blades(weight_sum: f32, texels: usize, chunk_area: f32, ty: &FoliageType) -> u64 {
    if texels == 0 {
        return 0;
    }
    let mean = weight_sum / texels as f32;
    (mean * chunk_area * ty.density * ty.blades_per_clump as f32).round().max(0.0) as u64
}

fn count_foliage(
    changed: Query<(), Changed<FoliageDensityMap>>,
    maps: Query<(&FoliageDensityMap, &TerrainChunkOf)>,
    terrains: Query<&TerrainData>,
    config: Option<Res<FoliageConfig>>,
    mut counts: ResMut<FoliageCounts>,
) {
    let config_changed = config.as_ref().is_some_and(|c| c.is_changed());
    if changed.is_empty() && !config_changed {
        return;
    }
    let Some(config) = config else { return };
    let mut next = FoliageCounts::default();
    for (map, of) in &maps {
        let area = terrains.get(of.0).map_or(0.0, |t| t.chunk_size * t.chunk_size);
        let mut sums = [0.0f32; MAX_FOLIAGE_TYPES];
        for w in &map.density_weights {
            for (s, v) in sums.iter_mut().zip(w) {
                *s += v;
            }
        }
        for (i, ty) in config.types.iter().enumerate().take(MAX_FOLIAGE_TYPES) {
            if ty.enabled {
                next.blades[i] += estimate_blades(sums[i], map.density_weights.len(), area, ty);
            }
        }
    }
    if *counts != next {
        *counts = next;
    }
}

pub fn fmt_blades(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1} M blades", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0} k blades", n as f64 / 1_000.0)
    } else if n == 0 {
        "not painted".into()
    } else {
        format!("{n} blades")
    }
}

// ── Layout ───────────────────────────────────────────────────────────────────

/// The Foliage tab for `terrain`. Public because the Terrain component
/// mounts it.
pub fn build(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let root = column(commands, 10.0);

    let types = column(commands, 6.0);
    let label = group_label(commands, fonts, "Types", "grass painted onto the terrain");
    let tiles = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(5.0),
            row_gap: Val::Px(5.0),
            ..default()
        })
        .id();
    keyed_list(commands, tiles, types_snapshot);
    commands.entity(types).add_children(&[label, tiles]);

    let props = properties_card(commands, fonts);

    let brush = column(commands, 6.0);
    let blabel = group_label(commands, fonts, "Brush", "scroll wheel resizes");
    let bcard = brush_card(commands, fonts, terrain);
    commands.entity(brush).add_children(&[blabel, bcard]);

    commands.entity(root).add_children(&[types, props, brush]);
    root
}

#[derive(Component)]
struct TypeTile {
    index: usize,
}
#[derive(Component)]
struct AddTypeBtn;
#[derive(Component)]
struct RemoveTypeBtn;

fn types_snapshot(w: &Rx) -> KeyedSnapshot {
    let cfg = config(w);
    let mut items: Vec<(u64, u64)> = cfg
        .types
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (i, &t.name, format!("{:?}{:?}", t.color_tip, t.color_base)).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    let can_add = cfg.types.len() < MAX_FOLIAGE_TYPES;
    if can_add {
        items.push((u64::MAX, u64::MAX));
    }
    let types = cfg.types.clone();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| match types.get(i) {
            Some(t) => type_tile(c, f, i, t),
            None => add_tile(c, f),
        }),
    }
}

fn tile_node() -> Node {
    Node {
        // Three to a row: 3 x 31.8% plus two 5 px gaps.
        width: Val::Percent(31.8),
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(4.0),
        padding: UiRect::all(Val::Px(5.0)),
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(5.0)),
        overflow: Overflow::clip(),
        ..default()
    }
}

fn type_tile(commands: &mut Commands, fonts: &EmberFonts, index: usize, ty: &FoliageType) -> Entity {
    let tile = commands
        .spawn((
            tile_node(),
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TypeTile { index },
            Name::new(format!("foliage-type:{}", ty.name)),
        ))
        .id();
    let chosen = move |w: &Rx| active_type(w) == index;
    bind_bg(commands, tile, move |w| {
        if chosen(w) {
            rgb(accent()).with_alpha(0.22)
        } else if matches!(w.get::<UiInteraction>(tile), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)) {
            rgb(hover_bg())
        } else {
            rgb(popup_bg())
        }
    });
    bind_with(commands, tile, move |w| if chosen(w) { rgb(accent()) } else { rgb(border()) }, |world, e, c: &Color| {
        if let Some(mut b) = world.get_mut::<BorderColor>(e) {
            let want = BorderColor::all(*c);
            if *b != want {
                *b = want;
            }
        }
    });
    // The blade colours, tip over root: what the grass looks like, which is
    // what tells two tiles apart at a glance.
    let swatch = commands
        .spawn((
            Node { width: Val::Percent(100.0), height: Val::Px(40.0), flex_direction: FlexDirection::Column, border_radius: BorderRadius::all(Val::Px(3.0)), overflow: Overflow::clip(), ..default() },
            PASS_THROUGH,
        ))
        .id();
    let tip = commands.spawn((Node { width: Val::Percent(100.0), flex_grow: 1.0, ..default() }, BackgroundColor(Color::from(ty.color_tip)), PASS_THROUGH)).id();
    let base = commands.spawn((Node { width: Val::Percent(100.0), flex_grow: 1.0, ..default() }, BackgroundColor(Color::from(ty.color_base)), PASS_THROUGH)).id();
    commands.entity(swatch).add_children(&[tip, base]);
    let name = commands
        .spawn((Text::new(ty.name.clone()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_primary())), bevy::text::TextLayout::no_wrap(), PASS_THROUGH))
        .id();
    let count = commands.spawn((Text::new(""), ui_font(&fonts.mono, 10.0), TextColor(rgb(placeholder())), PASS_THROUGH)).id();
    bind_text(commands, count, move |w| fmt_blades(w.get_resource::<FoliageCounts>().map_or(0, |c| c.blades[index.min(MAX_FOLIAGE_TYPES - 1)])));
    commands.entity(tile).add_children(&[swatch, name, count]);
    tile
}

/// The last tile: add a type. Hidden at [`MAX_FOLIAGE_TYPES`], because a
/// density map has that many weights per texel and a ninth type could be named
/// but never painted.
fn add_tile(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let tile = commands
        .spawn((
            Node { min_height: Val::Px(74.0), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..tile_node() },
            BackgroundColor(Color::NONE),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            AddTypeBtn,
            Name::new("foliage-add-type"),
        ))
        .id();
    bind_bg(commands, tile, move |w| match w.get::<UiInteraction>(tile) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => Color::NONE,
    });
    let ic = icon_text(commands, &fonts.phosphor, "plus", text_muted(), 16.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let t = commands.spawn((Text::new("Add type"), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())), PASS_THROUGH)).id();
    commands.entity(tile).add_children(&[ic, t]);
    tile
}

fn properties_card(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let c = card(commands);
    let heading = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Baseline, ..default() })
        .id();
    let title = commands.spawn((Text::new(""), ui_font(&fonts.ui, 12.5), TextColor(rgb(text_primary())))).id();
    bind_text(commands, title, |w| config(w).types.get(active_type(w)).map(|t| t.name.clone()).unwrap_or_default());
    let kind = commands.spawn((Text::new("grass"), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder())))).id();
    commands.entity(heading).add_children(&[title, kind]);

    let (name_row, name_c) = row(commands, fonts, "Name");
    let name_in = text_input(commands, &fonts.ui, "Name", "");
    commands.entity(name_in).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
        align_items: AlignItems::Center,
        border: UiRect::all(Val::Px(1.0)),
        border_radius: BorderRadius::all(Val::Px(4.0)),
        ..default()
    });
    bind_text_input(
        commands,
        name_in,
        |w| config(w).types.get(active_type(w)).map(|t| t.name.clone()).unwrap_or_default(),
        |w, v| {
            let i = active_type(&Rx::new(&*w));
            set_config(w, |c| {
                if let Some(t) = c.types.get_mut(i) {
                    t.name = v;
                }
            });
        },
    );
    commands.entity(name_c).add_child(name_in);

    let density = num_row(commands, fonts, "Density", "/ m²", 1.0, 128.0, 0.5, None, type_get(|t| t.density), type_set(|t, v| t.density = v));
    let clump = num_row(commands, fonts, "Blades / clump", "", 1.0, 16.0, 0.05, Some(1.0), type_get(|t| t.blades_per_clump as f32), type_set(|t, v| t.blades_per_clump = v.round().clamp(1.0, 16.0) as u32));
    let height = pair_row(commands, fonts, "Height", "m", (0.01, 2.0, 0.01),
        (type_get(|t| t.height_range.x), type_set(|t, v| t.height_range.x = v.min(t.height_range.y))),
        (type_get(|t| t.height_range.y), type_set(|t, v| t.height_range.y = v.max(t.height_range.x))),
    );
    let width = pair_row(commands, fonts, "Width", "cm", (0.2, 50.0, 0.1),
        (type_get(|t| t.width_range.x * 100.0), type_set(|t, v| t.width_range.x = (v / 100.0).min(t.width_range.y))),
        (type_get(|t| t.width_range.y * 100.0), type_set(|t, v| t.width_range.y = (v / 100.0).max(t.width_range.x))),
    );
    let wind = num_row(commands, fonts, "Wind", "%", 0.0, 200.0, 1.0, None, type_get(|t| t.wind_strength * 100.0), type_set(|t, v| t.wind_strength = v / 100.0));

    let (en_row, en_c) = row(commands, fonts, "Enabled");
    let sw = toggle_switch(commands, true);
    bind_2way(
        commands,
        sw,
        |w| config(w).types.get(active_type(w)).is_none_or(|t| t.enabled),
        |w, v: &bool| {
            let i = active_type(&Rx::new(&*w));
            set_config(w, |c| {
                if let Some(t) = c.types.get_mut(i) {
                    t.enabled = *v;
                }
            });
        },
    );
    let sp = commands.spawn(Node { flex_grow: 1.0, ..default() }).id();
    let remove = small_button(commands, fonts, "trash", "Remove type");
    commands.entity(remove).insert(RemoveTypeBtn);
    // The last type stays: the brush always has something to paint.
    bind_display(commands, remove, |w| config(w).types.len() > 1);
    commands.entity(en_c).add_children(&[sw, sp, remove]);

    commands.entity(c).add_children(&[heading, name_row, density, clump, height, width, wind, en_row]);
    c
}

fn brush_card(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let c = card(commands);
    let (mode, mode_c) = row(commands, fonts, "Mode");
    let seg = segmented(commands, &fonts.ui, &["Paint", "Erase", "Grow", "Trim"], 0);
    bind_2way(
        commands,
        seg,
        |w| MODES.iter().position(|m| *m == settings(w).brush_type).unwrap_or(0),
        |w, i: &usize| {
            set_settings(w, |s| s.brush_type = MODES[(*i).min(3)]);
            // Picking a mode is picking up the brush: arm foliage painting.
            if let Some(mut tool) = w.get_resource_mut::<ActiveTool>() {
                if *tool != ActiveTool::FoliagePaint {
                    *tool = ActiveTool::FoliagePaint;
                }
            }
        },
    );
    commands.entity(mode_c).add_child(seg);

    let size = num_row(commands, fonts, "Size", "m", 0.25, 256.0, 0.1, None,
        move |w| radius_metres(settings(w).brush_radius, chunk_size(w, terrain)),
        move |w, v| {
            let cs = w.get::<TerrainData>(terrain).map_or(64.0, |t| t.chunk_size);
            set_settings(w, |s| s.brush_radius = radius_fraction(*v, cs));
        },
    );
    let strength = num_row(commands, fonts, "Strength", "%", 1.0, 100.0, 0.5, None,
        |w| settings(w).brush_strength * 100.0, |w, v| set_settings(w, |s| s.brush_strength = (*v / 100.0).clamp(0.01, 1.0)));
    let soft = num_row(commands, fonts, "Soft edge", "%", 0.0, 100.0, 0.5, None,
        |w| settings(w).brush_falloff * 100.0, |w, v| set_settings(w, |s| s.brush_falloff = (*v / 100.0).clamp(0.0, 1.0)));
    // Grow and Trim aim at this blade-height multiplier; Paint and Erase work
    // the density and ignore it, so it only shows for those two.
    let target = num_row(commands, fonts, "Grow to", "%", MIN_HEIGHT_SCALE * 100.0, MAX_HEIGHT_SCALE * 100.0, 1.0, None,
        |w| settings(w).brush_height * 100.0, |w, v| set_settings(w, |s| s.brush_height = *v / 100.0));
    bind_display(commands, target, |w| settings(w).brush_type.paints_height());
    let note = commands
        .spawn((Text::new("Size is the brush radius. Grow and Trim change blade height where you paint; 100% is the type's own height."), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
        .id();
    commands.entity(c).add_children(&[mode, size, strength, soft, target, note]);
    c
}

// ── Small builders ───────────────────────────────────────────────────────────
//
// The terrain editor's own builders cannot be used here (the dependency runs
// the other way), so these match their look: same card, row, label width and
// field.

fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands.spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Column, row_gap: Val::Px(gap), ..default() }).id()
}

fn card(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::all(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            BorderColor::all(rgb(border())),
        ))
        .id()
}

fn group_label(commands: &mut Commands, fonts: &EmberFonts, title: &str, note: &str) -> Entity {
    let r = commands
        .spawn(Node { width: Val::Percent(100.0), flex_direction: FlexDirection::Row, justify_content: JustifyContent::SpaceBetween, align_items: AlignItems::Baseline, ..default() })
        .id();
    let t = commands.spawn((Text::new(title.to_uppercase()), ui_font(&fonts.ui, 10.5), TextColor(rgb(text_muted())))).id();
    let n = commands.spawn((Text::new(note.to_string()), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder())))).id();
    commands.entity(r).add_children(&[t, n]);
    r
}

fn row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> (Entity, Entity) {
    let r = commands
        .spawn(Node { width: Val::Percent(100.0), min_height: Val::Px(24.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(8.0), ..default() })
        .id();
    let l = commands
        .spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_muted())), Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, ..default() }))
        .id();
    let c = commands
        .spawn(Node { flex_grow: 1.0, min_width: Val::Px(0.0), flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: Val::Px(6.0), ..default() })
        .id();
    commands.entity(r).add_children(&[l, c]);
    (r, c)
}

#[allow(clippy::too_many_arguments)]
fn field<G, S>(commands: &mut Commands, fonts: &EmberFonts, axis: &str, min: f32, max: f32, step: f32, snap: Option<f32>, get: G, set: S) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let dv = drag_value(commands, &fonts.ui, axis, value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    if let Some(s) = snap {
        commands.entity(dv).insert(DragSnap(s));
    }
    renzora_ember::inspector::fill_control(commands, dv);
    bind_2way(commands, dv, get, move |w: &mut World, v: &f32| set(w, &v.clamp(min, max)));
    dv
}

fn unit(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((Text::new(text.to_string()), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), Node { flex_shrink: 0.0, min_width: Val::Px(14.0), ..default() }))
        .id()
}

#[allow(clippy::too_many_arguments)]
fn num_row<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, unit_text: &str, min: f32, max: f32, step: f32, snap: Option<f32>, get: G, set: S) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let (r, c) = row(commands, fonts, label);
    let f = field(commands, fonts, "", min, max, step, snap, get, set);
    commands.entity(c).add_child(f);
    if !unit_text.is_empty() {
        let u = unit(commands, fonts, unit_text);
        commands.entity(c).add_child(u);
    }
    r
}

type Getter = Box<dyn for<'w> Fn(&Rx<'w>) -> f32 + Send + Sync>;
type Setter = Box<dyn Fn(&mut World, &f32) + Send + Sync>;

/// `[label] [min] [max] [unit]`.
fn pair_row(commands: &mut Commands, fonts: &EmberFonts, label: &str, unit_text: &str, (min, max, step): (f32, f32, f32), lo: (Getter, Setter), hi: (Getter, Setter)) -> Entity {
    let (r, c) = row(commands, fonts, label);
    let (lg, ls) = lo;
    let (hg, hs) = hi;
    let a = field(commands, fonts, "min", min, max, step, None, move |w| lg(w), move |w, v| ls(w, v));
    let b = field(commands, fonts, "max", min, max, step, None, move |w| hg(w), move |w, v| hs(w, v));
    let u = unit(commands, fonts, unit_text);
    commands.entity(c).add_children(&[a, b, u]);
    r
}

fn small_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(5.0),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("foliage-btn:{label}")),
        ))
        .id();
    bind_bg(commands, b, move |w| match w.get::<UiInteraction>(b) {
        Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed) => rgb(hover_bg()),
        _ => rgb(popup_bg()),
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    commands.entity(ic).insert(PASS_THROUGH);
    let t = commands.spawn((Text::new(label.to_string()), ui_font(&fonts.ui, 11.5), TextColor(rgb(text_primary())), PASS_THROUGH)).id();
    commands.entity(b).add_children(&[ic, t]);
    b
}

/// A getter for a field of the chosen type.
fn type_get<F>(f: F) -> Getter
where
    F: Fn(&FoliageType) -> f32 + Send + Sync + 'static,
{
    Box::new(move |w| config(w).types.get(active_type(w)).map(&f).unwrap_or(0.0))
}

/// A setter for a field of the chosen type.
fn type_set<F>(f: F) -> Setter
where
    F: Fn(&mut FoliageType, f32) + Send + Sync + 'static,
{
    Box::new(move |w, v| {
        let i = active_type(&Rx::new(&*w));
        set_config(w, |c| {
            if let Some(t) = c.types.get_mut(i) {
                f(t, *v);
            }
        });
    })
}

// ── Systems ──────────────────────────────────────────────────────────────────

/// Picking a type also picks up the foliage brush.
fn foliage_tile_click(
    q: Query<(&UiInteraction, &TypeTile), Changed<UiInteraction>>,
    mut settings: Option<ResMut<FoliagePaintSettings>>,
    mut active: Option<ResMut<ActiveTool>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (i, tile) in &q {
        if *i == UiInteraction::Pressed {
            settings.active_type = tile.index;
            if let Some(active) = active.as_mut() {
                if **active != ActiveTool::FoliagePaint {
                    **active = ActiveTool::FoliagePaint;
                }
            }
        }
    }
}

fn foliage_add_type(
    q: Query<&UiInteraction, (With<AddTypeBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
    config: Option<Res<FoliageConfig>>,
) {
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let mut cfg = config.map(|c| c.clone()).unwrap_or_default();
    if cfg.types.len() >= MAX_FOLIAGE_TYPES {
        return;
    }
    cfg.types.push(FoliageType::default());
    let new = cfg.types.len() - 1;
    commands.insert_resource(cfg);
    commands.queue(move |w: &mut World| set_settings(w, |s| s.active_type = new));
}

fn foliage_remove_type(
    q: Query<&UiInteraction, (With<RemoveTypeBtn>, Changed<UiInteraction>)>,
    mut commands: Commands,
    config: Option<Res<FoliageConfig>>,
    settings: Option<Res<FoliagePaintSettings>>,
) {
    if !q.iter().any(|i| *i == UiInteraction::Pressed) {
        return;
    }
    let mut cfg = config.map(|c| c.clone()).unwrap_or_default();
    if cfg.types.len() <= 1 {
        return;
    }
    let idx = settings.map(|s| s.active_type).unwrap_or(0).min(cfg.types.len() - 1);
    cfg.types.remove(idx);
    let active = idx.min(cfg.types.len() - 1);
    commands.insert_resource(cfg);
    commands.queue(move |w: &mut World| set_settings(w, |s| s.active_type = active));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The brush stores a fraction of a chunk side; the panel speaks metres.
    #[test]
    fn brush_size_round_trips_through_metres() {
        for (f, cs) in [(0.1, 64.0), (0.25, 128.0), (0.01, 32.0)] {
            let m = radius_metres(f, cs);
            assert!((radius_fraction(m, cs) - f).abs() < 1e-6);
        }
        assert_eq!(radius_fraction(1e6, 64.0), RADIUS_FRACTION.1);
        assert_eq!(radius_fraction(0.0, 64.0), RADIUS_FRACTION.0);
    }

    #[test]
    fn a_fully_painted_chunk_counts_density_times_area() {
        let ty = FoliageType { density: 10.0, blades_per_clump: 2, ..default() };
        assert_eq!(estimate_blades(4.0, 4, 100.0, &ty), 2_000);
        assert_eq!(estimate_blades(0.0, 4, 100.0, &ty), 0);
        assert_eq!(estimate_blades(1.0, 0, 100.0, &ty), 0);
        assert_eq!(fmt_blades(1_200_000), "1.2 M blades");
        assert_eq!(fmt_blades(0), "not painted");
    }

    #[test]
    fn every_brush_mode_is_offered() {
        for m in [FoliageBrushType::Paint, FoliageBrushType::Erase, FoliageBrushType::Grow, FoliageBrushType::Trim] {
            assert!(MODES.contains(&m));
        }
    }
}
