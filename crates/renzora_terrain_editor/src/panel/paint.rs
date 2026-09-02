//! The Paint tab: the 4-brush tool grid, the layer list with its per-layer
//! material drop-zone, Brush Settings and the Foliage pointer.

use std::hash::{Hash, Hasher};

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::KeyedSnapshot;
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_bg, bind_display, bind_text, bind_text_color, keyed_list};
use renzora_ember::theme::*;
use renzora_ember::widgets::collapsible;
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::BrushShape;
use renzora_terrain::paint::{
    PaintBrushType, SurfacePaintSettings, SurfacePaintState, MAX_LAYERS,
};

use super::widgets::{field_row, labelled_slider, shape_button, wide_button};
use super::{
    hasher, paint_brush_type, set_paint, AddLayerBtn, LayerRow, MaterialClearBtn, MaterialDropZone,
    PaintToolBtn, ShapeTarget,
};

const PAINT_TOOLS: &[(PaintBrushType, &str, &str)] = &[
    (PaintBrushType::Paint, "paint-brush", "Paint"),
    (PaintBrushType::Erase, "eraser", "Erase"),
    (PaintBrushType::Smooth, "waves", "Smooth"),
    (PaintBrushType::Fill, "palette", "Fill"),
];

pub(super) fn paint_content(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
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
            Text::new("Paint foliage with the Foliage tool (tree icon) in the viewport toolbar."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let f2 = commands
        .spawn((
            Text::new("Density, brush, and grass settings live in the Foliage panel."),
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

// ── Layers ───────────────────────────────────────────────────────────────────

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

    // Empty state: fresh terrains have no layers until Add Layer (or the
    // first paint stroke, which auto-creates one).
    let hint = commands
        .spawn((
            Text::new("No layers yet — click Add Layer, then paint."),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { margin: UiRect::bottom(Val::Px(2.0)), ..default() },
        ))
        .id();
    bind_display(commands, hint, |w| layer_count(w) == 0);

    let add = wide_button(commands, fonts, Some("plus"), "Add Layer", text_muted());
    commands.entity(add).insert(AddLayerBtn);
    // Adding past MAX_LAYERS is a no-op, so hide the button once it's reached.
    bind_display(commands, add, |w| layer_count(w) < MAX_LAYERS);

    commands.entity(body).add_children(&[list, hint, add]);
}

fn layer_count(w: &Rx) -> usize {
    // Real count only — no phantom placeholder rows. An empty painter shows
    // the "no layers" hint plus the Add Layer button.
    w.get_resource::<SurfacePaintState>()
        .map(|s| s.layer_count)
        .unwrap_or(0)
}

fn active_layer(w: &Rx) -> usize {
    w.get_resource::<SurfacePaintSettings>()
        .map(|s| s.active_layer)
        .unwrap_or(0)
}

fn layer_name(w: &Rx, i: usize) -> String {
    if let Some(ps) = w.get_resource::<SurfacePaintState>() {
        if let Some(p) = ps.layers_preview.get(i) {
            return p.name.clone();
        }
    }
    format!("Layer {}", i + 1)
}

fn layer_material_source(w: &Rx, i: usize) -> Option<String> {
    w.get_resource::<SurfacePaintState>()
        .and_then(|ps| ps.layers_preview.get(i))
        .and_then(|p| p.material_source.clone())
}

fn layers_snapshot(world: &Rx) -> KeyedSnapshot {
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

    // Material drop-zone, shown only on the active layer — one target at a
    // time, so a drop is never ambiguous about which layer it lands on.
    let drop = material_drop_zone(commands, fonts, index);
    bind_display(commands, drop, move |w| active_layer(w) == index);

    commands.entity(wrap).add_children(&[row, drop]);
    wrap
}

// ── Material drop-zone (asset drop + clear) ──────────────────────────────────

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

// ── Brush Settings ───────────────────────────────────────────────────────────

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
