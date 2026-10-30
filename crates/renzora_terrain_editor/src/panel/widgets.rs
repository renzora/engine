//! The small builders every mode shares: cards, group labels, hints, labelled
//! rows, numeric fields with a unit, segmented and dropdown rows, and buttons.
//!
//! Every colour comes from the ember theme at the moment it is drawn (through
//! the `bind_*` closures, which re-read the palette), so the component follows
//! a theme switch and looks like the rest of the inspector in either theme.

use bevy::prelude::*;

use renzora_ember::cursor_icon::HoverCursor;
use renzora_ember::font::{icon_text, ui_font, EmberFonts};
use renzora_ember::interaction::PASS_THROUGH;
use renzora_ember::reactive::tracked::{bind_2way, bind_bg, bind_text, bind_text_color};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value, dropdown_compact, segmented, toggle_switch, DragRange, DragSnap};
use renzora_ember::UiInteraction;

use super::LABEL_W;

// ── Containers ───────────────────────────────────────────────────────────────

/// A full-width column with a gap: the body of a mode, or a group inside one.
pub(super) fn column(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

/// A full-width row of children with a gap, wrapping when they run out of room.
pub(super) fn wrap_row(commands: &mut Commands, gap: f32) -> Entity {
    commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            flex_wrap: FlexWrap::Wrap,
            align_items: AlignItems::Center,
            column_gap: Val::Px(gap),
            row_gap: Val::Px(gap),
            ..default()
        })
        .id()
}

/// A raised card: the one box each group of related settings sits in, so a
/// mode reads as a few labelled blocks rather than one long list of fields.
pub(super) fn card(commands: &mut Commands) -> Entity {
    let e = commands
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
            Name::new("terrain-card"),
        ))
        .id();
    bind_bg(commands, e, |_| rgb(card_bg()));
    e
}

/// A group heading: an uppercase name on the left and a quieter note on the
/// right (a shortcut, where the content comes from, what to click).
pub(super) fn group_label(commands: &mut Commands, fonts: &EmberFonts, title: &str, note: &str) -> Entity {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Baseline,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let t = commands
        .spawn((
            Text::new(title.to_uppercase()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let n = commands
        .spawn((
            Text::new(note.to_string()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(placeholder())),
        ))
        .id();
    commands.entity(row).add_children(&[t, n]);
    row
}

/// Like [`group_label`], with a note that follows the world.
pub(super) fn group_label_live<F>(commands: &mut Commands, fonts: &EmberFonts, title: &str, note: F) -> Entity
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    let row = group_label(commands, fonts, title, "");
    let n = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 10.5), TextColor(rgb(placeholder()))))
        .id();
    bind_text(commands, n, note);
    commands.entity(row).add_child(n);
    row
}

/// A line of quiet explanatory text.
pub(super) fn hint(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 10.5),
            TextColor(rgb(placeholder())),
        ))
        .id()
}

/// A text line bound to the world.
pub(super) fn live_text<F>(commands: &mut Commands, fonts: &EmberFonts, size: f32, color: (u8, u8, u8), value: F) -> Entity
where
    F: for<'w> Fn(&Rx<'w>) -> String + Send + Sync + 'static,
{
    let e = commands
        .spawn((
            Text::new(""),
            ui_font(&fonts.ui, size),
            TextColor(rgb(color)),
            PASS_THROUGH,
        ))
        .id();
    bind_text(commands, e, value);
    e
}

// ── Rows ─────────────────────────────────────────────────────────────────────

/// `[label] [content...]`: a fixed-width muted label and a content area that
/// takes the rest of the width.
pub(super) fn row(commands: &mut Commands, fonts: &EmberFonts, label: &str) -> (Entity, Entity) {
    let row = commands
        .spawn(Node {
            width: Val::Percent(100.0),
            min_height: Val::Px(24.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            ..default()
        })
        .id();
    let lbl = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(text_muted())),
            Node { width: Val::Px(LABEL_W), flex_shrink: 0.0, ..default() },
        ))
        .id();
    let content = commands
        .spawn(Node {
            flex_grow: 1.0,
            min_width: Val::Px(0.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(6.0),
            ..default()
        })
        .id();
    commands.entity(row).add_children(&[lbl, content]);
    (row, content)
}

/// A numeric field that fills the space it is given. `snap` quantises the
/// model for whole-number fields (see [`DragSnap`]).
#[allow(clippy::too_many_arguments)]
pub(super) fn num_field<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    axis: &str,
    min: f32,
    max: f32,
    step: f32,
    snap: Option<f32>,
    get: G,
    set: S,
) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    // `min` is a transient seed: `bind_2way` corrects it from the world on
    // its first run, before the field is ever drawn.
    let dv = drag_value(commands, &fonts.ui, axis, value_text(), min, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    if let Some(s) = snap {
        commands.entity(dv).insert(DragSnap(s));
    }
    renzora_ember::inspector::fill_control(commands, dv);
    bind_2way(commands, dv, get, move |w: &mut World, v: &f32| {
        let v = if max > min { v.clamp(min, max) } else { *v };
        set(w, &v);
    });
    dv
}

/// A short unit after a field ("m", "°", "%"). Real units are the point of
/// this layout: a brush size of 0.081 told nobody anything.
pub(super) fn unit(commands: &mut Commands, fonts: &EmberFonts, text: &str) -> Entity {
    commands
        .spawn((
            Text::new(text.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
            Node { flex_shrink: 0.0, min_width: Val::Px(14.0), ..default() },
            PASS_THROUGH,
        ))
        .id()
}

/// `[label] [field] [unit]`.
#[allow(clippy::too_many_arguments)]
pub(super) fn num_row<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    unit_text: &str,
    min: f32,
    max: f32,
    step: f32,
    get: G,
    set: S,
) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let (r, content) = row(commands, fonts, label);
    let f = num_field(commands, fonts, "", min, max, step, None, get, set);
    commands.entity(content).add_child(f);
    if !unit_text.is_empty() {
        let u = unit(commands, fonts, unit_text);
        commands.entity(content).add_child(u);
    }
    r
}

/// `[label] [segmented control]`, bound two ways to an index.
pub(super) fn segmented_row<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    options: &[&str],
    get: G,
    set: S,
) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let (r, content) = row(commands, fonts, label);
    let seg = segmented(commands, &fonts.ui, options, 0);
    bind_2way(commands, seg, get, set);
    commands.entity(content).add_child(seg);
    r
}

/// `[label] [dropdown]`, bound two ways to an index.
pub(super) fn dropdown_row<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    options: &[&str],
    get: G,
    set: S,
) -> Entity
where
    G: for<'w> Fn(&Rx<'w>) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let (r, content) = row(commands, fonts, label);
    let dd = dropdown_compact(commands, fonts, options, 0, 140.0);
    bind_2way(commands, dd, get, set);
    commands.entity(content).add_child(dd);
    r
}

/// `[label] [switch]`.
pub(super) fn switch_row<G, S>(commands: &mut Commands, fonts: &EmberFonts, label: &str, get: G, set: S) -> (Entity, Entity)
where
    G: for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let (r, content) = row(commands, fonts, label);
    let sw = toggle_switch(commands, false);
    bind_2way(commands, sw, get, set);
    commands.entity(content).add_child(sw);
    (r, content)
}

// ── Buttons ──────────────────────────────────────────────────────────────────

/// A button. `primary` is the one action a card is for (Apply, Build): filled
/// with the accent. The returned entity carries `UiInteraction`; give it a
/// marker and a click system.
pub(super) fn button(commands: &mut Commands, fonts: &EmberFonts, icon: Option<&str>, label: &str, primary: bool) -> Entity {
    let (btn, _) = button_parts(commands, fonts, icon, label, primary);
    btn
}

/// [`button`], also returning its label so a caller can bind the text.
pub(super) fn button_parts(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: Option<&str>,
    label: &str,
    primary: bool,
) -> (Entity, Entity) {
    let btn = commands
        .spawn((
            Node {
                height: Val::Px(24.0),
                padding: UiRect::horizontal(Val::Px(9.0)),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                flex_shrink: 0.0,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(5.0)),
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            UiInteraction::default(),
            HoverCursor(bevy::window::SystemCursorIcon::Pointer),
            Name::new(format!("terrain-btn:{label}")),
        ))
        .id();
    bind_bg(commands, btn, move |w| {
        let hot = matches!(w.get::<UiInteraction>(btn), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed));
        match (primary, hot) {
            (true, _) => rgb(accent()),
            (false, true) => rgb(hover_bg()),
            (false, false) => rgb(popup_bg()),
        }
    });
    let fg = move || if primary { on_accent() } else { text_primary() };
    if let Some(name) = icon {
        let ic = icon_text(commands, &fonts.phosphor, name, fg(), 12.0);
        commands.entity(ic).insert(PASS_THROUGH);
        commands.entity(btn).add_child(ic);
    }
    let t = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.5),
            TextColor(rgb(fg())),
            PASS_THROUGH,
        ))
        .id();
    commands.entity(btn).add_child(t);
    (btn, t)
}

/// A flexible spacer for a button row.
pub(super) fn spacer(commands: &mut Commands) -> Entity {
    commands.spawn(Node { flex_grow: 1.0, ..default() }).id()
}

/// Background for a selectable tile or row: accent tint when chosen, the hover
/// surface under the pointer, else the card colour.
pub(super) fn choice_bg(w: &Rx, e: Entity, chosen: bool) -> Color {
    if chosen {
        rgb(accent()).with_alpha(0.22)
    } else if matches!(w.get::<UiInteraction>(e), Some(UiInteraction::Hovered) | Some(UiInteraction::Pressed)) {
        rgb(hover_bg())
    } else {
        rgb(popup_bg())
    }
}

/// Border for a selectable tile or row: the accent when chosen.
pub(super) fn bind_choice_border<F>(commands: &mut Commands, e: Entity, chosen: F)
where
    F: for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + 'static,
{
    renzora_ember::reactive::tracked::bind_with(
        commands,
        e,
        move |w| if chosen(w) { rgb(accent()) } else { Color::NONE },
        |world, target, c: &Color| {
            if let Some(mut b) = world.get_mut::<BorderColor>(target) {
                let want = BorderColor::all(*c);
                if *b != want {
                    *b = want;
                }
            }
        },
    );
}

/// Text colour that brightens when chosen.
pub(super) fn bind_choice_text<F>(commands: &mut Commands, e: Entity, chosen: F)
where
    F: for<'w> Fn(&Rx<'w>) -> bool + Send + Sync + 'static,
{
    bind_text_color(commands, e, move |w| if chosen(w) { rgb(text_primary()) } else { rgb(text_muted()) });
}
