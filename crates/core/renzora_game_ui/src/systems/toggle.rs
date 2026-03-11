//! Toggle switch: slides knob left/right on click.

use bevy::prelude::*;

use crate::components::{ToggleData, UiWidgetPart};

pub fn toggle_system(
    mut toggles: Query<(&mut ToggleData, &Interaction, &Children), Changed<Interaction>>,
    mut parts: Query<(&UiWidgetPart, &mut Node, &mut BackgroundColor)>,
) {
    for (mut data, interaction, children) in &mut toggles {
        if *interaction == Interaction::Pressed {
            data.on = !data.on;
        }

        for child in children.iter() {
            let Ok((part, mut node, mut bg)) = parts.get_mut(child) else {
                continue;
            };
            match part.role.as_str() {
                "track" => {
                    bg.0 = if data.on {
                        data.on_color
                    } else {
                        data.off_color
                    };
                }
                "knob" => {
                    // Slide knob to right when on, left when off
                    node.left = if data.on {
                        Val::Percent(50.0)
                    } else {
                        Val::Percent(0.0)
                    };
                    bg.0 = data.knob_color;
                }
                _ => {}
            }
        }
    }
}
