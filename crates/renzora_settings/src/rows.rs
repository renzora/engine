//! The form-row vocabulary every tab builder is written in.
//!
//! A tab is a stack of ember `section`s; each section body is a stack of rows.
//! [`settings_row`] is the labeled one; the `ctl_*` builders
//! produce the control that goes in its right-hand slot, already two-way bound
//! to the live resource so an edit lands the same frame.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{drag_value, dropdown, toggle_switch, DragRange};

/// Width of the control slot on the right of a [`settings_row`].
///
/// Fixed rather than filling the row: Settings is a form read down the page,
/// and a dropdown stretched to the window's edge was a long empty bar with its
/// value at one end and its caret at the other. A fixed slot keeps every
/// control the same size and lined up on one right-hand column.
const CONTROL_W: f32 = 220.0;

/// A labeled form row parented under `body`: the label on the left, taking the
/// free width and wrapping when it needs to, the control in a fixed slot on the
/// right, and a hairline between rows.
///
/// It used to be the inspector's row with a zebra stripe. The inspector's
/// narrow label column clipped Settings' longer labels ("Graphics Quality
/// (game)"), and the stripes made a short form look like a table.
///
/// `_idx` is the row's position in its section, from when rows were striped;
/// the tab builders still pass it.
pub(crate) fn settings_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    _idx: usize,
    label: &str,
    control: Entity,
) {
    use renzora_ember::theme::{divider, rgb, text_primary};
    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 12.0),
            TextColor(rgb(text_primary())),
            Node {
                flex_grow: 1.0,
                min_width: Val::Px(0.0),
                ..default()
            },
        ))
        .id();
    // Dropdowns and number fields fill the slot; toggles keep their own size,
    // pushed to its right edge so every control ends on the same line.
    let slot = commands
        .spawn((
            Node {
                width: Val::Px(CONTROL_W),
                flex_shrink: 0.0,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::FlexEnd,
                align_items: AlignItems::Center,
                ..default()
            },
            Name::new("settings-control"),
        ))
        .id();
    commands.entity(slot).add_child(control);
    renzora_ember::inspector::fill_if_growable(commands, control);
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(16.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(7.0)),
                border: UiRect::bottom(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(rgb(divider())),
            Name::new("settings-row"),
        ))
        .id();
    commands.entity(row).add_children(&[lbl, slot]);
    commands.entity(body).add_child(row);
}

/// A muted, control-less note row (the "takes effect after restart" lines).
pub(crate) fn note_row(commands: &mut Commands, fonts: &EmberFonts, body: Entity, text: &str) {
    let lbl = commands
        .spawn((
            Text::new(text),
            ui_font(&fonts.ui, 11.0),
            TextColor(renzora_ember::theme::rgb(renzora_ember::theme::text_muted())),
        ))
        .id();
    let row = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                ..default()
            },
            Name::new("note-row"),
        ))
        .id();
    commands.entity(row).add_child(lbl);
    commands.entity(body).add_child(row);
}

/// A horizontal container with the given children — a row inside a section body.
pub(crate) fn hrow(commands: &mut Commands, kids: &[Entity]) -> Entity {
    let row = commands
        .spawn((Node {
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(8.0), Val::Px(4.0)),
            ..default()
        },))
        .id();
    commands.entity(row).add_children(kids);
    row
}

/// A themed ember button (Styled(Role::Button)) carrying a marker — picks up
/// Theme.button + hover/press states, editable under "Button" in the Theme tab.
pub(crate) fn text_button<M: Component>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    marker: M,
) -> Entity {
    let btn = renzora_ember::widgets::button(commands, &fonts.ui, label);
    commands.entity(btn).insert(marker);
    btn
}

/// In a tab split into per-section categories, hide every section but the
/// focused one (`focus == Some(key)`). With `focus == None` the whole tab shows.
/// Sections stay parented (despawned with the panel — no leak) but get
/// `Display::None`.
pub(crate) fn focus_hide(commands: &mut Commands, sec: Entity, focus: Option<&str>, key: &str) {
    if focus.is_some() && focus != Some(key) {
        commands.entity(sec).queue(|mut e: EntityWorldMut| {
            if let Some(mut n) = e.get_mut::<Node>() {
                n.display = Display::None;
            }
        });
    }
}

// Control builders — each carries its own two-way binding to live state.

pub(crate) fn ctl_toggle<G, S>(commands: &mut Commands, init: bool, get: G, set: S) -> Entity
where
    G: Fn(&Rx) -> bool + Send + Sync + 'static,
    S: Fn(&mut World, &bool) + Send + Sync + 'static,
{
    let sw = toggle_switch(commands, init);
    bind_2way(commands, sw, get, set);
    sw
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn ctl_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    init: f32,
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
    let dv = drag_value(commands, &fonts.ui, "", renzora_ember::theme::value_text(), init, step);
    if max > min {
        commands.entity(dv).insert(DragRange { min, max });
    }
    bind_2way(commands, dv, get, set);
    dv
}

pub(crate) fn ctl_dropdown<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    options: &[&str],
    init: usize,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let dd = dropdown(commands, fonts, options, init);
    bind_2way(commands, dd, get, set);
    dd
}
