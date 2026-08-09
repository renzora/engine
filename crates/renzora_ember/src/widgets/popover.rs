//! Popover — a button that toggles a panel below it.
//!
//! All three builders sit on the shared [`Popup`](super::popup::Popup)
//! machinery. They used to carry their own `EmberPopover` component + toggle
//! system, which was a third copy of "click the trigger, flip the panel's
//! display" — and, more seriously, it meant their panels were never tagged
//! [`OverlaySurface`](super::popup::OverlaySurface) (`tag_popup_panels` only
//! looks for `Popup`). An untagged floating panel is invisible to
//! `correct_pointer_state`, so clicks and scrolls leaked straight through it to
//! whatever sat behind. Building on `Popup` fixes that and adds outside-click
//! dismiss and flip-up-when-clipped positioning, which the local version never
//! had.

use bevy::prelude::*;

use crate::font::{icon_text, EmberFonts};
use crate::theme::*;

use super::button::{button, icon_label_button};
use super::popup::Popup;

/// The panel body shared by the three popover builders: a floating surface
/// holding `content`. Tagging happens via `Popup` on the trigger, so this only
/// has to describe the look.
fn popover_panel(commands: &mut Commands, content: Entity) -> Entity {
    let panel = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(100.0),
                left: Val::Px(0.0),
                margin: UiRect::top(Val::Px(4.0)),
                padding: UiRect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(6.0)),
                display: Display::None,
                ..default()
            },
            BackgroundColor(rgb(popup_bg())),
            BorderColor::all(rgb(border())),
            GlobalZIndex(600),
            Name::new("popover-panel"),
        ))
        .id();
    commands.entity(panel).add_child(content);
    panel
}

/// The `position: relative` wrapper holding a trigger + its panel.
fn popover_wrap(commands: &mut Commands, trigger: Entity, panel: Entity) -> Entity {
    let wrap = commands
        .spawn((
            Node {
                position_type: PositionType::Relative,
                ..default()
            },
            Name::new("popover"),
        ))
        .id();
    commands.entity(trigger).insert(Popup::new(panel));
    commands.entity(wrap).add_children(&[trigger, panel]);
    wrap
}

/// A button that toggles a popover panel below it (holding `content`).
pub fn popover(commands: &mut Commands, fonts: &EmberFonts, label: &str, content: Entity) -> Entity {
    let trigger = button(commands, &fonts.ui, label);
    let panel = popover_panel(commands, content);
    popover_wrap(commands, trigger, panel)
}

/// Like [`popover`] but the trigger is a framed icon+label button (e.g. a "+"
/// glyph beside "Add"). Returns the wrapper entity.
pub fn labeled_icon_popover(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    content: Entity,
) -> Entity {
    let trigger = icon_label_button(commands, fonts, icon, label);
    let panel = popover_panel(commands, content);
    popover_wrap(commands, trigger, panel)
}

/// Like [`popover`] but triggered by a Phosphor icon instead of a text label
/// (e.g. a gear/cog). Returns the wrapper entity.
pub fn icon_popover(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    size: f32,
    content: Entity,
) -> Entity {
    let trigger = icon_text(commands, &fonts.phosphor, icon, text_muted(), size);
    commands.entity(trigger).insert((
        Interaction::default(),
        crate::cursor_icon::HoverCursor(bevy::window::SystemCursorIcon::Pointer),
    ));
    let panel = popover_panel(commands, content);
    popover_wrap(commands, trigger, panel)
}
