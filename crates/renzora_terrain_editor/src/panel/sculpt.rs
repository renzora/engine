//! The Sculpt tab: the 17-brush tool grid, the per-brush Tool Settings
//! (Flatten / Noise / Terrace / Stamp), Brush Settings and Heightmap Import.

use bevy::prelude::*;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text_color};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::{
    BrushFalloffType, BrushShape, FlattenMode, NoiseMode, StampBlendMode, StampBrushData,
    TerrainBrushType, TerrainSettings,
};

use super::widgets::{
    caption, enum_combo, falloff_type_button, field_row, labelled_drag, labelled_slider,
    section_col, shape_button, wide_button,
};
use super::{
    brush_type, set_settings, FlattenModeCombo, HeightmapExportBtn, HeightmapImportBtn,
    NoiseModeCombo, SculptToolBtn, ShapeTarget, StampBlendCombo, StampLoadBtn, StampPresetCombo,
};

const SCULPT_TOOLS: &[(TerrainBrushType, &str, &str)] = &[
    (TerrainBrushType::Sculpt, "mountains", "Sculpt"),
    (TerrainBrushType::Smooth, "waves", "Smooth"),
    (TerrainBrushType::Flatten, "equals", "Flatten"),
    (TerrainBrushType::Ramp, "arrow-fat-line-up", "Ramp"),
    (TerrainBrushType::Erosion, "tree", "Erosion"),
    (TerrainBrushType::Hydro, "drop", "Hydro"),
    (TerrainBrushType::Noise, "waveform", "Noise"),
    (TerrainBrushType::Terrace, "stairs", "Terrace"),
    (TerrainBrushType::Pinch, "arrows-in-cardinal", "Pinch"),
    (TerrainBrushType::Relax, "activity", "Relax"),
    (TerrainBrushType::Retop, "graph", "Retop"),
    (TerrainBrushType::Cliff, "chart-bar", "Cliff"),
    (TerrainBrushType::Raise, "arrows-out-cardinal", "Raise"),
    (TerrainBrushType::Lower, "arrows-out-cardinal", "Lower"),
    (TerrainBrushType::SetHeight, "equals", "Set H"),
    (TerrainBrushType::Erase, "eraser", "Erase"),
    (TerrainBrushType::Stamp, "stamp", "Stamp"),
];

pub(super) fn sculpt_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    // Tool grid (4 columns).
    let grid = sculpt_tool_grid(commands, fonts);

    // Tool Settings section.
    let (tool_sec, tool_body) = collapsible(commands, fonts, None, "Tool Settings", true);
    tool_settings(commands, fonts, tool_body);

    // Brush Settings section.
    let (brush_sec, brush_body) = collapsible(commands, fonts, None, "Brush Settings", true);
    brush_settings(commands, fonts, brush_body);

    // Heightmap Import section.
    let (hm_sec, hm_body) = collapsible(commands, fonts, None, "Heightmap Import", false);
    heightmap_section(commands, fonts, hm_body);

    commands
        .entity(root)
        .add_children(&[grid, tool_sec, brush_sec, hm_sec]);
    root
}

fn sculpt_tool_grid(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
    let mut kids = Vec::with_capacity(SCULPT_TOOLS.len());
    for &(bt, icon, label) in SCULPT_TOOLS {
        kids.push(sculpt_tool_button(commands, fonts, bt, icon, label));
    }
    commands.entity(grid).add_children(&kids);
    grid
}

fn sculpt_tool_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    brush: TerrainBrushType,
    icon: &str,
    label: &str,
) -> Entity {
    // 4 per row: each cell is ~23% wide so 4 fit with the 4px gaps.
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(23.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(2.0),
                padding: UiRect::vertical(Val::Px(6.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            SculptToolBtn { brush },
            Name::new(format!("terrain-tool:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if brush_type(w) == brush {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(btn),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(popup_bg())
        } else {
            rgb(card_bg())
        }
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 20.0);
    bind_text_color(commands, ic, move |w| {
        if brush_type(w) == brush {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text_color(commands, lbl, move |w| {
        if brush_type(w) == brush {
            rgb(text_primary())
        } else {
            rgb(text_muted())
        }
    });
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

// ── Tool Settings ────────────────────────────────────────────────────────────

fn tool_settings(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    // Strength (always present).
    let strength = labelled_slider(
        commands,
        fonts,
        "Strength",
        0.01,
        1.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.brush_strength).unwrap_or(0.5),
        |w, v| set_settings(w, |s| s.brush_strength = *v),
    );
    commands.entity(body).add_child(strength);

    // Flatten-specific (shown only for the Flatten brush).
    let flatten = flatten_settings(commands, fonts);
    bind_display(commands, flatten, |w| {
        brush_type(w) == TerrainBrushType::Flatten
    });
    commands.entity(body).add_child(flatten);

    // Noise-specific.
    let noise = noise_settings(commands, fonts);
    bind_display(commands, noise, |w| brush_type(w) == TerrainBrushType::Noise);
    commands.entity(body).add_child(noise);

    // Terrace-specific.
    let terrace = terrace_settings(commands, fonts);
    bind_display(commands, terrace, |w| {
        brush_type(w) == TerrainBrushType::Terrace
    });
    commands.entity(body).add_child(terrace);

    // Stamp-specific (the heightmap-stamp brush: procedural preset or PNG).
    let stamp = stamp_settings(commands, fonts);
    bind_display(commands, stamp, |w| brush_type(w) == TerrainBrushType::Stamp);
    commands.entity(body).add_child(stamp);
}

fn flatten_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = section_col(commands);

    // Mode combo.
    let mode_row = field_row(commands, fonts, "Mode");
    let combo = enum_combo(commands, fonts, FlattenModeCombo, |w| {
        flatten_mode_label(w.get_resource::<TerrainSettings>().map(|s| s.flatten_mode).unwrap_or_default())
    });
    commands.entity(mode_row).add_child(combo);

    // Target Height drag.
    let target = labelled_drag(
        commands,
        fonts,
        "Target Height",
        0.0,
        1.0,
        0.005,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.target_height).unwrap_or(0.5),
        |w, v| set_settings(w, |s| s.target_height = *v),
    );

    commands.entity(col).add_children(&[mode_row, target]);
    col
}

pub(super) fn flatten_mode_label(m: FlattenMode) -> String {
    match m {
        FlattenMode::Both => "Both",
        FlattenMode::Raise => "Raise",
        FlattenMode::Lower => "Lower",
    }
    .to_string()
}

fn noise_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = section_col(commands);

    let title = caption(commands, fonts, "Noise", text_primary());

    // Mode combo.
    let mode_row = field_row(commands, fonts, "Mode");
    let combo = enum_combo(commands, fonts, NoiseModeCombo, |w| {
        w.get_resource::<TerrainSettings>()
            .map(|s| s.noise_mode.display_name().to_string())
            .unwrap_or_default()
    });
    commands.entity(mode_row).add_child(combo);

    let scale = labelled_drag(
        commands, fonts, "Scale", 1.0, 500.0, 0.5,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.noise_scale).unwrap_or(30.0),
        |w, v| set_settings(w, |s| s.noise_scale = *v),
    );
    let octaves = labelled_drag(
        commands, fonts, "Octaves", 1.0, 8.0, 0.1,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.noise_octaves as f32).unwrap_or(5.0),
        |w, v| set_settings(w, |s| s.noise_octaves = v.round().clamp(1.0, 8.0) as u32),
    );
    let lac = labelled_drag(
        commands, fonts, "Lacunarity", 1.0, 4.0, 0.05,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.noise_lacunarity).unwrap_or(2.0),
        |w, v| set_settings(w, |s| s.noise_lacunarity = *v),
    );
    let pers = labelled_drag(
        commands, fonts, "Persistence", 0.1, 0.9, 0.01,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.noise_persistence).unwrap_or(0.5),
        |w, v| set_settings(w, |s| s.noise_persistence = *v),
    );
    let seed = labelled_drag(
        commands, fonts, "Seed", 0.0, 0.0, 1.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.noise_seed as f32).unwrap_or(42.0),
        |w, v| set_settings(w, |s| s.noise_seed = v.max(0.0).round() as u32),
    );

    // Warp (only meaningful for the Warped mode).
    let warp = labelled_slider(
        commands, fonts, "Warp", 0.0, 5.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.warp_strength).unwrap_or(0.5),
        |w, v| set_settings(w, |s| s.warp_strength = *v),
    );
    bind_display(commands, warp, |w| {
        w.get_resource::<TerrainSettings>().map(|s| s.noise_mode).unwrap_or_default() == NoiseMode::Warped
    });

    commands
        .entity(col)
        .add_children(&[title, mode_row, scale, octaves, lac, pers, seed, warp]);
    col
}

fn terrace_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = section_col(commands);
    let title = caption(commands, fonts, "Terrace", text_primary());
    let steps = labelled_drag(
        commands, fonts, "Steps", 2.0, 32.0, 0.1,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.terrace_steps as f32).unwrap_or(8.0),
        |w, v| set_settings(w, |s| s.terrace_steps = v.round().clamp(2.0, 32.0) as u32),
    );
    let sharp = labelled_slider(
        commands, fonts, "Sharpness", 0.0, 1.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.terrace_sharpness).unwrap_or(0.8),
        |w, v| set_settings(w, |s| s.terrace_sharpness = *v),
    );
    commands.entity(col).add_children(&[title, steps, sharp]);
    col
}

fn stamp_settings(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let col = section_col(commands);
    let title = caption(commands, fonts, "Stamp", text_primary());

    // Source: procedural preset combo (shows the loaded stamp's name, which
    // is also the PNG filename after a Load) + a Load-PNG button.
    let src_row = field_row(commands, fonts, "Shape");
    let preset = enum_combo(commands, fonts, StampPresetCombo, |w| {
        w.get_resource::<StampBrushData>()
            .filter(|s| s.is_loaded())
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "Choose...".to_string())
    });
    commands.entity(src_row).add_child(preset);

    let load = wide_button(commands, fonts, Some("image"), "Load PNG...", text_muted());
    commands.entity(load).insert(StampLoadBtn);

    // Blend mode.
    let blend_row = field_row(commands, fonts, "Blend");
    let blend = enum_combo(commands, fonts, StampBlendCombo, |w| {
        stamp_blend_label(
            w.get_resource::<TerrainSettings>()
                .map(|s| s.stamp_blend_mode)
                .unwrap_or_default(),
        )
    });
    commands.entity(blend_row).add_child(blend);

    // Rotation is stored in radians (apply_stamp feeds it to cos/sin);
    // the field speaks degrees.
    let rotation = labelled_drag(
        commands, fonts, "Rotation", 0.0, 360.0, 1.0,
        |w| {
            w.get_resource::<TerrainSettings>()
                .map(|s| s.stamp_rotation.to_degrees())
                .unwrap_or(0.0)
        },
        |w, v| set_settings(w, |s| s.stamp_rotation = v.to_radians()),
    );
    let height_scale = labelled_slider(
        commands, fonts, "Height Scale", 0.0, 2.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.stamp_height_scale).unwrap_or(1.0),
        |w, v| set_settings(w, |s| s.stamp_height_scale = *v),
    );

    let hint = commands
        .spawn((
            Text::new("Click to stamp once. Brush size sets the footprint."),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
        ))
        .id();

    commands
        .entity(col)
        .add_children(&[title, src_row, load, blend_row, rotation, height_scale, hint]);
    col
}

pub(super) fn stamp_blend_label(m: StampBlendMode) -> String {
    match m {
        StampBlendMode::Add => "Add",
        StampBlendMode::Subtract => "Subtract",
        StampBlendMode::Replace => "Replace",
        StampBlendMode::Max => "Max",
        StampBlendMode::Min => "Min",
    }
    .to_string()
}

// ── Brush Settings ───────────────────────────────────────────────────────────

fn brush_settings(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let size = labelled_slider(
        commands, fonts, "Size", 1.0, 200.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.brush_radius).unwrap_or(20.0),
        |w, v| set_settings(w, |s| s.brush_radius = *v),
    );
    let falloff = labelled_slider(
        commands, fonts, "Falloff", 0.0, 1.0,
        |w| w.get_resource::<TerrainSettings>().map(|s| s.falloff).unwrap_or(0.7),
        |w, v| set_settings(w, |s| s.falloff = *v),
    );

    // Shape buttons (sculpt → TerrainSettings.brush_shape).
    let shape_row = field_row(commands, fonts, "Shape");
    for (shape, icon) in [
        (BrushShape::Circle, "circle"),
        (BrushShape::Square, "square"),
        (BrushShape::Diamond, "diamond"),
    ] {
        let b = shape_button(commands, fonts, ShapeTarget::Sculpt, shape, icon, "");
        commands.entity(shape_row).add_child(b);
    }

    // Falloff-type buttons.
    let ft_row = field_row(commands, fonts, "Falloff Type");
    for (ft, label) in [
        (BrushFalloffType::Smooth, "S"),
        (BrushFalloffType::Linear, "L"),
        (BrushFalloffType::Spherical, "O"),
        (BrushFalloffType::Tip, "T"),
        (BrushFalloffType::Flat, "F"),
    ] {
        let b = falloff_type_button(commands, fonts, ft, label);
        commands.entity(ft_row).add_child(b);
    }

    commands
        .entity(body)
        .add_children(&[size, falloff, shape_row, ft_row]);
}

// ── Heightmap Import ─────────────────────────────────────────────────────────

fn heightmap_section(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let note = commands
        .spawn((
            Text::new("Import a heightmap PNG or RAW16 file."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(4.0)), ..default() },
        ))
        .id();
    let import = wide_button(commands, fonts, Some("plus"), "Import Heightmap...", text_primary());
    commands.entity(import).insert(HeightmapImportBtn);
    let export = wide_button(commands, fonts, None, "Export Heightmap...", text_muted());
    commands.entity(export).insert(HeightmapExportBtn);
    commands.entity(body).add_children(&[note, import, export]);
}
