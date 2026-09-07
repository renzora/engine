//! Shared widgets for the terrain tooling's compact rows: a labelled slider, a
//! labelled drag field, a labelled dropdown, and the cluster that groups them.
//!
//! These were the private helpers of a **brush settings bar** that ran across the
//! top of the scene. That bar is gone -- it drew size / strength / falloff, which
//! the Terrain component's own Brush Settings section already had, so it was a
//! second copy of one set of resources sitting over the viewport. What survives
//! is the widget vocabulary, which the generator section still builds from.
//!
//! They are deliberately compact: the value rides inside the label rather than in
//! a box of its own, because these rows are narrow.

use bevy::prelude::*;

use renzora_editor_framework::ActiveTool;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_2way, bind_text};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::*;
use renzora_ember::widgets::{drag_value_flat, dropdown_compact, slider_ranged, DragRange, DragSnap};


/// Slider width in the bar. Wide enough to aim with, narrow enough that three of
/// them plus the toggles still fit one line on a typical viewport.
pub(crate) const SLIDER_W: f32 = 78.0;
/// The fill shared by every one of the viewport's context bars — this one and
/// the generator's.
///
/// A step *past* the surface the tool strip above it uses, so the two read as
/// two bands rather than one tall one: the strip is flat `panel`, and mixing
/// `panel` toward `extreme` landed only two levels off it, which at a 1px rule's
/// worth of separation was not enough to see. Starting from `extreme` and
/// leaning toward the active-tab surface keeps both ends *theme* colours — the
/// thing that stops this walking into the background on a light palette, where
/// "a bit lighter" would mean "a bit closer to the page".
pub(crate) fn context_bar_bg() -> Color {
    mix(header_bg(), tab_active(), 0.22)
}
/// A horizontal run of related controls.
pub(crate) fn cluster(commands: &mut Commands, name: &str) -> Entity {
    commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(6.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            Name::new(format!("terrain-bar:{name}")),
        ))
        .id()
}
/// `[Label 20.0] [────●───]`. The value rides in the label rather than sitting in
/// its own box: a toolbar has no room for a third element per setting, and the
/// number is only ever glanced at.
#[allow(clippy::too_many_arguments)]
pub(crate) fn labelled_slider<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &'static str,
    min: f32,
    max: f32,
    decimals: usize,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + Copy + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    bind_text(commands, text, move |w| {
        format!("{label} {:.*}", decimals, get(w))
    });
    // Seeded at `min`; `bind_2way` corrects it from the world on its first run,
    // before the user ever sees it.
    let sld = slider_ranged(commands, min, min, max);
    commands.entity(sld).insert(Node {
        width: Val::Px(SLIDER_W),
        height: Val::Px(18.0),
        position_type: PositionType::Relative,
        align_items: AlignItems::Center,
        ..default()
    });
    bind_2way(commands, sld, get, set);
    commands.entity(row).add_children(&[text, sld]);
    row
}
/// `[Label] [12.5]` — a scrubbable number. `snap` quantizes the model for fields
/// whose setter rounds into an integer; without it the model and the rounded
/// read-back fight each other mid-drag.
#[allow(clippy::too_many_arguments)]
pub(crate) fn labelled_drag<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    min: f32,
    max: f32,
    step: f32,
    snap: Option<f32>,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> f32 + Send + Sync + 'static,
    S: Fn(&mut World, &f32) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(3.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let dv = drag_value_flat(commands, &fonts.ui, "", value_text(), min, step);
    commands.entity(dv).insert(DragRange { min, max });
    if let Some(s) = snap {
        commands.entity(dv).insert(DragSnap(s));
    }
    bind_2way(commands, dv, get, set);
    commands.entity(row).add_children(&[text, dv]);
    row
}
pub(crate) fn labelled_dropdown<G, S>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    label: &str,
    options: &[&str],
    width: f32,
    get: G,
    set: S,
) -> Entity
where
    G: Fn(&Rx) -> usize + Send + Sync + 'static,
    S: Fn(&mut World, &usize) + Send + Sync + 'static,
{
    let row = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                column_gap: Val::Px(4.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
        ))
        .id();
    let text = commands
        .spawn((
            Text::new(label.to_string()),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_muted())),
        ))
        .id();
    let dd = dropdown_compact(commands, fonts, options, 0, width);
    bind_2way(commands, dd, get, set);
    commands.entity(row).add_children(&[text, dd]);
    row
}
pub(crate) fn tool_is(w: &Rx, want: ActiveTool) -> bool {
    w.get_resource::<ActiveTool>().copied() == Some(want)
}
