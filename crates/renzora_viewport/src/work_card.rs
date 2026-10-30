//! The progress card at the top of the viewport while the scene fills in:
//! a label, a count and a bar, for whatever is in
//! [`ViewportWork`] (a new terrain being shaped and built, so far).
//!
//! **At the top, not the middle.** The work fills the scene in where you are
//! looking, nearest first, so a card over the centre would hide the very
//! thing that shows it working. The model-loading card (`load_overlay`) sits
//! in the middle because there is nothing to see until its model arrives.
//!
//! **A real fraction.** Unlike the model loader, the owners of this work
//! count it as they go, so the bar fills rather than just moving.
//!
//! It doesn't take the pointer: you can keep working while it runs.

use bevy::prelude::*;

use renzora::core::viewport_types::ViewportWork;
use renzora::core::PlayModeState;
use renzora_ember::font::{ui_font, EmberFonts};
use renzora_ember::reactive::tracked::{bind_display, bind_text, bind_with};
use renzora_ember::reactive::Rx;
use renzora_ember::theme::{accent, panel_bg, rgb, text_muted, text_primary};

/// Bar size in logical pixels.
const BAR_WIDTH: f32 = 260.0;
const BAR_HEIGHT: f32 = 6.0;
/// Gap above the card, clear of the viewport's own toolbar row.
const TOP: f32 = 12.0;

/// The first entry, the one the card shows.
fn first(w: &Rx) -> Option<renzora::core::viewport_types::WorkItem> {
    w.get_resource::<ViewportWork>().and_then(|work| work.items.first().cloned())
}

/// Build the card for a viewport's content node. Slot 0 only: the work is
/// global, so a copy in every view would repeat it.
pub(crate) fn build(commands: &mut Commands, fonts: &EmberFonts) -> Entity {
    // Spans the top of the viewport so the card can centre itself without
    // knowing the viewport's width.
    let strip = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                top: Val::Px(TOP),
                justify_content: JustifyContent::Center,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-work-strip"),
        ))
        .id();
    bind_display(commands, strip, |w| {
        first(w).is_some() && !w.get_resource::<PlayModeState>().is_some_and(|p| p.is_in_play_mode())
    });

    let card = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                ..default()
            },
            // Translucent like the stats readout: it sits over the render.
            BackgroundColor(rgb(panel_bg()).with_alpha(0.9)),
            bevy::picking::Pickable::IGNORE,
            Name::new("vp-work-card"),
        ))
        .id();

    // "Building terrain" on the left, "120 / 256" on the right.
    let row = commands
        .spawn((
            Node {
                width: Val::Px(BAR_WIDTH),
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let label = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 12.0), TextColor(rgb(text_primary())), bevy::picking::Pickable::IGNORE))
        .id();
    bind_text(commands, label, |w| first(w).map(|item| item.label).unwrap_or_default());
    let count = commands
        .spawn((Text::new(""), ui_font(&fonts.ui, 11.0), TextColor(rgb(text_muted())), bevy::picking::Pickable::IGNORE))
        .id();
    bind_text(commands, count, |w| {
        first(w).map(|item| format!("{} / {}", item.done, item.total)).unwrap_or_default()
    });
    commands.entity(row).add_children(&[label, count]);

    let track = commands
        .spawn((
            Node {
                width: Val::Px(BAR_WIDTH),
                height: Val::Px(BAR_HEIGHT),
                border_radius: BorderRadius::all(Val::Px(BAR_HEIGHT * 0.5)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::WHITE.with_alpha(0.12)),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    let fill = commands
        .spawn((
            Node { width: Val::Px(0.0), height: Val::Percent(100.0), ..default() },
            BackgroundColor(rgb(accent())),
            bevy::picking::Pickable::IGNORE,
        ))
        .id();
    // In whole pixels, so the bar is only touched when it visibly moves.
    bind_with(
        commands,
        fill,
        |w| {
            first(w)
                .filter(|item| item.total > 0)
                .map_or(0u32, |item| (BAR_WIDTH * item.done.min(item.total) as f32 / item.total as f32) as u32)
        },
        |world, fill, px: &u32| {
            if let Some(mut node) = world.get_mut::<Node>(fill) {
                node.width = Val::Px(*px as f32);
            }
        },
    );
    commands.entity(track).add_child(fill);

    commands.entity(card).add_children(&[row, track]);
    commands.entity(strip).add_child(card);
    strip
}
