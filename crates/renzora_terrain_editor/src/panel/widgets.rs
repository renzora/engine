//! The small builders both tabs share: labelled rows, scrubbable fields,
//! sliders, wide buttons, enum combos, and the square toggle the shape and
//! falloff-type buttons are made of.

use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::reactive::Rx;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_text};
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, slider, DragRange};
use renzora_ember::cursor_icon::HoverCursor;

use renzora_terrain::data::{BrushFalloffType, BrushShape, TerrainSettings};
use renzora_terrain::paint::SurfacePaintSettings;

use super::{FalloffTypeBtn, ShapeBtn, ShapeTarget, LABEL_W};

// ── Shape / falloff-type toggle buttons ──────────────────────────────────────

pub(super) fn shape_button(
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

pub(super) fn falloff_type_button(
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

fn toggle_bg(w: &Rx, btn: Entity, active: bool) -> Color {
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

pub(super) fn section_col(commands: &mut Commands) -> Entity {
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

pub(super) fn caption(
    commands: &mut Commands,
    fonts: &EmberFonts,
    text: &str,
    color: (u8, u8, u8),
) -> Entity {
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
pub(super) fn field_row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> Entity {
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
pub(super) fn labelled_drag<G, S>(
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
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
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
pub(super) fn labelled_slider<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
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
    let get_n = move |w: &Rx| ((get(w) - min) / span).clamp(0.0, 1.0);
    let set_n = move |w: &mut World, v: &f32| {
        let real = min + v.clamp(0.0, 1.0) * span;
        set(w, &real);
    };
    bind_2way(commands, sld, get_n, set_n);
    commands.entity(row).add_child(sld);
    row
}

/// A wide (full-width) action button with an optional leading icon.
pub(super) fn wide_button(
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
pub(super) fn enum_combo<M, L>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    marker: M,
    label_fn: L,
) -> Entity
where
    M: Component,
    L: Fn(&Rx) -> String + Send + Sync + 'static,
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
