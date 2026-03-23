//! Checkbox: toggles `checked` on click, updates checkmark visibility.

use bevy::prelude::*;

use crate::components::{CheckboxData, UiWidgetPart};

pub fn checkbox_system(
    mut checkboxes: Query<(&mut CheckboxData, &Interaction, &Children), Changed<Interaction>>,
    mut parts: Query<(&UiWidgetPart, &mut Visibility, &mut BackgroundColor)>,
) {
    for (mut data, interaction, children) in &mut checkboxes {
        if *interaction == Interaction::Pressed {
            data.checked = !data.checked;
        }

        for child in children.iter() {
            let Ok((part, mut vis, mut bg)) = parts.get_mut(child) else {
                continue;
            };
            match part.role.as_str() {
                "checkmark" => {
                    *vis = if data.checked {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                    bg.0 = data.check_color;
                }
                "box" => {
                    bg.0 = data.box_color;
                }
                _ => {}
            }
        }
    }
}
