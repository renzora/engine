//! Bevy-native (ember) port of the egui `TerrainToolsPanel` (panel id
//! "terrain_tools"): an enable toggle over a Sculpt / Paint tab bar.
//!
//! * **Sculpt** — a 16-tool brush grid plus collapsibles for Tool Settings
//!   (strength + per-brush Flatten / Noise / Terrace controls), Brush Settings
//!   (size / falloff / shape / falloff-type) and Heightmap Import (import +
//!   export buttons).
//! * **Paint** — a 4-tool brush grid plus collapsibles for Layers (selectable
//!   list + per-active-layer material drop-zone + Add Layer), Brush Settings
//!   (size / strength / falloff + shape) and Foliage (info text).
//!
//! Every control writes back into the exact resources the egui panel mutates:
//! [`TerrainToolState`], [`TerrainSettings`], [`SurfacePaintSettings`] and
//! [`SurfacePaintState`] (the last via its `pending_commands` queue). The native
//! content is registered with `register_panel_content("terrain_tools", true, …)`
//! so it overrides the egui panel body.

use bevy::ecs::system::RunSystemOnce;
use bevy::prelude::*;
use bevy::ui::{ComputedNode, RelativeCursorPosition};
use std::hash::{Hash, Hasher};

use renzora::core::CurrentProject;
use renzora_editor_framework::{AssetDragPayload, SplashState};
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::panel::RegisterPanelContent;
use renzora_ember::reactive::{
    bind_2way, bind_bg, bind_display, bind_text, bind_text_color, keyed_list, KeyedSnapshot,
};
use renzora_ember::theme::*;
use renzora_ember::widgets::{
    collapsible, drag_value, menu_item, screen_menu, slider, DragRange,
};
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::{
    BrushFalloffType, BrushShape, FlattenMode, NoiseMode, TerrainBrushType, TerrainSettings,
    TerrainTab, TerrainToolState,
};
use renzora_terrain::paint::{
    PaintBrushType, SurfacePaintCommand, SurfacePaintSettings, SurfacePaintState, MAX_LAYERS,
};

const LABEL_W: f32 = 100.0;
const MATERIAL_EXTS: &[&str] = &["material"];

pub struct NativeTerrain;

impl Plugin for NativeTerrain {
    fn build(&self, app: &mut App) {
        app.register_panel_content("terrain_tools", true, build);
        app.add_systems(
            Update,
            (
                enable_toggle_click,
                tab_click,
                sculpt_tool_click,
                paint_tool_click,
                shape_btn_click,
                falloff_type_btn_click,
                flatten_mode_combo_open,
                noise_mode_combo_open,
                layer_row_click,
                add_layer_click,
                heightmap_import_click,
                heightmap_export_click,
                material_drop,
                material_clear_click,
                material_drop_highlight,
            )
                .run_if(in_state(SplashState::Editor)),
        );
    }
}

// ── State accessors (mirror the egui panel's `get_resource` reads) ───────────

fn tool_active(w: &World) -> bool {
    w.get_resource::<TerrainToolState>()
        .map(|t| t.active)
        .unwrap_or_default()
}

fn settings_tab(w: &World) -> TerrainTab {
    w.get_resource::<TerrainSettings>()
        .map(|s| s.tab)
        .unwrap_or_default()
}

fn brush_type(w: &World) -> TerrainBrushType {
    w.get_resource::<TerrainSettings>()
        .map(|s| s.brush_type)
        .unwrap_or_default()
}

fn paint_brush_type(w: &World) -> PaintBrushType {
    w.get_resource::<SurfacePaintSettings>()
        .map(|s| s.brush_type)
        .unwrap_or_default()
}

fn set_settings(w: &mut World, f: impl FnOnce(&mut TerrainSettings)) {
    if let Some(mut s) = w.get_resource_mut::<TerrainSettings>() {
        f(&mut s);
    }
}

fn set_paint(w: &mut World, f: impl FnOnce(&mut SurfacePaintSettings)) {
    if let Some(mut s) = w.get_resource_mut::<SurfacePaintSettings>() {
        f(&mut s);
    }
}

fn hasher() -> std::collections::hash_map::DefaultHasher {
    std::collections::hash_map::DefaultHasher::new()
}

// ── Panel root ───────────────────────────────────────────────────────────────

fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(8.0),
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            Name::new("native-terrain"),
        ))
        .id();

    // ── Enable / disable toggle (full-width pill) ────────────────────────────
    let toggle = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(32.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(6.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            EnableToggle,
            Name::new("terrain-enable"),
        ))
        .id();
    bind_bg(commands, toggle, move |w| {
        if tool_active(w) {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(toggle),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(card_bg())
        }
    });
    let toggle_icon = icon_text(commands, &fonts.phosphor, "mountains", text_primary(), 14.0);
    bind_text_color(commands, toggle_icon, |w| {
        if tool_active(w) {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    let toggle_label = commands
        .spawn((
            Text::new("Enable Terrain Mode"),
            ui_font(&fonts.ui, 13.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, toggle_label, |w| {
        if tool_active(w) {
            "Terrain Mode Active".to_string()
        } else {
            "Enable Terrain Mode".to_string()
        }
    });
    bind_text_color(commands, toggle_label, |w| {
        if tool_active(w) {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands
        .entity(toggle)
        .add_children(&[toggle_icon, toggle_label]);

    // ── Inactive hint (shown only when the tool is off) ──────────────────────
    let hint = commands
        .spawn((
            Text::new("Select a terrain entity and enable terrain mode to begin editing."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_display(commands, hint, |w| !tool_active(w));

    // ── Active body (tabs + content; shown only when the tool is on) ─────────
    let body = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    bind_display(commands, body, tool_active);

    let tabs = tab_bar(commands, fonts);

    // Sculpt + Paint content, toggled by the active tab.
    let sculpt = sculpt_content(commands, fonts);
    bind_display(commands, sculpt, |w| settings_tab(w) == TerrainTab::Sculpt);
    let paint = paint_content(commands, fonts);
    bind_display(commands, paint, |w| settings_tab(w) == TerrainTab::Paint);

    commands.entity(body).add_children(&[tabs, sculpt, paint]);

    commands.entity(root).add_children(&[toggle, hint, body]);
    root
}

#[derive(Component)]
struct EnableToggle;

// ── Tab bar ──────────────────────────────────────────────────────────────────

#[derive(Component)]
struct TabBtn {
    tab: TerrainTab,
}

fn tab_bar(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let sculpt = tab_button(commands, fonts, "mountains", "Sculpt", TerrainTab::Sculpt);
    let paint = tab_button(commands, fonts, "paint-brush", "Paint", TerrainTab::Paint);
    commands.entity(row).add_children(&[sculpt, paint]);
    row
}

fn tab_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    tab: TerrainTab,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
                height: Val::Px(30.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            TabBtn { tab },
            Name::new(format!("terrain-tab:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if settings_tab(w) == tab {
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
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 13.0);
    bind_text_color(commands, ic, move |w| {
        if settings_tab(w) == tab {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text_color(commands, lbl, move |w| {
        if settings_tab(w) == tab {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

// ── Sculpt content ───────────────────────────────────────────────────────────

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
];

fn sculpt_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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

#[derive(Component)]
struct SculptToolBtn {
    brush: TerrainBrushType,
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

// ── Sculpt: Tool Settings ────────────────────────────────────────────────────

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
}

#[derive(Component)]
struct FlattenModeCombo;

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

fn flatten_mode_label(m: FlattenMode) -> String {
    match m {
        FlattenMode::Both => "Both",
        FlattenMode::Raise => "Raise",
        FlattenMode::Lower => "Lower",
    }
    .to_string()
}

#[derive(Component)]
struct NoiseModeCombo;

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

// ── Sculpt: Brush Settings ───────────────────────────────────────────────────

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

// ── Sculpt: Heightmap Import ─────────────────────────────────────────────────

#[derive(Component)]
struct HeightmapImportBtn;
#[derive(Component)]
struct HeightmapExportBtn;

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

// ── Paint content ────────────────────────────────────────────────────────────

const PAINT_TOOLS: &[(PaintBrushType, &str, &str)] = &[
    (PaintBrushType::Paint, "paint-brush", "Paint"),
    (PaintBrushType::Erase, "eraser", "Erase"),
    (PaintBrushType::Smooth, "waves", "Smooth"),
    (PaintBrushType::Fill, "palette", "Fill"),
];

fn paint_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let root = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(8.0),
            ..default()
        })
        .id();

    let grid = paint_tool_grid(commands, fonts);

    let (layers_sec, layers_body) = collapsible(commands, fonts, None, "Layers", true);
    layers_section(commands, fonts, layers_body);

    let (brush_sec, brush_body) = collapsible(commands, fonts, None, "Brush Settings", true);
    paint_brush_settings(commands, fonts, brush_body);

    let (foliage_sec, foliage_body) = collapsible(commands, fonts, None, "Foliage", false);
    let f1 = commands
        .spawn((
            Text::new("Auto-scatter foliage based on layer weights."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let f2 = commands
        .spawn((
            Text::new("Configure foliage per-layer via TerrainFoliageConfig component."),
            ui_font(&fonts.ui, 10.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::top(Val::Px(4.0)), ..default() },
        ))
        .id();
    commands.entity(foliage_body).add_children(&[f1, f2]);

    commands
        .entity(root)
        .add_children(&[grid, layers_sec, brush_sec, foliage_sec]);
    root
}

#[derive(Component)]
struct PaintToolBtn {
    brush: PaintBrushType,
}

fn paint_tool_grid(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    let grid = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            column_gap: Val::Px(4.0),
            ..default()
        })
        .id();
    let mut kids = Vec::with_capacity(PAINT_TOOLS.len());
    for &(bt, icon, label) in PAINT_TOOLS {
        kids.push(paint_tool_button(commands, fonts, bt, icon, label));
    }
    commands.entity(grid).add_children(&kids);
    grid
}

fn paint_tool_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    brush: PaintBrushType,
    icon: &str,
    label: &str,
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                flex_basis: Val::Px(0.0),
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
            PaintToolBtn { brush },
            Name::new(format!("terrain-paint-tool:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        if paint_brush_type(w) == brush {
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
        if paint_brush_type(w) == brush {
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
        if paint_brush_type(w) == brush {
            rgb(text_primary())
        } else {
            rgb(text_muted())
        }
    });
    commands.entity(btn).add_children(&[ic, lbl]);
    btn
}

// ── Paint: Layers ────────────────────────────────────────────────────────────

#[derive(Component)]
struct LayerRow {
    index: usize,
}
#[derive(Component)]
struct AddLayerBtn;

fn layers_section(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let list = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();
    keyed_list(commands, list, layers_snapshot);

    let add = wide_button(commands, fonts, Some("plus"), "Add Layer", text_muted());
    commands.entity(add).insert(AddLayerBtn);
    // Match egui: hide Add Layer once MAX_LAYERS is reached.
    bind_display(commands, add, |w| layer_count(w) < MAX_LAYERS);

    commands.entity(body).add_children(&[list, add]);
}

fn layer_count(w: &World) -> usize {
    w.get_resource::<SurfacePaintState>()
        .map(|s| s.layer_count.max(2))
        .unwrap_or(2)
}

fn active_layer(w: &World) -> usize {
    w.get_resource::<SurfacePaintSettings>()
        .map(|s| s.active_layer)
        .unwrap_or(0)
}

fn layer_name(w: &World, i: usize) -> String {
    if let Some(ps) = w.get_resource::<SurfacePaintState>() {
        if let Some(p) = ps.layers_preview.get(i) {
            return p.name.clone();
        }
    }
    match i {
        0 => "Grass".to_string(),
        1 => "Dirt".to_string(),
        2 => "Water".to_string(),
        3 => "Rock".to_string(),
        _ => format!("Layer {}", i + 1),
    }
}

fn layer_material_source(w: &World, i: usize) -> Option<String> {
    w.get_resource::<SurfacePaintState>()
        .and_then(|ps| ps.layers_preview.get(i))
        .and_then(|p| p.material_source.clone())
}

fn layers_snapshot(world: &World) -> KeyedSnapshot {
    let count = layer_count(world).min(MAX_LAYERS);
    // Key + hash on STRUCTURE (index + name) — not on selection or the material
    // path — so selecting a row / dropping a material never rebuilds the list.
    let names: Vec<String> = (0..count).map(|i| layer_name(world, i)).collect();
    let items: Vec<(u64, u64)> = (0..count)
        .map(|i| {
            let mut k = hasher();
            i.hash(&mut k);
            let mut h = hasher();
            (i, &names[i]).hash(&mut h);
            (k.finish(), h.finish())
        })
        .collect();
    KeyedSnapshot {
        items,
        build: Box::new(move |c, f, i| layer_row(c, f, i, &names[i])),
    }
}

fn layer_row(commands: &mut Commands, fonts: &EmberFonts, index: usize, name: &str) -> Entity {
    let wrap = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .id();

    // Selectable row.
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(26.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                padding: UiRect::horizontal(Val::Px(8.0)),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            LayerRow { index },
            Name::new(format!("terrain-layer:{index}")),
        ))
        .id();
    bind_bg(commands, row, move |w| {
        if active_layer(w) == index {
            rgb(accent())
        } else if matches!(
            w.get::<Interaction>(row),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        ) {
            rgb(hover_bg())
        } else {
            rgb(card_bg())
        }
    });
    let label = commands
        .spawn((
            Text::new(format!("{}  {}", index + 1, name)),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text_color(commands, label, move |w| {
        if active_layer(w) == index {
            Color::WHITE
        } else {
            rgb(text_primary())
        }
    });
    commands.entity(row).add_child(label);

    // Material drop-zone for the active layer (matches egui's per-active-layer
    // `asset_drop_target`). Hidden unless this row is the active layer.
    let drop = material_drop_zone(commands, fonts, index);
    bind_display(commands, drop, move |w| active_layer(w) == index);

    commands.entity(wrap).add_children(&[row, drop]);
    wrap
}

// ── Material drop-zone (asset drop + browse + clear) ─────────────────────────

#[derive(Component)]
struct MaterialDropZone {
    layer: usize,
}
#[derive(Component)]
struct MaterialClearBtn {
    layer: usize,
}

fn material_drop_zone(commands: &mut Commands, fonts: &EmberFonts, layer: usize) -> Entity {
    let path_text = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            bevy::text::TextLayout::no_wrap(),
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    bind_text(commands, path_text, move |w| {
        match layer_material_source(w, layer) {
            Some(p) if !p.is_empty() => std::path::Path::new(&p)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(p),
            _ => "Drop .material file".to_string(),
        }
    });
    bind_text_color(commands, path_text, move |w| {
        match layer_material_source(w, layer) {
            Some(p) if !p.is_empty() => rgb(text_primary()),
            _ => rgb(text_muted()),
        }
    });
    let drop_box = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(6.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            MaterialDropZone { layer },
            Name::new("terrain-mat-drop"),
        ))
        .id();
    commands.entity(drop_box).add_child(path_text);
    let clear = commands
        .spawn((
            Text::new("\u{2715}"),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { padding: UiRect::horizontal(Val::Px(2.0)), ..default() },
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            MaterialClearBtn { layer },
            Name::new("terrain-mat-clear"),
        ))
        .id();
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    commands.entity(row).add_children(&[drop_box, clear]);
    row
}

// ── Paint: Brush Settings ────────────────────────────────────────────────────

fn paint_brush_settings(commands: &mut Commands, fonts: &EmberFonts, body: Entity) {
    let size = labelled_slider(
        commands, fonts, "Size", 0.01, 0.5,
        |w| w.get_resource::<SurfacePaintSettings>().map(|s| s.brush_radius).unwrap_or(0.1),
        |w, v| set_paint(w, |s| s.brush_radius = *v),
    );
    let strength = labelled_slider(
        commands, fonts, "Strength", 0.01, 1.0,
        |w| w.get_resource::<SurfacePaintSettings>().map(|s| s.brush_strength).unwrap_or(0.5),
        |w, v| set_paint(w, |s| s.brush_strength = *v),
    );
    let falloff = labelled_slider(
        commands, fonts, "Falloff", 0.0, 1.0,
        |w| w.get_resource::<SurfacePaintSettings>().map(|s| s.brush_falloff).unwrap_or(1.0),
        |w, v| set_paint(w, |s| s.brush_falloff = *v),
    );

    // Shape buttons (paint → SurfacePaintSettings.brush_shape).
    let shape_row = field_row(commands, fonts, "Shape");
    for (shape, icon) in [
        (BrushShape::Circle, "circle"),
        (BrushShape::Square, "square"),
        (BrushShape::Diamond, "diamond"),
    ] {
        let b = shape_button(commands, fonts, ShapeTarget::Paint, shape, icon, "");
        commands.entity(shape_row).add_child(b);
    }

    commands
        .entity(body)
        .add_children(&[size, strength, falloff, shape_row]);
}

// ── Shape / falloff-type toggle buttons ──────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ShapeTarget {
    Sculpt,
    Paint,
}

#[derive(Component)]
struct ShapeBtn {
    target: ShapeTarget,
    shape: BrushShape,
}

fn shape_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    target: ShapeTarget,
    shape: BrushShape,
    icon: &str,
    _label: &str,
) -> Entity {
    let btn = small_toggle(commands);
    commands.entity(btn).insert(ShapeBtn { target, shape });
    bind_bg(commands, btn, move |w| {
        let cur = match target {
            ShapeTarget::Sculpt => w.get_resource::<TerrainSettings>().map(|s| s.brush_shape),
            ShapeTarget::Paint => w.get_resource::<SurfacePaintSettings>().map(|s| s.brush_shape),
        };
        toggle_bg(w, btn, cur == Some(shape))
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, text_primary(), 12.0);
    commands.entity(btn).add_child(ic);
    btn
}

#[derive(Component)]
struct FalloffTypeBtn {
    ft: BrushFalloffType,
}

fn falloff_type_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    ft: BrushFalloffType,
    label: &str,
) -> Entity {
    let btn = small_toggle(commands);
    commands.entity(btn).insert(FalloffTypeBtn { ft });
    bind_bg(commands, btn, move |w| {
        let cur = w.get_resource::<TerrainSettings>().map(|s| s.falloff_type);
        toggle_bg(w, btn, cur == Some(ft))
    });
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(btn).add_child(lbl);
    btn
}

fn small_toggle(commands: &mut Commands) -> Entity {
    commands
        .spawn((
            Node {
                width: Val::Px(28.0),
                height: Val::Px(24.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new("terrain-toggle"),
        ))
        .id()
}

fn toggle_bg(w: &World, btn: Entity, active: bool) -> Color {
    if active {
        rgb(accent())
    } else if matches!(
        w.get::<Interaction>(btn),
        Some(Interaction::Hovered) | Some(Interaction::Pressed)
    ) {
        rgb(popup_bg())
    } else {
        rgb(card_bg())
    }
}

// ── Shared builders ──────────────────────────────────────────────────────────

fn section_col(commands: &mut Commands) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            margin: UiRect::top(Val::Px(2.0)),
            ..default()
        })
        .id()
}

fn caption(commands: &mut Commands, fonts: &EmberFonts, text: &str, color: (u8, u8, u8)) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(color)),
            Node { margin: UiRect::vertical(Val::Px(2.0)), ..default() },
        ))
        .id()
}

/// A row whose left cell is a fixed-width muted label and whose remaining
/// children flow to the right.
fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(4.0),
            margin: UiRect::bottom(Val::Px(2.0)),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, ..default() },
        ))
        .id();
    commands.entity(row).add_child(lbl);
    row
}

/// A labelled scrubbable numeric field: `[label] [drag_value]`.
fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = field_row(commands, fonts, label);
    // `min` is a transient seed — `bind_2way` corrects it from the live world on
    // its first run before the user ever sees the field.
    let dv = drag_value(commands, &fonts.ui, "", value_text(), min, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    commands.entity(row).add_child(dv);
    row
}

/// A labelled slider: `[label] [slider 0..1 mapped to min..max]`.
fn labelled_slider<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&World) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = field_row(commands, fonts, label);
    let span = (max - min).max(1e-6);
    // The ember slider's model is 0..1; map to the real range both ways.
    // `0.0` is a transient seed — `bind_2way` corrects it from the live world.
    let sld = slider(commands, 0.0);
    commands.entity(sld).insert(Node {
        flex_grow: 1.0,
        min_width: Val::Px(0.0),
        ..default()
    });
    let get_n = move |w: &World| ((get(w) - min) / span).clamp(0.0, 1.0);
    let set_n = move |w: &mut World, v: &f32| {
        let real = min + v.clamp(0.0, 1.0) * span;
        set(w, &real);
    };
    bind_2way(commands, sld, get_n, set_n);
    commands.entity(row).add_child(sld);
    row
}

/// A wide (full-width) action button with an optional leading icon.
fn wide_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: Option<&str>,
    label: &str,
    text_color: (u8, u8, u8),
) -> Entity {
    let btn = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(24.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                margin: UiRect::top(Val::Px(2.0)),
                ..default()
            },
            BackgroundColor(rgb(card_bg())),
            Interaction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("terrain-btn:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| match w.get::<Interaction>(btn) {
        Some(Interaction::Hovered) | Some(Interaction::Pressed) => rgb(hover_bg()),
        _ => rgb(card_bg()),
    });
    let mut kids = Vec::new();
    if let Some(name) = icon {
        kids.push(icon_text(commands, &fonts.phosphor, name, text_color, 12.0));
    }
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_color)),
        ))
        .id();
    kids.push(t);
    commands.entity(btn).add_children(&kids);
    btn
}

/// A combo (dropdown trigger) showing `label_fn(world)` with a caret. The marker
/// component `M` drives the system that opens the screen-menu of options.
fn enum_combo<M, L>(commands: &mut Commands, fonts: &EmberFonts, marker: M, label_fn: L) -> Entity
where
    M: Component,
    L: Fn(&World) -> String + Send + Sync + 'static,
{
    let combo = commands
        .spawn((
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::SpaceBetween,
                column_gap: Val::Px(4.0),
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            Interaction::default(),
            RelativeCursorPosition::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            marker,
            Name::new("terrain-combo"),
        ))
        .id();
    let val = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    bind_text(commands, val, label_fn);
    let caret = icon_text(commands, &fonts.phosphor, "caret-down", text_muted(), 9.0);
    commands.entity(combo).add_children(&[val, caret]);
    combo
}

// ── Systems: clicks, combos, drops ───────────────────────────────────────────

fn enable_toggle_click(
    q: Query<&Interaction, (With<EnableToggle>, Changed<Interaction>)>,
    mut tool: Option<ResMut<TerrainToolState>>,
) {
    let Some(tool) = tool.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        tool.active = !tool.active;
    }
}

fn tab_click(
    q: Query<(&Interaction, &TabBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.tab = btn.tab;
        }
    }
}

fn sculpt_tool_click(
    q: Query<(&Interaction, &SculptToolBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.brush_type = btn.brush;
        }
    }
}

fn paint_tool_click(
    q: Query<(&Interaction, &PaintToolBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<SurfacePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.brush_type = btn.brush;
        }
    }
}

fn shape_btn_click(
    q: Query<(&Interaction, &ShapeBtn), Changed<Interaction>>,
    mut terrain: Option<ResMut<TerrainSettings>>,
    mut paint: Option<ResMut<SurfacePaintSettings>>,
) {
    for (interaction, btn) in &q {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match btn.target {
            ShapeTarget::Sculpt => {
                if let Some(t) = terrain.as_mut() {
                    t.brush_shape = btn.shape;
                }
            }
            ShapeTarget::Paint => {
                if let Some(p) = paint.as_mut() {
                    p.brush_shape = btn.shape;
                }
            }
        }
    }
}

fn falloff_type_btn_click(
    q: Query<(&Interaction, &FalloffTypeBtn), Changed<Interaction>>,
    mut settings: Option<ResMut<TerrainSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            settings.falloff_type = btn.ft;
        }
    }
}

fn flatten_mode_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<FlattenModeCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let modes = [FlattenMode::Both, FlattenMode::Raise, FlattenMode::Lower];
    let kids: Vec<Entity> = modes
        .iter()
        .map(|&mode| {
            menu_item(&mut commands, &fonts, "dot", &flatten_mode_label(mode), move |w| {
                set_settings(w, |s| s.flatten_mode = mode);
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn noise_mode_combo_open(
    q: Query<
        (&Interaction, &RelativeCursorPosition, &ComputedNode),
        (With<NoiseModeCombo>, Changed<Interaction>),
    >,
    windows: Query<&Window>,
    fonts: Option<Res<EmberFonts>>,
    mut commands: Commands,
) {
    let Some(fonts) = fonts else { return };
    let Some((_, rcp, cn)) = q.iter().find(|(i, _, _)| **i == Interaction::Pressed) else {
        return;
    };
    let Some(cursor) = windows.iter().next().and_then(|w| w.cursor_position()) else {
        return;
    };
    let size = cn.size() * cn.inverse_scale_factor();
    let top_left = cursor - (rcp.normalized.unwrap_or(Vec2::ZERO) + Vec2::splat(0.5)) * size;
    let menu = screen_menu(&mut commands, top_left.x, top_left.y + size.y + 2.0);
    let kids: Vec<Entity> = NoiseMode::all()
        .iter()
        .map(|&mode| {
            menu_item(&mut commands, &fonts, "dot", mode.display_name(), move |w| {
                set_settings(w, |s| s.noise_mode = mode);
            })
        })
        .collect();
    commands.entity(menu).add_children(&kids);
}

fn layer_row_click(
    q: Query<(&Interaction, &LayerRow), Changed<Interaction>>,
    mut settings: Option<ResMut<SurfacePaintSettings>>,
) {
    let Some(settings) = settings.as_mut() else { return };
    for (interaction, row) in &q {
        if *interaction == Interaction::Pressed {
            settings.active_layer = row.index;
        }
    }
}

fn add_layer_click(
    q: Query<&Interaction, (With<AddLayerBtn>, Changed<Interaction>)>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    let Some(state) = state.as_mut() else { return };
    if q.iter().any(|i| *i == Interaction::Pressed) {
        state.pending_commands.push(SurfacePaintCommand::AddLayer);
    }
}

fn heightmap_import_click(
    q: Query<&Interaction, (With<HeightmapImportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let _ = world.run_system_once(run_heightmap_import);
        });
    }
}

fn heightmap_export_click(
    q: Query<&Interaction, (With<HeightmapExportBtn>, Changed<Interaction>)>,
    mut commands: Commands,
) {
    if q.iter().any(|i| *i == Interaction::Pressed) {
        commands.queue(|world: &mut World| {
            let _ = world.run_system_once(run_heightmap_export);
        });
    }
}

/// Import a heightmap file into every chunk (mirrors the egui import button).
fn run_heightmap_import(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Heightmap", &["png", "r16", "raw"])
            .pick_file()
        else {
            return;
        };
        let import_settings = renzora_terrain::heightmap_import::HeightmapImportSettings::default();
        let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
        let Some(terrain_data) = terrain_query.iter(world).next().cloned() else {
            return;
        };
        match renzora_terrain::heightmap_import::import_heightmap(&path, &import_settings, &terrain_data) {
            Ok(imported) => {
                let mut chunk_query = world.query::<&mut renzora_terrain::data::TerrainChunkData>();
                for mut chunk in chunk_query.iter_mut(world) {
                    if let Some((_, _, heights)) = imported
                        .iter()
                        .find(|(cx, cz, _)| *cx == chunk.chunk_x && *cz == chunk.chunk_z)
                    {
                        chunk.base_heights = heights.clone();
                        chunk.dirty = true;
                    }
                }
            }
            Err(e) => bevy::log::error!("Heightmap import failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

/// Export the composed heightmap to a PNG (mirrors the egui export button).
fn run_heightmap_export(world: &mut World) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let Some(path) = rfd::FileDialog::new().add_filter("PNG", &["png"]).save_file() else {
            return;
        };
        let mut terrain_query = world.query::<&renzora_terrain::data::TerrainData>();
        let Some(terrain_data) = terrain_query.iter(world).next().cloned() else {
            return;
        };
        let mut chunk_query = world.query::<&renzora_terrain::data::TerrainChunkData>();
        let chunks: Vec<&renzora_terrain::data::TerrainChunkData> = chunk_query.iter(world).collect();
        match renzora_terrain::heightmap_import::export_heightmap_png16(&terrain_data, &chunks) {
            Ok(data) => {
                if let Err(e) = std::fs::write(&path, &data) {
                    bevy::log::error!("Failed to write heightmap: {e}");
                }
            }
            Err(e) => bevy::log::error!("Heightmap export failed: {e}"),
        }
    }
    #[cfg(target_arch = "wasm32")]
    let _ = world;
}

/// Drop a dragged `.material` asset onto the hovered layer zone → queue an
/// `AssignMaterial` command (matches the egui drop target).
fn material_drop(
    mouse: Res<ButtonInput<MouseButton>>,
    payload: Option<Res<AssetDragPayload>>,
    project: Option<Res<CurrentProject>>,
    zones: Query<(&RelativeCursorPosition, &MaterialDropZone)>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    if !mouse.just_released(MouseButton::Left) {
        return;
    }
    let (Some(payload), Some(state)) = (payload, state.as_mut()) else {
        return;
    };
    if !payload.is_detached || !payload.matches_extensions(MATERIAL_EXTS) {
        return;
    }
    for (rcp, zone) in &zones {
        if !rcp.cursor_over {
            continue;
        }
        let path = project
            .as_ref()
            .map(|p| p.make_asset_relative(&payload.path))
            .unwrap_or_else(|| payload.path.to_string_lossy().to_string());
        state.pending_commands.push(SurfacePaintCommand::AssignMaterial {
            layer: zone.layer,
            path,
        });
        break;
    }
}

fn material_clear_click(
    q: Query<(&Interaction, &MaterialClearBtn), Changed<Interaction>>,
    mut state: Option<ResMut<SurfacePaintState>>,
) {
    let Some(state) = state.as_mut() else { return };
    for (interaction, btn) in &q {
        if *interaction == Interaction::Pressed {
            state
                .pending_commands
                .push(SurfacePaintCommand::ClearMaterial(btn.layer));
        }
    }
}

/// Accent the zone border while a compatible `.material` asset is dragged over.
fn material_drop_highlight(
    payload: Option<Res<AssetDragPayload>>,
    mut zones: Query<(&RelativeCursorPosition, &mut BorderColor), With<MaterialDropZone>>,
) {
    for (rcp, mut bc) in &mut zones {
        let active = payload
            .as_ref()
            .is_some_and(|p| p.is_detached && rcp.cursor_over && p.matches_extensions(MATERIAL_EXTS));
        let want = BorderColor::all(rgb(if active { accent() } else { border() }));
        if *bc != want {
            *bc = want;
        }
    }
}
