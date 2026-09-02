//! The form-row vocabulary every tab builder is written in.
//!
//! A tab is a stack of ember `section`s; each section body is a stack of rows.
//! [`settings_row`] is the labeled, zebra-striped one; the `ctl_*` builders
//! produce the control that goes in its right-hand slot, already two-way bound
//! to the live resource so an edit lands the same frame.

use bevy::prelude::*;

use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::bind_2way;
use renzora_ember::reactive::Rx;
use renzora_ember::widgets::{drag_value, dropdown, toggle_switch, DragRange};

/// A labeled, zebra-striped form row — the shared ember `inspector_row` + its
/// stripe color, parented under `body`.
pub(crate) fn settings_row(
    commands: &mut Commands,
    fonts: &EmberFonts,
    body: Entity,
    idx: usize,
    label: &str,
    control: Entity,
) {
    let row = renzora_ember::inspector::inspector_row(commands, &fonts.ui, label, control);
    commands
        .entity(row)
        .insert(BackgroundColor(renzora_ember::inspector::inspector_stripe(idx)));
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
                padding: UiRect::axes(Val::Px(8.0), Val::Px(3.0)),
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
