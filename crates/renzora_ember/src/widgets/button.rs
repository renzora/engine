//! Button — a clickable box with themed hover/press states.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::style::{Role, Styled, WidgetState};
use crate::theme::*;

/// Marks a button so [`button_interact`] drives its `Styled.state`. Shared with
/// other button-like widgets (e.g. the number stepper's `±` keys).
#[derive(Component)]
pub(crate) struct EmberButton;

/// A clickable button with hover/press color states.
pub fn button(commands: &mut Commands, font: &bevy::text::FontSource, label: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                padding: UiRect::axes(Val::Px(12.0), Val::Px(6.0)),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("button"),
        ))
        .id();
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(font, 12.0),
            TextColor(rgb(text_primary())),
        ))
        .id();
    commands.entity(b).add_child(t);
    b
}

/// A clickable button with a leading Phosphor icon and a label, themed with the
/// same hover/press states as [`button`].
pub fn icon_label_button(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> Entity {
    icon_label_button_parts(commands, fonts, icon, label).0
}

/// [`icon_label_button`] that also hands back its icon and label entities, for
/// callers that restyle the parts — e.g. a toolbar that hides the label to fall
/// back to an icon-only button when the panel gets too narrow.
pub fn icon_label_button_parts(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
) -> (Entity, Entity, Entity) {
    let b = commands
        .spawn((
            Node {
                flex_direction: FlexDirection::Row,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                column_gap: Val::Px(5.0),
                padding: UiRect::axes(Val::Px(10.0), Val::Px(5.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                // A button is a fixed-size control, never a flexible one: left
                // shrinkable, a crowded toolbar squeezed it until its label broke
                // onto a second line ("New / Folder", "Add / Entity") and the row
                // grew taller. Now the row overflows into whatever *is* shrinkable
                // (search boxes, breadcrumbs) instead of mangling the buttons.
                flex_shrink: 0.0,
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::Button),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("icon-button"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 12.0);
    let t = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            // Belt-and-braces with `flex_shrink: 0.0` above: even if a caller
            // width-constrains the button, the label stays on one line.
            bevy::text::TextLayout::no_wrap(),
        ))
        .id();
    commands.entity(b).add_children(&[ic, t]);
    (b, ic, t)
}

/// An [`icon_label_button`] that degrades instead of deforming: while `compact`
/// reads true it drops the label, squares up to an icon-only key, and moves the
/// label to a hover tooltip so it stays identifiable.
///
/// WHY panels want this: a button is not a flexible control, so a crowded
/// toolbar can't take width from it — without a collapse rule the row either
/// overflows or (before [`icon_label_button_parts`] pinned `flex_shrink: 0.0`)
/// squeezed the button until its label broke onto a second line. Collapsing the
/// label buys back ~40-60px per button and keeps everything on one row.
pub fn icon_label_button_collapsing<F>(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    label: &str,
    compact: F,
) -> Entity
where
    F: Fn(&World) -> bool + Clone + Send + Sync + 'static,
{
    let (btn, _ic, text) = icon_label_button_parts(commands, fonts, icon, label);
    let hidden = compact.clone();
    crate::reactive::bind_display(commands, text, move |w| !hidden(w));
    let label = label.to_string();
    crate::reactive::bind_with(commands, btn, compact, move |w, e, compact: &bool| {
        if let Some(mut n) = w.get_mut::<Node>(e) {
            // Even padding + a square footprint in the collapsed form, so it
            // reads as a key rather than a label-less pill.
            n.padding = if *compact {
                UiRect::axes(Val::Px(6.0), Val::Px(5.0))
            } else {
                UiRect::axes(Val::Px(10.0), Val::Px(5.0))
            };
            n.min_width = if *compact { Val::Px(24.0) } else { Val::Auto };
        }
        // Only tip what the button no longer says: a tooltip echoing a visible
        // label is noise.
        if let Ok(mut ent) = w.get_entity_mut(e) {
            if *compact {
                ent.insert(crate::widgets::tooltip::HoverTooltip::new(label.clone()));
            } else {
                ent.remove::<crate::widgets::tooltip::HoverTooltip>();
            }
        }
    });
    btn
}

/// An icon-only square button (Styled `IconButton`), themed with the same
/// hover/press states as [`button`]. For chrome actions (close, gear, …).
pub fn icon_button(commands: &mut Commands, fonts: &EmberFonts, icon: &str) -> Entity {
    let b = commands
        .spawn((
            Node {
                width: Val::Px(22.0),
                height: Val::Px(22.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                border_radius: BorderRadius::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(rgb(tab_active())),
            Interaction::default(),
            EmberButton,
            Styled::new(Role::IconButton),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("icon-button"),
        ))
        .id();
    let ic = icon_text(commands, &fonts.phosphor, icon, text_muted(), 13.0);
    commands.entity(b).add_child(ic);
    b
}

pub(crate) fn button_interact(
    mut q: Query<(&Interaction, &mut Styled), (With<EmberButton>, Changed<Interaction>)>,
) {
    for (interaction, mut styled) in &mut q {
        styled.state = match interaction {
            Interaction::Pressed => WidgetState::Pressed,
            Interaction::Hovered => WidgetState::Hover,
            Interaction::None => WidgetState::Normal,
        };
    }
}
