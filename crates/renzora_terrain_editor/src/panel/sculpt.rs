//! The Sculpt mode: the brushes in two labelled groups, one card with only the
//! chosen brush's own settings, and one Brush card (size in metres, falloff
//! curve, shape) that every sculpt brush shares.
//!
//! The per-brush card is built once with every setting any brush has, and each
//! row shows only while the chosen brush reads it ([`tool_settings`]). The
//! table is the contract: a row appears exactly when the brush code in
//! `renzora_terrain::sculpt` reads the field behind it, so nothing on screen is
//! a control that does nothing. Sculpt brushes have no slope or height mask in
//! the brush code, so the Brush card has none (Paint does).

use bevy::prelude::*;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::UiInteraction;

use renzora_terrain::data::{
    BrushShape, FlattenMode, NoiseMode, StampBlendMode, StampBrushData, TerrainBrushType, TerrainData, TerrainSettings,
};

use super::falloff::falloff_card_rows;
use super::widgets::{
    button, button_parts, card, column, dropdown_row, group_label, hint, live_text, num_field, num_row, row,
    segmented_row, unit,
};
use super::{brush_type, set_settings, HeightPickBtn, SculptToolBtn, StampLoadBtn, StampPresetCombo};
use crate::systems::HeightPick;

/// The Shape group: brushes that move ground into a form you choose.
pub(crate) const SHAPE_TOOLS: &[(TerrainBrushType, &str, &str)] = &[
    (TerrainBrushType::Sculpt, "mountains", "Sculpt"),
    (TerrainBrushType::Raise, "arrow-up", "Raise"),
    (TerrainBrushType::Lower, "arrow-down", "Lower"),
    (TerrainBrushType::Flatten, "equals", "Flatten"),
    (TerrainBrushType::SetHeight, "ruler", "Set H"),
    (TerrainBrushType::Ramp, "arrow-up-right", "Ramp"),
    (TerrainBrushType::Smooth, "waves", "Smooth"),
    (TerrainBrushType::Relax, "activity", "Relax"),
    (TerrainBrushType::Pinch, "arrows-in-cardinal", "Pinch"),
    (TerrainBrushType::Stamp, "stamp", "Stamp"),
    (TerrainBrushType::Erase, "eraser", "Erase"),
    (TerrainBrushType::Retop, "graph", "Retop"),
];

/// The Weather and detail group: brushes that add what nature would.
pub(crate) const DETAIL_TOOLS: &[(TerrainBrushType, &str, &str)] = &[
    (TerrainBrushType::Noise, "waveform", "Noise"),
    (TerrainBrushType::Erosion, "wind", "Erode"),
    (TerrainBrushType::Hydro, "cloud-rain", "Rain"),
    (TerrainBrushType::Terrace, "stairs", "Terrace"),
    (TerrainBrushType::Cliff, "chart-bar", "Cliff"),
];

/// One setting on the per-brush card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolSetting {
    Strength,
    /// Flatten's Both / Raise / Lower.
    Affects,
    /// Set Height's target, in metres, with Pick from ground.
    TargetHeight,
    NoiseMode,
    NoiseScale,
    NoiseDetail,
    NoiseRoughness,
    NoiseLacunarity,
    NoiseSeed,
    /// Only for the noise modes that read it; see [`NoiseMode::uses_warp`].
    NoiseWarp,
    TerraceStep,
    TerraceSharpness,
    StampShape,
    StampBlend,
    StampRotation,
    StampHeight,
}

/// What the chosen brush reads, in the order the card lists it. Mirrors the
/// fields `renzora_terrain::sculpt` reads per brush.
pub(crate) fn tool_settings(brush: TerrainBrushType) -> &'static [ToolSetting] {
    use ToolSetting::*;
    match brush {
        TerrainBrushType::Flatten => &[Strength, Affects],
        TerrainBrushType::SetHeight => &[Strength, TargetHeight],
        TerrainBrushType::Noise => &[
            Strength,
            NoiseMode,
            NoiseScale,
            NoiseDetail,
            NoiseRoughness,
            NoiseLacunarity,
            NoiseWarp,
            NoiseSeed,
        ],
        TerrainBrushType::Terrace => &[Strength, TerraceStep, TerraceSharpness],
        TerrainBrushType::Stamp => &[StampShape, StampBlend, Strength, StampRotation, StampHeight],
        TerrainBrushType::Raise
        | TerrainBrushType::Lower
        | TerrainBrushType::Sculpt
        | TerrainBrushType::Smooth
        | TerrainBrushType::Relax
        | TerrainBrushType::Ramp
        | TerrainBrushType::Pinch
        | TerrainBrushType::Erase
        | TerrainBrushType::Retop
        | TerrainBrushType::Erosion
        | TerrainBrushType::Hydro
        | TerrainBrushType::Cliff => &[Strength],
    }
}

/// One sentence on what the brush does, and what Shift does to it.
pub(crate) fn tool_hint(brush: TerrainBrushType) -> &'static str {
    match brush {
        TerrainBrushType::Sculpt => "Raises the ground. Hold Shift to lower it.",
        TerrainBrushType::Raise => "Raises the ground under the brush.",
        TerrainBrushType::Lower => "Lowers the ground under the brush.",
        TerrainBrushType::Flatten => "Levels the ground to the height where the stroke starts.",
        TerrainBrushType::SetHeight => "Brings the ground to the target height.",
        TerrainBrushType::Ramp => "Slopes from the stroke's start height out to the brush edge. Shift reverses it.",
        TerrainBrushType::Smooth => "Rounds off bumps and ridges.",
        TerrainBrushType::Relax => "Removes small bumps without shrinking the hills and valleys around them.",
        TerrainBrushType::Pinch => "Sharpens ridges and creases. Shift softens them.",
        TerrainBrushType::Stamp => "Click to stamp the shape once. The brush size sets its footprint.",
        TerrainBrushType::Erase => "Pulls the ground back toward the level a new terrain starts at.",
        TerrainBrushType::Retop => "A strong, wide smooth for reshaping a whole area.",
        TerrainBrushType::Noise => "Adds natural variation. Shift smooths instead.",
        TerrainBrushType::Erosion => "Weathers slopes so loose ground slides downhill.",
        TerrainBrushType::Hydro => "Rain that carves gullies and carries sediment downhill.",
        TerrainBrushType::Terrace => "Cuts the slope into level steps.",
        TerrainBrushType::Cliff => "Builds rock ledges and outcrops. Shift smooths them.",
    }
}

/// The brush's name as the card heading shows it.
pub(crate) fn tool_name(brush: TerrainBrushType) -> &'static str {
    SHAPE_TOOLS
        .iter()
        .chain(DETAIL_TOOLS)
        .find(|(b, _, _)| *b == brush)
        .map(|(_, _, l)| *l)
        .filter(|l| *l != "Set H")
        .unwrap_or(brush.display_name())
}

fn shows(w: &Rx, setting: ToolSetting) -> bool {
    tool_settings(brush_type(w)).contains(&setting)
}

fn ts<T: Default>(w: &Rx, f: impl Fn(&TerrainSettings) -> T) -> T {
    w.get_resource::<TerrainSettings>().map(f).unwrap_or_default()
}

pub(super) fn sculpt_content(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let root = column(commands, 10.0);

    let shape = tool_group(commands, fonts, "Shape", "scroll wheel resizes", SHAPE_TOOLS);
    let detail = tool_group(commands, fonts, "Weather and detail", "", DETAIL_TOOLS);
    let tool = tool_card(commands, fonts, terrain);

    let brush_col = column(commands, 6.0);
    let brush_label = group_label(commands, fonts, "Brush", "shared by every sculpt brush");
    let brush = brush_card(commands, fonts);
    commands.entity(brush_col).add_children(&[brush_label, brush]);

    commands.entity(root).add_children(&[shape, detail, tool, brush_col]);
    root
}

fn tool_group(
    commands: &mut Commands,
    fonts: &EmberFonts,
    title: &str,
    note: &str,
    tools: &[(TerrainBrushType, &str, &str)],
) -> Entity {
    let col = column(commands, 6.0);
    let label = group_label(commands, fonts, title, note);
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            column_gap: Val::Px(4.0),
            row_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let kids: Vec<Entity> = tools.iter().map(|&(b, icon, label)| tool_button(commands, fonts, b, icon, label)).collect();
    commands.entity(grid).add_children(&kids);
    commands.entity(col).add_children(&[label, grid]);
    col
}

/// A compact icon over a label, six to a row.
fn tool_button(commands: &mut Commands, fonts: &EmberFonts, brush: TerrainBrushType, icon: &str, label: &str) -> Entity {
    let btn = commands
        .spawn((
            Node {
                // Six to a row: 6 x 15.4% plus five 4 px gaps fills the width.
                width: Val::Percent(15.4),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(3.0),
                padding: UiRect::vertical(Val::Px(6.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(Color::NONE),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            SculptToolBtn { brush },
            Name::new(format!("terrain-tool:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| super::widgets::choice_bg(w, btn, brush_type(w) == brush));
    super::widgets::bind_choice_border(commands, btn, move |w| brush_type(w) == brush);
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 16.0);
    commands.entity(ic).insert(PASS_THROUGH);
    super::widgets::bind_choice_text(commands, ic, move |w| brush_type(w) == brush);
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            PASS_THROUGH,
        ))
        .id();
    super::widgets::bind_choice_text(commands, lbl, move |w| brush_type(w) == brush);
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

// ── The chosen brush's card ──────────────────────────────────────────────────

fn tool_card(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let c = card(commands);
    let heading = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Baseline,
            ..default()
        })
        .id();
    let name = live_text(commands, fonts, 12.5, text_primary(), |w| tool_name(brush_type(w)).to_string());
    let kind = live_text(commands, fonts, 10.5, placeholder(), |_| "brush".to_string());
    commands.entity(heading).add_children(&[name, kind]);
    let about = live_text(commands, fonts, 10.5, placeholder(), |w| tool_hint(brush_type(w)).to_string());

    let mut rows = vec![heading, about];
    for setting in [
        ToolSetting::StampShape,
        ToolSetting::StampBlend,
        ToolSetting::Strength,
        ToolSetting::Affects,
        ToolSetting::TargetHeight,
        ToolSetting::NoiseMode,
        ToolSetting::NoiseScale,
        ToolSetting::NoiseDetail,
        ToolSetting::NoiseRoughness,
        ToolSetting::NoiseLacunarity,
        ToolSetting::NoiseWarp,
        ToolSetting::NoiseSeed,
        ToolSetting::TerraceStep,
        ToolSetting::TerraceSharpness,
        ToolSetting::StampRotation,
        ToolSetting::StampHeight,
    ] {
        let r = setting_row(commands, fonts, setting, terrain);
        bind_display(commands, r, move |w| {
            shows(w, setting) && (setting != ToolSetting::NoiseWarp || ts(w, |s| s.noise_mode).uses_warp())
        });
        rows.push(r);
    }
    commands.entity(c).add_children(&rows);
    c
}

fn setting_row(commands: &mut Commands, fonts: &EmberFonts, setting: ToolSetting, terrain: Entity) -> Entity {
    match setting {
        ToolSetting::Strength => num_row(commands, fonts, "Strength", "%", 1.0, 100.0, 0.5,
            |w| ts(w, |s| s.brush_strength) * 100.0,
            |w, v| set_settings(w, |s| s.brush_strength = (*v / 100.0).clamp(0.01, 1.0)),
        ),
        ToolSetting::Affects => segmented_row(commands, fonts, "Affects", &["Both", "Raise", "Lower"],
            |w| match ts(w, |s| s.flatten_mode) {
                FlattenMode::Both => 0,
                FlattenMode::Raise => 1,
                FlattenMode::Lower => 2,
            },
            |w, i| set_settings(w, |s| s.flatten_mode = [FlattenMode::Both, FlattenMode::Raise, FlattenMode::Lower][(*i).min(2)]),
        ),
        ToolSetting::TargetHeight => target_height_row(commands, fonts, terrain),
        ToolSetting::NoiseMode => {
            let names: Vec<&str> = NoiseMode::all().iter().map(|m| m.display_name()).collect();
            dropdown_row(commands, fonts, "Pattern", &names,
                |w| { let m = ts(w, |s| s.noise_mode); NoiseMode::all().iter().position(|x| *x == m).unwrap_or(0) },
                |w, i| set_settings(w, |s| s.noise_mode = NoiseMode::all().get(*i).copied().unwrap_or_default()),
            )
        }
        ToolSetting::NoiseScale => num_row(commands, fonts, "Feature size", "m", 1.0, 500.0, 0.5,
            |w| ts(w, |s| s.noise_scale), |w, v| set_settings(w, |s| s.noise_scale = *v)),
        ToolSetting::NoiseDetail => {
            let (r, content) = row(commands, fonts, "Detail");
            let f = num_field(commands, fonts, "", 1.0, 8.0, 0.05, Some(1.0),
                |w| ts(w, |s| s.noise_octaves as f32),
                |w, v| set_settings(w, |s| s.noise_octaves = v.round().clamp(1.0, 8.0) as u32));
            let u = unit(commands, fonts, "layers");
            commands.entity(content).add_children(&[f, u]);
            r
        }
        ToolSetting::NoiseRoughness => num_row(commands, fonts, "Roughness", "%", 10.0, 90.0, 0.5,
            |w| ts(w, |s| s.noise_persistence) * 100.0, |w, v| set_settings(w, |s| s.noise_persistence = *v / 100.0)),
        ToolSetting::NoiseLacunarity => num_row(commands, fonts, "Lacunarity", "x", 1.0, 4.0, 0.02,
            |w| ts(w, |s| s.noise_lacunarity), |w, v| set_settings(w, |s| s.noise_lacunarity = *v)),
        ToolSetting::NoiseWarp => num_row(commands, fonts, "Warp", "", 0.0, 5.0, 0.02,
            |w| ts(w, |s| s.warp_strength), |w, v| set_settings(w, |s| s.warp_strength = *v)),
        ToolSetting::NoiseSeed => {
            let (r, content) = row(commands, fonts, "Seed");
            let f = num_field(commands, fonts, "", 0.0, 0.0, 1.0, Some(1.0),
                |w| ts(w, |s| s.noise_seed as f32),
                |w, v| set_settings(w, |s| s.noise_seed = v.max(0.0).round() as u32));
            commands.entity(content).add_child(f);
            r
        }
        ToolSetting::TerraceStep => num_row(commands, fonts, "Step height", "m", 0.25, 100.0, 0.1,
            |w| ts(w, |s| s.terrace_height), |w, v| set_settings(w, |s| s.terrace_height = *v)),
        ToolSetting::TerraceSharpness => num_row(commands, fonts, "Sharpness", "%", 0.0, 100.0, 0.5,
            |w| ts(w, |s| s.terrace_sharpness) * 100.0, |w, v| set_settings(w, |s| s.terrace_sharpness = *v / 100.0)),
        ToolSetting::StampShape => {
            let (r, content) = row(commands, fonts, "Stamp");
            let (pick, label) = button_parts(commands, fonts, Some("caret-down"), "Choose", false);
            commands.entity(pick).insert(StampPresetCombo);
            bind_text(commands, label, |w| {
                w.get_resource::<StampBrushData>().filter(|s| s.is_loaded()).map(|s| s.name.clone()).unwrap_or_else(|| "Choose".into())
            });
            let load = button(commands, fonts, Some("image"), "Load PNG", false);
            commands.entity(load).insert(StampLoadBtn);
            commands.entity(content).add_children(&[pick, load]);
            r
        }
        ToolSetting::StampBlend => {
            let names: Vec<&str> = StampBlendMode::all().iter().map(|m| m.display_name()).collect();
            dropdown_row(commands, fonts, "Blend", &names,
                |w| { let m = ts(w, |s| s.stamp_blend_mode); StampBlendMode::all().iter().position(|x| *x == m).unwrap_or(0) },
                |w, i| set_settings(w, |s| s.stamp_blend_mode = StampBlendMode::all().get(*i).copied().unwrap_or_default()),
            )
        }
        // Stored in radians (apply_stamp feeds it to cos/sin); shown in degrees.
        ToolSetting::StampRotation => num_row(commands, fonts, "Rotation", "°", 0.0, 360.0, 1.0,
            |w| ts(w, |s| s.stamp_rotation.to_degrees()), |w, v| set_settings(w, |s| s.stamp_rotation = v.to_radians())),
        ToolSetting::StampHeight => num_row(commands, fonts, "Height", "%", 0.0, 200.0, 1.0,
            |w| ts(w, |s| s.stamp_height_scale) * 100.0, |w, v| set_settings(w, |s| s.stamp_height_scale = *v / 100.0)),
    }
}

/// Set Height's target in the terrain's own metres. The setting is stored as a
/// fraction of the terrain's height range, which is what the brush reads, but
/// a fraction is not a height anyone can aim for.
fn target_height_row(commands: &mut Commands, fonts: &EmberFonts, terrain: Entity) -> Entity {
    let (r, content) = row(commands, fonts, "Target");
    let range = move |w: &Rx| w.get::<TerrainData>(terrain).map(|t| (t.min_height, t.max_height)).unwrap_or((0.0, 1.0));
    let f = num_field(commands, fonts, "", 0.0, 0.0, 0.1, None,
        move |w| { let (lo, hi) = range(w); lo + ts(w, |s| s.target_height) * (hi - lo) },
        move |w, v| {
            let (lo, hi) = w.get::<TerrainData>(terrain).map(|t| (t.min_height, t.max_height)).unwrap_or((0.0, 1.0));
            let t = ((*v - lo) / (hi - lo).max(1e-3)).clamp(0.0, 1.0);
            set_settings(w, |s| s.target_height = t);
        },
    );
    let u = unit(commands, fonts, "m");
    let (pick, label) = button_parts(commands, fonts, Some("eyedropper"), "Pick", false);
    commands.entity(pick).insert(HeightPickBtn);
    bind_text(commands, label, |w| {
        if w.get_resource::<HeightPick>().is_some_and(|p| p.armed) { "Click ground".into() } else { "Pick".into() }
    });
    commands.entity(content).add_children(&[f, u, pick]);
    r
}

// ── Brush card ───────────────────────────────────────────────────────────────

fn brush_card(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let c = card(commands);
    let size = num_row(commands, fonts, "Size", "m", 1.0, 200.0, 0.5,
        |w| ts(w, |s| s.brush_radius), |w, v| set_settings(w, |s| s.brush_radius = *v));
    let falloff = falloff_card_rows(commands, fonts, super::falloff::FalloffTarget::Sculpt);
    let shape = segmented_row(commands, fonts, "Shape", &["Circle", "Square", "Diamond"],
        |w| shape_index(ts(w, |s| s.brush_shape)),
        |w, i| set_settings(w, |s| s.brush_shape = shape_at(*i)),
    );
    let note = hint(commands, fonts, "Size is the brush radius. The scroll wheel over the viewport changes it.");
    let mut kids = vec![size];
    kids.extend(falloff);
    kids.extend([shape, note]);
    commands.entity(c).add_children(&kids);
    c
}

pub(super) fn shape_index(s: BrushShape) -> usize {
    match s {
        BrushShape::Circle => 0,
        BrushShape::Square => 1,
        BrushShape::Diamond => 2,
    }
}

pub(super) fn shape_at(i: usize) -> BrushShape {
    match i {
        1 => BrushShape::Square,
        2 => BrushShape::Diamond,
        _ => BrushShape::Circle,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every brush is in exactly one of the two groups: one missing would be a
    /// brush you cannot pick, one listed twice a button that lies about which
    /// group it is in.
    #[test]
    fn every_brush_is_in_one_group() {
        for b in TerrainBrushType::all() {
            let n = SHAPE_TOOLS.iter().chain(DETAIL_TOOLS).filter(|(x, _, _)| x == b).count();
            assert_eq!(n, 1, "{b:?}");
        }
        assert!(SHAPE_TOOLS.len() <= 12 && DETAIL_TOOLS.len() <= 6, "six to a row, two rows at most");
    }

    #[test]
    fn every_icon_resolves() {
        for (_, icon, _) in SHAPE_TOOLS.iter().chain(DETAIL_TOOLS) {
            assert!(renzora_ember::font::icon_glyph(icon).is_some(), "unknown Phosphor icon {icon:?}");
        }
    }

    /// The card shows what the brush code reads and nothing else: Flatten
    /// takes its target from where the stroke starts, so it has no target
    /// field; Set Height has one; only Noise shows noise settings.
    #[test]
    fn each_brush_shows_only_what_it_reads() {
        use ToolSetting::*;
        let flatten = tool_settings(TerrainBrushType::Flatten);
        assert!(flatten.contains(&Affects) && !flatten.contains(&TargetHeight));
        assert!(tool_settings(TerrainBrushType::SetHeight).contains(&TargetHeight));
        assert!(!tool_settings(TerrainBrushType::SetHeight).contains(&Affects));
        assert!(tool_settings(TerrainBrushType::Terrace).contains(&TerraceStep));
        assert!(tool_settings(TerrainBrushType::Stamp).contains(&StampShape));
        for b in TerrainBrushType::all() {
            let s = tool_settings(*b);
            assert!(s.contains(&Strength), "{b:?} reads its strength");
            if *b != TerrainBrushType::Noise {
                assert!(!s.contains(&NoiseScale), "{b:?} shows noise settings it never reads");
            }
            if *b != TerrainBrushType::Stamp {
                assert!(!s.contains(&StampBlend), "{b:?}");
            }
            assert!(!tool_hint(*b).is_empty());
        }
    }

    #[test]
    fn shapes_round_trip_through_their_index() {
        for s in [BrushShape::Circle, BrushShape::Square, BrushShape::Diamond] {
            assert_eq!(shape_at(shape_index(s)), s);
        }
    }

    #[test]
    fn set_height_is_named_in_full_on_the_card() {
        assert_eq!(tool_name(TerrainBrushType::SetHeight), "Set Height");
        assert_eq!(tool_name(TerrainBrushType::Hydro), "Rain");
    }
}
