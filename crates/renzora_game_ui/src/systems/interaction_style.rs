//! Applies interaction-state-dependent style overrides (hover, press, disabled).
//!
//! When `Interaction` changes, merges the active state's `UiStateStyle` overrides
//! onto the individual style components. The downstream `apply_widget_style_system`
//! then syncs the result to bevy_ui components.

use bevy::prelude::*;

use crate::components::*;

pub fn interaction_style_system(
    mut widgets: Query<
        (
            &Interaction,
            &UiInteractionStyle,
            Option<&mut UiFill>,
            Option<&mut UiStroke>,
            Option<&mut UiOpacity>,
            Option<&mut UiBorderRadius>,
            Option<&mut UiCursor>,
            Option<&mut UiTextStyle>,
            Option<&mut UiPadding>,
        ),
        Changed<Interaction>,
    >,
) {
    for (interaction, istyle, fill, stroke, opacity, border_radius, cursor, text, padding) in &mut widgets {
        let overrides = match interaction {
            Interaction::None => &istyle.normal,
            Interaction::Hovered => &istyle.hovered,
            Interaction::Pressed => &istyle.pressed,
        };

        if let Some(ref f) = overrides.fill {
            if let Some(mut fill) = fill {
                *fill = f.clone();
            }
        }
        if let Some(ref s) = overrides.stroke {
            if let Some(mut stroke) = stroke {
                *stroke = s.clone();
            }
        }
        if let Some(o) = overrides.opacity {
            if let Some(mut opacity) = opacity {
                opacity.0 = o;
            }
        }
        if let Some(ref r) = overrides.border_radius {
            if let Some(mut border_radius) = border_radius {
                *border_radius = *r;
            }
        }
        if let Some(c) = overrides.cursor {
            if let Some(mut cursor) = cursor {
                *cursor = c;
            }
        }
        if let Some(mut text) = text {
            if let Some(color) = overrides.text_color {
                text.color = color;
            }
            if let Some(size) = overrides.text_size {
                text.size = size;
            }
        }
        if let Some(p) = overrides.padding {
            if let Some(mut padding) = padding {
                *padding = p;
            }
        }
    }
}
