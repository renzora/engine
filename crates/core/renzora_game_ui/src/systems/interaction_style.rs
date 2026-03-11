//! Applies interaction-state-dependent style overrides (hover, press, disabled).

use bevy::prelude::*;

use crate::components::UiInteractionStyle;

pub fn interaction_style_system(
    mut widgets: Query<
        (&Interaction, &UiInteractionStyle, &mut BackgroundColor, Option<&mut BorderColor>),
        Changed<Interaction>,
    >,
) {
    for (interaction, style, mut bg, border) in &mut widgets {
        let overrides = match interaction {
            Interaction::None => &style.normal,
            Interaction::Hovered => &style.hovered,
            Interaction::Pressed => &style.pressed,
        };

        if let Some(color) = overrides.bg_color {
            bg.0 = color;
        }

        if let Some(color) = overrides.border_color {
            if let Some(mut bc) = border {
                *bc = BorderColor::all(color);
            }
        }
    }
}
