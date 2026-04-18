//! Dropdown: click to open/close option list, select an option.
//!
//! Structure:
//! - Root entity: DropdownData + Button + Interaction
//!   - Child "display": shows current selection text
//!   - Child "options_panel": Visibility toggled, contains option children
//!     - Child "option_N": clickable options

use bevy::prelude::*;

use crate::components::{DropdownData, UiWidgetPart};

pub fn dropdown_system(
    mut dropdowns: Query<(&mut DropdownData, &Interaction, &Children), Changed<Interaction>>,
    mut parts: Query<(&UiWidgetPart, &mut Visibility, Option<&Interaction>)>,
    mut texts: Query<&mut bevy::ui::widget::Text>,
) {
    for (mut data, interaction, children) in &mut dropdowns {
        // Toggle open on click
        if *interaction == Interaction::Pressed {
            data.open = !data.open;
        }

        // Update children
        for child in children.iter() {
            let Ok((part, mut vis, _)) = parts.get_mut(child) else {
                continue;
            };

            match part.role.as_str() {
                "display" => {
                    // Update display text
                    if let Ok(mut text) = texts.get_mut(child) {
                        if data.selected >= 0 && (data.selected as usize) < data.options.len() {
                            text.0 = data.options[data.selected as usize].clone();
                        } else {
                            text.0 = data.placeholder.clone();
                        }
                    }
                }
                "options_panel" => {
                    *vis = if data.open {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                _ => {}
            }
        }
    }
}

/// Handles clicks on individual dropdown options.
pub fn dropdown_option_system(
    options: Query<(&UiWidgetPart, &Interaction, &ChildOf), Changed<Interaction>>,
    panels: Query<(&UiWidgetPart, &ChildOf)>,
    mut dropdowns: Query<&mut DropdownData>,
) {
    for (part, interaction, child_of) in &options {
        if *interaction != Interaction::Pressed {
            continue;
        }

        // Check if this is an option_N part
        let Some(idx_str) = part.role.strip_prefix("option_") else {
            continue;
        };
        let Ok(idx) = idx_str.parse::<i32>() else {
            continue;
        };

        // Walk up: option -> options_panel -> dropdown root
        let panel_entity = child_of.parent();
        if let Ok((panel_part, panel_parent)) = panels.get(panel_entity) {
            if panel_part.role == "options_panel" {
                if let Ok(mut data) = dropdowns.get_mut(panel_parent.parent()) {
                    data.selected = idx;
                    data.open = false;
                }
            }
        }
    }
}
