//! Asset tile — a card with a square thumbnail (icon) over a label, themed from
//! [`crate::style::AssetTileStyle`] (card bg/hover, border, thumbnail). The asset
//! browser composes its own richer tile (live selection, rendered thumbnails,
//! drag); this is the reusable, showcaseable card.

use bevy::prelude::*;
use bevy::window::SystemCursorIcon;

use crate::font::{icon_text, ui_font, EmberFonts};
use crate::reactive::{bind_bg, bind_with};
use crate::style::Theme;
use crate::theme::{rgb, text_primary};

/// A themed asset card: a thumbnail icon over a label. Hover-reactive; colors
/// come from `Theme.asset_tile` (editable in the Theme tab) and repaint live.
pub fn asset_tile(
    commands: &mut Commands,
    fonts: &EmberFonts,
    icon: &str,
    icon_color: (u8, u8, u8),
    label: &str,
) -> Entity {
    let card = commands
        .spawn((
            Node {
                width: Val::Px(92.0),
                flex_direction: FlexDirection::Column,
                border: UiRect::all(Val::Px(1.0)),
                border_radius: BorderRadius::all(Val::Px(4.0)),
                overflow: Overflow::clip(),
                ..default()
            },
            BackgroundColor(Color::NONE),
            Interaction::default(),
            crate::cursor_icon::HoverCursor(SystemCursorIcon::Pointer),
            Name::new("asset-tile"),
        ))
        .id();
    bind_bg(commands, card, move |w| {
        let hovered = matches!(
            w.get::<Interaction>(card),
            Some(Interaction::Hovered) | Some(Interaction::Pressed)
        );
        let st = &w.resource::<Theme>().asset_tile;
        if hovered {
            st.card_hover.color()
        } else {
            st.card_bg.color()
        }
    });
    bind_with(
        commands,
        card,
        move |w| {
            matches!(
                w.get::<Interaction>(card),
                Some(Interaction::Hovered) | Some(Interaction::Pressed)
            )
        },
        move |w, e, hovered: &bool| {
            let (hov, norm) = {
                let st = &w.resource::<Theme>().asset_tile;
                (st.border_selected.color(), st.border.color())
            };
            if let Some(mut bc) = w.get_mut::<BorderColor>(e) {
                *bc = BorderColor::all(if *hovered { hov } else { norm });
            }
        },
    );

    let thumb = commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(72.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::NONE),
        ))
        .id();
    bind_bg(commands, thumb, |w| {
        w.resource::<Theme>().asset_tile.thumb_bg.color()
    });
    let ic = icon_text(commands, &fonts.phosphor, icon, icon_color, 34.0);
    commands.entity(thumb).add_child(ic);

    let lbl = commands
        .spawn((
            Text::new(label),
            ui_font(&fonts.ui, 11.0),
            TextColor(rgb(text_primary())),
            bevy::text::TextLayout::no_wrap(),
            Node {
                padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)),
                ..default()
            },
        ))
        .id();
    commands.entity(card).add_children(&[thumb, lbl]);
    card
}
